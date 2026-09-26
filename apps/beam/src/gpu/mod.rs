//! GPU-native flat vector search on wgpu (Metal on this Mac).
//!
//! [`GpuContext`] owns the wgpu `Instance` / `Adapter` / `Device` / `Queue`.
//! [`GpuContext::new`] returns `None` when no adapter is reachable, so callers
//! (bench + tests) can skip gracefully on a GPU-less host.
//!
//! [`GpuFlatIndex`] uploads a collection's row-major vectors to a GPU **storage
//! buffer once** at construction. Each `search_knn` uploads the (small) query,
//! dispatches the `flat.wgsl` compute kernel (one invocation per DB row,
//! workgroup size 64), reads the `n` per-row distances back, and runs the shared
//! CPU top-k. Scores use the SAME convention as the CPU oracle
//! ([`crate::collection::Metric::code`]), so GPU and CPU top-k agree.
//!
//! wgpu API usage is matched to the pinned `wgpu = "24"` already in the
//! workspace (see `apps/jet/crates/grid-render-webgpu`): `Instance::new(&desc)`,
//! `request_adapter` → `Option`, `request_device(&desc, None)`,
//! `Maintain::Wait` blocking readback, `entry_point: Some(..)` on the compute
//! pipeline.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::collection::{l2_normalize, Collection, Metric};
use crate::index::{topk, Neighbor, VectorIndex};
use crate::payload::{Filter, Payload};

pub mod ivfpq;

/// Owns the wgpu handles for one GPU (adapter). Cheap to clone the `Device` /
/// `Queue` out of (both are Arc-backed handles).
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Acquire the primary GPU adapter (Metal on macOS) and open a device.
    /// Returns `None` if no adapter is present or the device request fails —
    /// the graceful-skip signal for GPU-less CI.
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Option<Self> {
        // `Backends::PRIMARY` = Metal | Vulkan | DX12 — Metal on this Mac.
        // `BEAM_WGPU_BACKEND=vulkan|metal|dx12|gl` narrows the selection: it
        // forces a specific backend for cross-backend verification (e.g. the
        // Vulkan path via MoltenVK on macOS, the same wgpu backend NVIDIA uses).
        let backends = match std::env::var("BEAM_WGPU_BACKEND").ok().as_deref() {
            Some("vulkan") => wgpu::Backends::VULKAN,
            Some("metal") => wgpu::Backends::METAL,
            Some("dx12") => wgpu::Backends::DX12,
            Some("gl") => wgpu::Backends::GL,
            _ => wgpu::Backends::PRIMARY,
        };
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        // Request exactly the adapter's limits so large storage buffers (the DB)
        // are bindable up to what Metal supports.
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("beam_gpu_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: adapter.limits(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .ok()?;
        Some(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// `(backend, name)` from `adapter.get_info()` — backend via `Debug`
    /// (e.g. `"Metal"`), name the device string (e.g. `"Apple M-series GPU"`).
    pub fn adapter_info(&self) -> (String, String) {
        let info = self.adapter.get_info();
        (format!("{:?}", info.backend), info.name)
    }
}

/// The `Params` uniform handed to the kernel: row count, dimension, metric code.
/// 16 bytes (uniform-buffer alignment friendly). `_pad` rounds to 16.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Params {
    n: u32,
    dim: u32,
    metric: u32,
    _pad: u32,
}

/// The `BatchParams` uniform handed to the `main_batch` kernel: row count,
/// dimension, metric code, and the number of queries in the current tile. 16
/// bytes (uniform-buffer alignment friendly), so no padding field is needed.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BatchParams {
    n: u32,
    dim: u32,
    metric: u32,
    num_q: u32,
}

/// The `TopkParams` uniform handed to the `main_batch_topk` kernel. Adds `want`
/// (`min(k, n_live)`, the per-query result length the kernel emits) to the batch
/// params; padded to 32 bytes (uniform alignment).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct TopkParams {
    n: u32,
    dim: u32,
    metric: u32,
    num_q: u32,
    want: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
}

/// The `TiledParams` uniform handed to the GEMM-tiled kernel (`main_batch_tiled`).
/// Adds the split-k geometry (`num_splits`, `split_len`) to the top-k params;
/// padded to 32 bytes (uniform alignment). Mirrors `TiledParams` in `flat.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct TiledParams {
    n: u32,
    dim: u32,
    metric: u32,
    num_q: u32,
    want: u32,
    num_splits: u32,
    split_len: u32,
    _p0: u32,
}

/// Compile-time cap on `k` for the GPU-side per-query top-k kernel
/// (`main_batch_topk`) — it MUST equal `MAX_K` in `flat.wgsl` (the per-thread
/// register top-k / shared-memory list length). A batched query with `k` at or
/// below this runs the GPU-top-k path (readback is `num_q * k`, tiny); a larger
/// `k` falls back to the `main_batch` `num_q * n` distance-matrix path + CPU
/// top-k, so large-k queries still work (just without the readback win).
pub const MAX_TOPK: usize = 32;

/// GEMM-tiled kernel (`main_batch_tiled`) tile shape — MUST mirror `TILE_Q_T` /
/// `TILE_N_T` / `MAX_TILE_DIM_T` in `flat.wgsl`. `TILE_Q_GEMM` queries share one
/// workgroup (== the `@workgroup_size`) and reuse each staged DB block, so each DB
/// element is read from global memory once per `TILE_Q_GEMM` queries instead of once
/// per query — the compute-bound lever. `TILE_N_GEMM` DB rows are staged per shared
/// tile. A batched query with `k <= MAX_TOPK` and `dim <= MAX_TILE_DIM` runs this
/// path; `dim > MAX_TILE_DIM` or `k > MAX_TOPK` falls back to `main_batch_topk` /
/// the distance-matrix path.
pub const TILE_Q_GEMM: usize = 64;
/// DB rows staged into shared memory per tile step (see [`TILE_Q_GEMM`]).
pub const TILE_N_GEMM: usize = 16;
/// Largest query dimension the tiled kernel supports (its shared-tile column
/// stride). Beyond this the host falls back to the `main_batch_topk` path.
pub const MAX_TILE_DIM: usize = 128;

/// Split-k tuning for the GEMM-tiled path: the batch has few query-tiles
/// (`ceil(num_q / TILE_Q_GEMM)`), so the DB row range is split across
/// `num_splits` workgroups PER query-tile to keep the GPU's cores busy. The host
/// targets ~[`GEMM_TARGET_WORKGROUPS`] total workgroups, never splitting finer than
/// [`GEMM_MIN_SPLIT_ROWS`] rows per split, capped at [`GEMM_MAX_SPLITS`]. Splitting
/// does NOT reduce DB reuse (reuse is across the queries within a workgroup); it
/// only trades a tiny per-query cross-split merge for GPU occupancy.
const GEMM_TARGET_WORKGROUPS: usize = 2048;
/// Never split a query-tile's DB range into pieces smaller than this many rows.
const GEMM_MIN_SPLIT_ROWS: usize = 1024;
/// Hard cap on split-k fan-out (bounds the partial readback + host merge).
const GEMM_MAX_SPLITS: usize = 256;

/// Cap on the per-tile readback: at most this many f32 distances (`tile × n`) are
/// computed + read back in one `main_batch` dispatch, so the batch is tiled into
/// chunks of `T = clamp(MAX_BATCH_TILE_FLOATS / n, 1, batch_len)` queries. ~8M
/// f32 ≈ 32 MB per tile — big enough to amortize the dispatch floor across many
/// queries, bounded enough to keep the readback buffer reasonable at n = 1M.
const MAX_BATCH_TILE_FLOATS: usize = 8_000_000;

/// Hard upper bound on the number of queries scored in one `main_batch` tile —
/// the kernel loops the tile's queries in-thread, so this simply caps the
/// in-kernel loop length (and the `tile * n` readback) at a sane ceiling
/// regardless of `n`. The readback cap ([`MAX_BATCH_TILE_FLOATS`]) is the usual
/// binding constraint; this is the fallback for very small `n`.
const MAX_BATCH_TILE_QUERIES: usize = 65_535;

/// GPU flat (brute-force) index. Owns the uploaded DB storage buffer and the
/// compute pipeline; `search_knn` is per-query upload → dispatch → readback.
pub struct GpuFlatIndex {
    device: wgpu::Device,
    queue: wgpu::Queue,
    dim: usize,
    /// Physical row count (live + tombstoned) = the number of rows uploaded to
    /// `db_buffer` and the kernel dispatch width.
    n: usize,
    metric: Metric,
    external_ids: Vec<String>,
    /// Row-aligned attribute payloads (snapshot of the collection), read on the
    /// host to build the GPU filter bitmask.
    payloads: Vec<Payload>,
    /// Per physical row liveness as the base keep-bitmask (`1` = live, `0` =
    /// tombstoned), snapshot from the collection. Folded into the filter mask so a
    /// tombstoned row is skipped by the SAME sentinel kernel filtered search uses.
    /// A delete-only change is reflected by [`GpuFlatIndex::refresh_mask`] WITHOUT
    /// re-uploading `db_buffer`.
    live: Vec<u32>,
    /// Cached live-row count (`== live.iter().filter(|&&b| b == 1).count()`).
    n_live: usize,
    /// The whole corpus (`n * dim` f32, live + tombstoned rows) uploaded once as a
    /// read-only storage buffer.
    db_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    /// Bind-group layout + pipeline for the `main_filtered` kernel (adds the
    /// per-row keep bitmask at binding 8; distances otherwise identical).
    filtered_bind_group_layout: wgpu::BindGroupLayout,
    filtered_pipeline: wgpu::ComputePipeline,
    /// Bind-group layout + pipeline for the `main_batch` kernel (scores a tile of
    /// queries against every row in one dispatch; the batched-throughput path).
    batch_bind_group_layout: wgpu::BindGroupLayout,
    batch_pipeline: wgpu::ComputePipeline,
    /// Bind-group layout + pipeline for the `main_batch_topk` kernel (one workgroup
    /// per query, GPU-side per-query top-k; reads back only `num_q * k` (id, score)
    /// pairs — the readback-killing batched-throughput path).
    topk_bind_group_layout: wgpu::BindGroupLayout,
    topk_pipeline: wgpu::ComputePipeline,
    /// Bind-group layout + pipeline for the `main_batch_tiled` GEMM kernel (a tile of
    /// TILE_Q queries reuses each shared-memory DB block across the tile, so DB rows
    /// are read from global memory once per tile instead of once per query — the
    /// compute-bound lever). Split-k over the DB range for occupancy; the host merges
    /// the per-(query, split) partial top-k.
    tiled_bind_group_layout: wgpu::BindGroupLayout,
    tiled_pipeline: wgpu::ComputePipeline,
}

impl GpuFlatIndex {
    /// Build the index: upload `collection`'s vectors to a GPU storage buffer
    /// once and compile the flat kernel. Reuses `ctx`'s device/queue.
    pub fn new(ctx: &GpuContext, collection: &Collection) -> Self {
        let device = ctx.device.clone();
        let queue = ctx.queue.clone();
        let dim = collection.dim();
        // Physical rows (live + tombstoned): the whole `data` buffer is uploaded and
        // the live-mask excludes tombstones at scoring time.
        let n = collection.capacity();
        let metric = collection.metric();

        // Upload the whole corpus once. `mapped_at_creation` via the util init
        // helper is the one-shot upload path; the buffer is read-only in-shader.
        let db_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("beam_db_vectors"),
            contents: bytemuck::cast_slice(collection.data()),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("beam_flat_kernel"),
            source: wgpu::ShaderSource::Wgsl(include_str!("flat.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beam_flat_bgl"),
            entries: &[
                // 0: db vectors (storage, read-only)
                storage_entry(0, true),
                // 1: query vector (storage, read-only)
                storage_entry(1, true),
                // 2: params (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 3: out distances (storage, read_write)
                storage_entry(3, false),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beam_flat_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("beam_flat_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions {
                zero_initialize_workgroup_memory: false,
                ..Default::default()
            },
            cache: None,
        });

        // Filtered kernel: same db/query/params/out (at disjoint bindings 4..8)
        // plus the per-row keep bitmask at binding 8.
        let filtered_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("beam_flat_filtered_bgl"),
                entries: &[
                    storage_entry(4, true), // db vectors
                    storage_entry(5, true), // query vector
                    // params (uniform)
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    storage_entry(7, false), // out distances (read_write)
                    storage_entry(8, true),  // keep bitmask (read-only)
                ],
            });
        let filtered_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("beam_flat_filtered_pipeline_layout"),
                bind_group_layouts: &[&filtered_bind_group_layout],
                push_constant_ranges: &[],
            });
        let filtered_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("beam_flat_filtered_pipeline"),
            layout: Some(&filtered_pipeline_layout),
            module: &shader,
            entry_point: Some("main_filtered"),
            compilation_options: wgpu::PipelineCompilationOptions {
                zero_initialize_workgroup_memory: false,
                ..Default::default()
            },
            cache: None,
        });

        // Batch kernel: db/queries/params/out at disjoint bindings 9..13 plus the
        // per-row keep bitmask at binding 13. Scores a tile of queries against
        // every row in one dispatch (the batched-throughput path).
        let batch_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("beam_flat_batch_bgl"),
                entries: &[
                    storage_entry(9, true),  // db vectors
                    storage_entry(10, true), // packed tile queries
                    // params (uniform)
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    storage_entry(12, false), // out distances (read_write, tile*n)
                    storage_entry(13, true),  // keep bitmask (read-only)
                ],
            });
        let batch_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("beam_flat_batch_pipeline_layout"),
                bind_group_layouts: &[&batch_bind_group_layout],
                push_constant_ranges: &[],
            });
        let batch_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("beam_flat_batch_pipeline"),
            layout: Some(&batch_pipeline_layout),
            module: &shader,
            entry_point: Some("main_batch"),
            compilation_options: wgpu::PipelineCompilationOptions {
                zero_initialize_workgroup_memory: false,
                ..Default::default()
            },
            cache: None,
        });

        // GPU-side per-query top-k kernel: db/queries/params/out at disjoint
        // bindings 14..18 plus the per-row keep bitmask at binding 18. One
        // workgroup per query; reads back only `num_q * k` (id, score) pairs.
        let topk_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("beam_flat_topk_bgl"),
                entries: &[
                    storage_entry(14, true), // db vectors
                    storage_entry(15, true), // packed tile queries
                    // params (uniform)
                    wgpu::BindGroupLayoutEntry {
                        binding: 16,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    storage_entry(17, false), // out (id, score) pairs (read_write)
                    storage_entry(18, true),  // keep bitmask (read-only)
                ],
            });
        let topk_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beam_flat_topk_pipeline_layout"),
            bind_group_layouts: &[&topk_bind_group_layout],
            push_constant_ranges: &[],
        });
        let topk_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("beam_flat_topk_pipeline"),
            layout: Some(&topk_pipeline_layout),
            module: &shader,
            entry_point: Some("main_batch_topk"),
            compilation_options: wgpu::PipelineCompilationOptions {
                zero_initialize_workgroup_memory: false,
                ..Default::default()
            },
            cache: None,
        });

        // GEMM-tiled kernel: db/queries/params/out at disjoint bindings 19..23 plus
        // the per-row keep bitmask at binding 23. A 2D workgroup grid (query-tile ×
        // DB split) with shared-memory DB reuse across the query tile.
        let tiled_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("beam_flat_tiled_bgl"),
                entries: &[
                    storage_entry(19, true), // db vectors
                    storage_entry(20, true), // packed tile queries
                    // params (uniform)
                    wgpu::BindGroupLayoutEntry {
                        binding: 21,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    storage_entry(22, false), // out partial (id, score) pairs (read_write)
                    storage_entry(23, true),  // keep bitmask (read-only)
                ],
            });
        let tiled_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("beam_flat_tiled_pipeline_layout"),
                bind_group_layouts: &[&tiled_bind_group_layout],
                push_constant_ranges: &[],
            });
        let tiled_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("beam_flat_tiled_pipeline"),
            layout: Some(&tiled_pipeline_layout),
            module: &shader,
            entry_point: Some("main_batch_tiled"),
            compilation_options: wgpu::PipelineCompilationOptions {
                zero_initialize_workgroup_memory: false,
                ..Default::default()
            },
            cache: None,
        });

        Self {
            device,
            queue,
            dim,
            n,
            metric,
            external_ids: collection.external_ids().to_vec(),
            payloads: collection.payloads().to_vec(),
            live: collection.live().iter().map(|&l| l as u32).collect(),
            n_live: collection.len(),
            db_buffer,
            bind_group_layout,
            pipeline,
            filtered_bind_group_layout,
            filtered_pipeline,
            batch_bind_group_layout,
            batch_pipeline,
            topk_bind_group_layout,
            topk_pipeline,
            tiled_bind_group_layout,
            tiled_pipeline,
        }
    }

    /// Number of **live** vectors (tombstoned rows excluded).
    pub fn len(&self) -> usize {
        self.n_live
    }

    /// Whether the index has zero live vectors.
    pub fn is_empty(&self) -> bool {
        self.n_live == 0
    }

    /// Physical row count (live + tombstoned) uploaded to the GPU.
    pub fn capacity(&self) -> usize {
        self.n
    }

    /// Number of tombstoned rows still resident in the GPU buffer (masked out of
    /// search).
    pub fn tombstoned(&self) -> usize {
        self.n - self.n_live
    }

    /// Re-sync the live-mask from `collection` WITHOUT re-uploading the vector
    /// buffer — the mask-only path for reflecting deletes on the GPU (requirement:
    /// avoid an O(n) re-upload on every delete). Valid only when no physical rows
    /// were added since the index was built (i.e. delete-only changes, so
    /// `collection.capacity() == self.capacity()`); returns `false` (a no-op)
    /// otherwise, signalling the caller to rebuild to pick up appended rows (an
    /// update/upsert appends, so it needs a rebuild to re-materialize the buffer).
    pub fn refresh_mask(&mut self, collection: &Collection) -> bool {
        if collection.capacity() != self.n {
            return false;
        }
        // Deletes only flip live bits and drop id-map entries; the per-row vectors,
        // external ids, and payloads of existing rows are unchanged, so only the
        // mask + live count need refreshing.
        self.live = collection.live().iter().map(|&l| l as u32).collect();
        self.n_live = collection.len();
        true
    }

    /// Run the kernel over every row and return the raw `n` per-row distances.
    fn compute_distances(&self, query: &[f32]) -> Vec<f32> {
        // Cosine normalizes the query on the host so the (unit) DB rows give a
        // true cosine similarity; L2/Dot upload the query as-is.
        let q = match self.metric {
            Metric::Cosine => l2_normalize(query),
            _ => query.to_vec(),
        };

        let query_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_query"),
                contents: bytemuck::cast_slice(&q),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let params = Params {
            n: self.n as u32,
            dim: self.dim as u32,
            metric: self.metric.code(),
            _pad: 0,
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let out_bytes = (self.n * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_out_dist"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_readback"),
            size: out_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beam_flat_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.db_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: out_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beam_flat_encoder"),
            });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("beam_flat_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            // One invocation per row, workgroup size 64 → ceil(n / 64) workgroups.
            let workgroups = (self.n as u32).div_ceil(64).max(1);
            cpass.dispatch_workgroups(workgroups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Blocking readback: map, then `Maintain::Wait` drives the queue to
        // completion and runs the map callback synchronously on this thread.
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async callback dropped")
            .expect("gpu buffer map failed");

        let data = slice.get_mapped_range();
        let scores: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        readback.unmap();
        scores
    }

    /// Run the filtered kernel: upload the query + the host-built per-row `keep`
    /// bitmask, dispatch `main_filtered` (which writes the metric's worst-case
    /// sentinel for dropped rows and the exact distance for kept rows), and read
    /// back the `n` per-row scores. Only kept rows carry a real score; the host
    /// top-k caps at the match count so sentinels are never selected.
    fn compute_distances_filtered(&self, query: &[f32], mask: &[u32]) -> Vec<f32> {
        let q = match self.metric {
            Metric::Cosine => l2_normalize(query),
            _ => query.to_vec(),
        };

        let query_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_query_filtered"),
                contents: bytemuck::cast_slice(&q),
                usage: wgpu::BufferUsages::STORAGE,
            });
        let mask_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_keep_mask"),
                contents: bytemuck::cast_slice(mask),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let params = Params {
            n: self.n as u32,
            dim: self.dim as u32,
            metric: self.metric.code(),
            _pad: 0,
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_params_filtered"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let out_bytes = (self.n * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_out_dist_filtered"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_readback_filtered"),
            size: out_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beam_flat_filtered_bind_group"),
            layout: &self.filtered_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.db_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: out_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: mask_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beam_flat_filtered_encoder"),
            });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("beam_flat_filtered_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.filtered_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            let workgroups = (self.n as u32).div_ceil(64).max(1);
            cpass.dispatch_workgroups(workgroups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async callback dropped")
            .expect("gpu buffer map failed");

        let data = slice.get_mapped_range();
        let scores: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        readback.unmap();
        scores
    }

    /// **Batched** k-NN: score a whole set of queries with as few GPU dispatches
    /// as possible, so the fixed per-dispatch + blocking-readback overhead
    /// amortizes across the batch instead of being paid once per query.
    ///
    /// Three GPU paths, auto-selected by `k` and `dim`:
    ///
    /// - `k <= `[`MAX_TOPK`] **and** `dim <= `[`MAX_TILE_DIM`] — the **GEMM-tiled**
    ///   path ([`Self::search_knn_batch_gemm`]): a tile of [`TILE_Q_GEMM`] queries
    ///   shares each shared-memory DB block, so DB rows are read from global memory
    ///   once per tile instead of once per query (the compute-bound lever that lifts
    ///   the memory-bound plateau). GPU-side per-query top-k, split-k for occupancy.
    /// - `k <= `[`MAX_TOPK`] but `dim > `[`MAX_TILE_DIM`] — the **GPU top-k** path
    ///   ([`Self::search_knn_batch_topk_tiled`]): ONE workgroup per query scans all
    ///   rows and selects the top-k on the GPU (no shared-memory tiling; used when
    ///   the query dim exceeds the tiled kernel's shared-tile width). Readback is
    ///   only `num_q * k` (id, score) pairs.
    /// - `k > `[`MAX_TOPK`] — the **distance-matrix** fallback
    ///   ([`Self::search_knn_batch_distmatrix`]): each tile scores its `T × n`
    ///   distance sub-matrix in one `main_batch` dispatch, reads back `T × n` f32,
    ///   and runs the shared CPU top-k — so large-k queries still work (the GPU
    ///   register/shared top-k is capped at `MAX_TOPK`).
    ///
    /// All fold the collection's live mask into the SAME keep-bitmask filtered +
    /// deleted single-query search uses, so a batched query excludes tombstoned
    /// rows and returns, per query, **exactly** what serial [`Self::search_knn`]
    /// returns (same row set, per-row scores bit-for-intent identical).
    ///
    /// Accepts anything sliceable to `[f32]` (e.g. `&[Vec<f32>]` or `&[&[f32]]`).
    /// A query whose length is not [`dim`](Self::capacity) yields an empty result
    /// at its position (matching the serial dimension-mismatch contract); an empty
    /// batch, `k == 0`, or an empty/all-tombstoned index yields all-empty results.
    pub fn search_knn_batch<Q: AsRef<[f32]>>(&self, queries: &[Q], k: usize) -> Vec<Vec<Neighbor>> {
        if k <= MAX_TOPK && self.dim <= MAX_TILE_DIM && self.dim.is_multiple_of(4) {
            // GEMM-tiled: shared-memory DB reuse across a query tile + vec4 inner loop
            // (the fast path for the common metric/k/dim). Needs dim <= MAX_TILE_DIM
            // (shared-tile width) and dim % 4 == 0 (the vec4 lanes).
            self.search_knn_batch_gemm(queries, k)
        } else if k <= MAX_TOPK {
            // dim too wide for the tiled kernel's shared tile (or not vec4-aligned):
            // one workgroup per query.
            let tile = queries.len().clamp(1, MAX_BATCH_TILE_QUERIES);
            self.search_knn_batch_topk_tiled(queries, k, tile)
        } else {
            self.search_knn_batch_distmatrix(queries, k)
        }
    }

    /// [`Self::search_knn_batch`] with an explicit query-tile size — the batch is
    /// processed in dispatches of at most `tile` queries each (`tile` is clamped to
    /// `1..=MAX_BATCH_TILE_QUERIES`). Routes to the same `k`-selected path as
    /// [`Self::search_knn_batch`]; the auto path picks `tile` itself, this hook lets
    /// a caller force a specific tile (used by the batched tests to cross the tiling
    /// boundary on a small corpus). Results are identical for any valid `tile`,
    /// since tiling only changes how many queries share one dispatch.
    #[doc(hidden)]
    pub fn search_knn_batch_tiled<Q: AsRef<[f32]>>(
        &self,
        queries: &[Q],
        k: usize,
        tile: usize,
    ) -> Vec<Vec<Neighbor>> {
        if k <= MAX_TOPK {
            self.search_knn_batch_topk_tiled(queries, k, tile)
        } else {
            self.search_knn_batch_distmatrix_tiled(queries, k, tile)
        }
    }

    /// GPU-side per-query top-k batched path: one `main_batch_topk` dispatch per
    /// tile (ONE workgroup per query), reading back only `num_q * want` (id, score)
    /// pairs — the throughput lever that removes the `num_q * n` readback + CPU
    /// top-k. `want = min(k, n_live)` (the same result length the serial path
    /// returns) and MUST be `<= `[`MAX_TOPK`]; callers route here only for
    /// `k <= MAX_TOPK`. Edge cases match [`Self::search_knn_batch`] (empty batch /
    /// `k == 0` / empty-or-all-tombstoned index → all-empty; wrong-dim query → empty
    /// slot). Result rows are already GPU-sorted best-first; the host only attaches
    /// each row's external id.
    #[doc(hidden)]
    pub fn search_knn_batch_topk_tiled<Q: AsRef<[f32]>>(
        &self,
        queries: &[Q],
        k: usize,
        tile: usize,
    ) -> Vec<Vec<Neighbor>> {
        let mut results: Vec<Vec<Neighbor>> = vec![Vec::new(); queries.len()];
        if queries.is_empty() || k == 0 || self.n == 0 || self.n_live == 0 {
            return results;
        }
        // The kernel emits exactly `want` real rows per query (want <= n_live), so
        // the result length matches the serial path's `min(k, n_live)`.
        let want = k.min(self.n_live).min(MAX_TOPK);
        let tile = tile.clamp(1, MAX_BATCH_TILE_QUERIES);

        let valid: Vec<usize> = (0..queries.len())
            .filter(|&i| queries[i].as_ref().len() == self.dim)
            .collect();
        if valid.is_empty() {
            return results;
        }

        // The keep-bitmask (collection live bits) is identical across every tile,
        // so upload it once and reuse it for all dispatches in this batch.
        let mask_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_topk_keep_mask"),
                contents: bytemuck::cast_slice(&self.live),
                usage: wgpu::BufferUsages::STORAGE,
            });

        for chunk in valid.chunks(tile) {
            // Pack this tile's (host-normalized) queries contiguously in `chunk` order.
            let mut packed = Vec::with_capacity(chunk.len() * self.dim);
            for &qi in chunk {
                match self.metric {
                    Metric::Cosine => packed.extend_from_slice(&l2_normalize(queries[qi].as_ref())),
                    _ => packed.extend_from_slice(queries[qi].as_ref()),
                }
            }
            // One dispatch → `chunk.len() * want` (row, score) pairs, query-major and
            // already sorted best-first.
            let pairs = self.dispatch_batch_topk(&packed, chunk.len(), want, &mask_buffer);
            for (local, &qi) in chunk.iter().enumerate() {
                let base = local * want;
                results[qi] = pairs[base..base + want]
                    .iter()
                    .map(|&(row, score)| Neighbor {
                        row,
                        external_id: self.external_ids[row as usize].clone(),
                        score,
                    })
                    .collect();
            }
        }
        results
    }

    /// **GEMM-tiled** batched path: the compute-bound lever. A tile of
    /// [`TILE_Q_GEMM`] queries shares one workgroup and reuses each staged
    /// shared-memory DB block, so every DB element is read from global memory once
    /// per `TILE_Q_GEMM` queries instead of once per query (the memory-bound plateau
    /// fix). The DB row range is split across `num_splits` workgroups per query-tile
    /// (split-k) for GPU occupancy; each thread selects its query's top-`want` over
    /// its split on the GPU, and the host merges the `num_splits` disjoint partials
    /// per query to the global top-k. Requires `k <= `[`MAX_TOPK`] and
    /// `dim <= `[`MAX_TILE_DIM`] (callers route here only when both hold). Edge cases
    /// match [`Self::search_knn_batch`] (empty batch / `k == 0` /
    /// empty-or-all-tombstoned index → all-empty; wrong-dim query → empty slot). The
    /// result equals, per query, exactly what the serial path and the CPU oracle
    /// return (exact flat).
    #[doc(hidden)]
    pub fn search_knn_batch_gemm<Q: AsRef<[f32]>>(
        &self,
        queries: &[Q],
        k: usize,
    ) -> Vec<Vec<Neighbor>> {
        let mut results: Vec<Vec<Neighbor>> = vec![Vec::new(); queries.len()];
        if queries.is_empty() || k == 0 || self.n == 0 || self.n_live == 0 {
            return results;
        }
        // Each split emits up to `want` real rows; the cross-split merge yields the
        // same `min(k, n_live)` the serial path returns. `want <= MAX_K` by the
        // caller's `k <= MAX_TOPK` gate.
        let want = k.min(self.n_live).min(MAX_TOPK);

        let valid: Vec<usize> = (0..queries.len())
            .filter(|&i| queries[i].as_ref().len() == self.dim)
            .collect();
        if valid.is_empty() {
            return results;
        }

        // Pack every valid (host-normalized) query contiguously in `valid` order.
        let mut packed = Vec::with_capacity(valid.len() * self.dim);
        for &qi in &valid {
            match self.metric {
                Metric::Cosine => packed.extend_from_slice(&l2_normalize(queries[qi].as_ref())),
                _ => packed.extend_from_slice(queries[qi].as_ref()),
            }
        }
        let num_q = valid.len();

        // Split-k geometry: aim for ~GEMM_TARGET_WORKGROUPS total workgroups
        // (num_qtiles × num_splits) for occupancy, never splitting finer than
        // GEMM_MIN_SPLIT_ROWS rows, capped at GEMM_MAX_SPLITS. `split_len` is derived
        // last and `num_splits` recomputed from it so no trailing split is empty.
        let num_qtiles = num_q.div_ceil(TILE_Q_GEMM).max(1);
        let splits_for_occupancy = (GEMM_TARGET_WORKGROUPS / num_qtiles).max(1);
        let max_splits_by_rows = self.n.div_ceil(GEMM_MIN_SPLIT_ROWS).max(1);
        let target_splits = splits_for_occupancy
            .min(max_splits_by_rows)
            .min(self.n)
            .clamp(1, GEMM_MAX_SPLITS);
        let split_len = self.n.div_ceil(target_splits).max(1);
        let num_splits = self.n.div_ceil(split_len);

        // The keep-bitmask (collection live bits) is identical for the whole batch.
        let mask_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_tiled_keep_mask"),
                contents: bytemuck::cast_slice(&self.live),
                usage: wgpu::BufferUsages::STORAGE,
            });

        // One dispatch → per-(query, split) partial top-`want` (row, score) pairs,
        // laid out `(local_q * num_splits + sp) * want + j`.
        let partials =
            self.dispatch_batch_tiled(&packed, num_q, want, num_splits, split_len, &mask_buffer);

        // Merge each query's `num_splits` disjoint partial lists to its global top-k.
        let stride = num_splits * want;
        let mut cand: Vec<(u32, f32)> = Vec::with_capacity(stride);
        for (local, &qi) in valid.iter().enumerate() {
            cand.clear();
            let base = local * stride;
            for entry in &partials[base..base + stride] {
                // Padding slots (a split with < want live rows) carry EMPTY_ROW.
                if entry.0 != u32::MAX {
                    cand.push(*entry);
                }
            }
            merge_split_topk(&mut cand, self.metric, want);
            results[qi] = cand
                .iter()
                .map(|&(row, score)| Neighbor {
                    row,
                    external_id: self.external_ids[row as usize].clone(),
                    score,
                })
                .collect();
        }
        results
    }

    /// [`Self::search_knn_batch`]'s **distance-matrix** path, forced regardless of
    /// `k` — the `T × n` distance sub-matrix per tile + CPU top-k (the pre-GPU-top-k
    /// batched path). Exposed so a test can compare the GPU-top-k path against it
    /// directly, and used as the `k > `[`MAX_TOPK`] fallback.
    #[doc(hidden)]
    pub fn search_knn_batch_distmatrix<Q: AsRef<[f32]>>(
        &self,
        queries: &[Q],
        k: usize,
    ) -> Vec<Vec<Neighbor>> {
        // Auto-pick the tile size so one dispatch reads back at most
        // MAX_BATCH_TILE_FLOATS distances.
        let tile = (MAX_BATCH_TILE_FLOATS / self.n.max(1)).clamp(1, MAX_BATCH_TILE_QUERIES);
        self.search_knn_batch_distmatrix_tiled(queries, k, tile)
    }

    /// [`Self::search_knn_batch_distmatrix`] with an explicit query-tile size. Each
    /// chunk is ONE `main_batch` dispatch that computes the `chunk.len() × n`
    /// distance sub-matrix (query-major), reads back `chunk.len() × n` f32, and runs
    /// the shared CPU top-k per query.
    #[doc(hidden)]
    pub fn search_knn_batch_distmatrix_tiled<Q: AsRef<[f32]>>(
        &self,
        queries: &[Q],
        k: usize,
        tile: usize,
    ) -> Vec<Vec<Neighbor>> {
        let mut results: Vec<Vec<Neighbor>> = vec![Vec::new(); queries.len()];
        if queries.is_empty() || k == 0 || self.n == 0 || self.n_live == 0 {
            return results;
        }
        let tile = tile.clamp(1, MAX_BATCH_TILE_QUERIES);
        // The kernel scores every physical row and sentinels the non-live ones, so
        // top-k is capped at the live count exactly like the serial path.
        let want = k.min(self.n_live);

        // Only well-dimensioned queries enter the GPU batch; a wrong-dim query
        // keeps its empty result (the serial contract). Cosine normalizes each
        // query on the host so the unit DB rows give a true cosine similarity.
        let valid: Vec<usize> = (0..queries.len())
            .filter(|&i| queries[i].as_ref().len() == self.dim)
            .collect();
        if valid.is_empty() {
            return results;
        }

        // The keep-bitmask (collection live bits) is identical across every tile,
        // so upload it once and reuse the buffer for all dispatches in this batch.
        let mask_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_batch_keep_mask"),
                contents: bytemuck::cast_slice(&self.live),
                usage: wgpu::BufferUsages::STORAGE,
            });

        for chunk in valid.chunks(tile) {
            // Pack this tile's (host-normalized) queries into one contiguous
            // `chunk.len() * dim` buffer, in `chunk` order.
            let mut packed = Vec::with_capacity(chunk.len() * self.dim);
            for &qi in chunk {
                match self.metric {
                    Metric::Cosine => packed.extend_from_slice(&l2_normalize(queries[qi].as_ref())),
                    _ => packed.extend_from_slice(queries[qi].as_ref()),
                }
            }
            // One dispatch → the `chunk.len() × n` distance sub-matrix (query-major).
            let scores = self.dispatch_batch(&packed, chunk.len(), &mask_buffer);
            // CPU top-k per query over its contiguous `n`-distance row.
            for (local, &qi) in chunk.iter().enumerate() {
                let row = &scores[local * self.n..(local + 1) * self.n];
                results[qi] = topk(row, self.metric, want, &self.external_ids);
            }
        }
        results
    }

    /// One `main_batch` dispatch: upload the packed `num_q` tile queries + params,
    /// run the 2D grid (one invocation per (query, DB row)) against the resident DB
    /// buffer and the reused keep `mask_buffer`, and read back the `num_q * n`
    /// query-major distance sub-matrix.
    fn dispatch_batch(
        &self,
        packed_q: &[f32],
        num_q: usize,
        mask_buffer: &wgpu::Buffer,
    ) -> Vec<f32> {
        let query_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_batch_queries"),
                contents: bytemuck::cast_slice(packed_q),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let params = BatchParams {
            n: self.n as u32,
            dim: self.dim as u32,
            metric: self.metric.code(),
            num_q: num_q as u32,
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_batch_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let out_len = num_q * self.n;
        let out_bytes = (out_len * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_batch_out_dist"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_batch_readback"),
            size: out_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beam_flat_batch_bind_group"),
            layout: &self.batch_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.db_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: out_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 13,
                    resource: mask_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beam_flat_batch_encoder"),
            });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("beam_flat_batch_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.batch_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            // 1D grid over DB rows (workgroup size 64): each invocation scores its
            // row against ALL `num_q` tile queries (looped in-kernel), so the whole
            // tile is one dispatch and each DB row is read from global memory once.
            let wg_x = (self.n as u32).div_ceil(64).max(1);
            cpass.dispatch_workgroups(wg_x, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async callback dropped")
            .expect("gpu buffer map failed");

        let data = slice.get_mapped_range();
        let scores: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        readback.unmap();
        scores
    }

    /// One `main_batch_topk` dispatch: upload the packed `num_q` tile queries +
    /// params, run ONE workgroup per query (each cooperatively scans the resident
    /// DB against the reused keep `mask_buffer` and selects its top-`want` on the
    /// GPU), and read back only the `num_q * want` (row, score) pairs — query-major,
    /// already sorted best-first. Readback is `num_q * want * 8` bytes (vs the
    /// distance matrix's `num_q * n * 4`), the throughput lever.
    fn dispatch_batch_topk(
        &self,
        packed_q: &[f32],
        num_q: usize,
        want: usize,
        mask_buffer: &wgpu::Buffer,
    ) -> Vec<(u32, f32)> {
        let query_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_topk_queries"),
                contents: bytemuck::cast_slice(packed_q),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let params = TopkParams {
            n: self.n as u32,
            dim: self.dim as u32,
            metric: self.metric.code(),
            num_q: num_q as u32,
            want: want as u32,
            _p0: 0,
            _p1: 0,
            _p2: 0,
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_topk_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // Output: num_q * want entries, each two u32s [score_bits, row].
        let out_u32 = num_q * want * 2;
        let out_bytes = (out_u32 * std::mem::size_of::<u32>()) as wgpu::BufferAddress;
        let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_topk_out"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_topk_readback"),
            size: out_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beam_flat_topk_bind_group"),
            layout: &self.topk_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: self.db_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 15,
                    resource: query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 16,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 17,
                    resource: out_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 18,
                    resource: mask_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beam_flat_topk_encoder"),
            });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("beam_flat_topk_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.topk_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            // One workgroup per query: workgroup `q` selects query `q`'s top-k.
            cpass.dispatch_workgroups(num_q as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async callback dropped")
            .expect("gpu buffer map failed");

        let data = slice.get_mapped_range();
        let raw: &[u32] = bytemuck::cast_slice(&data);
        // Decode [score_bits, row] pairs into (row, score).
        let pairs: Vec<(u32, f32)> = raw
            .chunks_exact(2)
            .map(|c| (c[1], f32::from_bits(c[0])))
            .collect();
        drop(data);
        readback.unmap();
        pairs
    }

    /// One `main_batch_tiled` (GEMM-tiled) dispatch: upload the packed `num_q`
    /// queries + params, run the 2D workgroup grid `(num_qtiles, num_splits)` — each
    /// workgroup stages DB tiles into shared memory and reuses them across its
    /// [`TILE_Q_GEMM`] queries — and read back the per-(query, split) partial
    /// top-`want` `(row, score)` pairs, laid out
    /// `(local_q * num_splits + split) * want + j`. A padding slot (a split with
    /// fewer than `want` live rows) carries `row == u32::MAX`.
    fn dispatch_batch_tiled(
        &self,
        packed_q: &[f32],
        num_q: usize,
        want: usize,
        num_splits: usize,
        split_len: usize,
        mask_buffer: &wgpu::Buffer,
    ) -> Vec<(u32, f32)> {
        let query_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_tiled_queries"),
                contents: bytemuck::cast_slice(packed_q),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let params = TiledParams {
            n: self.n as u32,
            dim: self.dim as u32,
            metric: self.metric.code(),
            num_q: num_q as u32,
            want: want as u32,
            num_splits: num_splits as u32,
            split_len: split_len as u32,
            _p0: 0,
        };
        let params_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("beam_tiled_params"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        // Output: num_q * num_splits * want entries, each two u32s [score_bits, row].
        let out_u32 = num_q * num_splits * want * 2;
        let out_bytes = (out_u32 * std::mem::size_of::<u32>()) as wgpu::BufferAddress;
        let out_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_tiled_out"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beam_tiled_readback"),
            size: out_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beam_flat_tiled_bind_group"),
            layout: &self.tiled_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 19,
                    resource: self.db_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 20,
                    resource: query_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 21,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 22,
                    resource: out_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 23,
                    resource: mask_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beam_flat_tiled_encoder"),
            });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("beam_flat_tiled_pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.tiled_pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            // 2D grid: x = query-tiles (TILE_Q_GEMM queries each), y = DB splits.
            let wg_x = (num_q as u32).div_ceil(TILE_Q_GEMM as u32).max(1);
            cpass.dispatch_workgroups(wg_x, num_splits as u32, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async callback dropped")
            .expect("gpu buffer map failed");

        let data = slice.get_mapped_range();
        let raw: &[u32] = bytemuck::cast_slice(&data);
        let pairs: Vec<(u32, f32)> = raw
            .chunks_exact(2)
            .map(|c| (c[1], f32::from_bits(c[0])))
            .collect();
        drop(data);
        readback.unmap();
        pairs
    }
}

/// Merge the GEMM-tiled path's disjoint per-split partial top-k candidates into
/// the query's global top-`want`, best-first, in place. `cand` holds every real
/// (row, score) from the query's `num_splits` partials (each split covers a
/// disjoint DB range, so there are no duplicate rows); this sorts them best-first
/// under `metric` (L2 ascending, Dot/Cosine descending) and truncates to `want`.
/// The candidate count is tiny (`<= num_splits * want`), so a full sort is cheap.
fn merge_split_topk(cand: &mut Vec<(u32, f32)>, metric: Metric, want: usize) {
    let larger_better = metric.larger_is_better();
    cand.sort_by(|a, b| {
        let ord = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
        if larger_better {
            ord.reverse()
        } else {
            ord
        }
    });
    cand.truncate(want);
}

/// A `COMPUTE`-visible storage-buffer bind-group-layout entry (`read_only`
/// picks read vs read_write).
fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl VectorIndex for GpuFlatIndex {
    fn search_knn(&self, query: &[f32], k: usize) -> Vec<Neighbor> {
        if query.len() != self.dim || self.n == 0 || k == 0 {
            return Vec::new();
        }
        if self.n_live == 0 {
            return Vec::new();
        }
        if self.n_live == self.n {
            // No tombstones: the original fast, unmasked scan (this is the path the
            // gpu-vs-cpu parity test exercises).
            let scores = self.compute_distances(query);
            return topk(&scores, self.metric, k, &self.external_ids);
        }
        // Tombstones present: fold the live-mask into the keep-bitmask and run the
        // SAME sentinel kernel filtered search uses, so deleted rows are excluded.
        // Cap top-k at the live count so sentinels are never selected.
        let scores = self.compute_distances_filtered(query, &self.live);
        topk(&scores, self.metric, k.min(self.n_live), &self.external_ids)
    }

    fn num_vectors(&self) -> usize {
        self.n
    }

    fn row_payload(&self, row: u32) -> &Payload {
        &self.payloads[row as usize]
    }

    /// Efficient GPU-side filtered scan: build the per-row keep bitmask on the
    /// host, hand it to the `main_filtered` kernel (which sentinels the dropped
    /// rows on the GPU), then take the top `min(k, #matching)` — capping at the
    /// match count so only matching rows are returned. This computes the exact
    /// same distances the CPU oracle does for the surviving rows, so filtered
    /// GPU top-k equals the filtered CPU oracle.
    fn search_knn_filtered(&self, query: &[f32], k: usize, filter: &Filter) -> Vec<Neighbor> {
        if query.len() != self.dim || self.n == 0 || k == 0 {
            return Vec::new();
        }
        // Host-built keep bitmask: a row survives iff it is LIVE and its payload
        // matches the filter (live AND filter) — the deleted-row live bit is just
        // one more clause folded into the keep-set. `+ match count`.
        let mut nmatch = 0usize;
        let mask: Vec<u32> = self
            .payloads
            .iter()
            .zip(&self.live)
            .map(|(p, &live)| {
                if live == 1 && filter.matches(p) {
                    nmatch += 1;
                    1
                } else {
                    0
                }
            })
            .collect();
        if nmatch == 0 {
            return Vec::new();
        }
        let scores = self.compute_distances_filtered(query, &mask);
        topk(&scores, self.metric, k.min(nmatch), &self.external_ids)
    }
}
