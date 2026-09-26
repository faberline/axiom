//! Surface-less smoke-render harness for CI integration tests.
//!
//! Why this module exists: [`crate::WebGpuRenderer::new`] requires a
//! `wgpu::Surface` and therefore a window-like target. CI workers
//! don't have one. wgpu does expose a software-fallback adapter
//! (Vulkan `llvmpipe` on Linux CI, Metal/Vulkan on developer
//! hardware), and a surface-less render to an `Rgba8UnormSrgb`
//! offscreen texture with `COPY_SRC` usage can be read back to
//! validate the cell-rect pipeline end-to-end without a window.
//!
//! This module owns just enough of that path:
//! [`request_smoke_adapter`] picks a software-preferred adapter,
//! [`HeadlessSmokeRenderer`] builds the cell-rect pipeline against
//! an offscreen target and runs single-frame
//! draw + readback. The integration test in `tests/headless_smoke.rs`
//! drives it.
//!
//! Skip semantics: `request_smoke_adapter` returning `None` is the
//! "no adapter at all reachable" signal — the integration test
//! treats it as a pass-with-message, not a failure. Only an adapter
//! present but device / pipeline / readback failure is a regression.
//!
//! Out of scope: MSAA (software adapters expose patchy support;
//! Slice 4p's toggle is unit-tested separately), DPR, scroll. A
//! later slice can extend this harness to cover them.

use crate::cell_rect::{self, CellInstance, PipelineConfig};
use crate::viewport::ViewportUniforms;
use crate::RendererError;

/// Bytes per `Rgba8UnormSrgb` pixel — used to size the readback
/// buffer and walk its rows.
const RGBA8_BYTES_PER_PIXEL: u32 = 4;

/// wgpu requires `copy_texture_to_buffer` row strides to be
/// multiples of 256 bytes. The readback unwinds that padding.
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

/// Round `value` up to the nearest multiple of `align`. `align`
/// must be non-zero.
fn align_up(value: u32, align: u32) -> u32 {
    debug_assert!(align > 0);
    value.div_ceil(align) * align
}

/// Try to acquire a wgpu adapter suitable for CI smoke testing.
///
/// Strategy:
/// 1. Request with `force_fallback_adapter: true` — software path
///    (llvmpipe on Linux CI), the AC's "software adapter" target.
/// 2. If that fails, request the default adapter — covers developer
///    hardware where no software adapter is registered but a real
///    GPU is present (still a valid CI smoke target).
/// 3. If both fail, return `None` — the integration test treats this
///    as "skip with message" rather than a regression.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/headless-adapter-integration-test-slice-4t.md#interface
/// @issue #1738
pub async fn request_smoke_adapter() -> Option<(wgpu::Instance, wgpu::Adapter)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    // Software-preferred. Falls back to hardware if no software
    // adapter is registered; falls back to None if neither path
    // surfaces an adapter at all.
    let software = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: true,
        })
        .await;
    if let Some(adapter) = software {
        return Some((instance, adapter));
    }
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await?;
    Some((instance, adapter))
}

/// Offscreen single-frame smoke renderer for the cell-rect pipeline.
///
/// Owns the device + queue + offscreen render target + cell-rect
/// pipeline. Each call to [`Self::render_single_cell`] clears the
/// target, draws one `CellInstance`, copies the texture into a
/// `MAP_READ` buffer, and returns the RGBA8 bytes in row-major
/// top-to-bottom order (256-byte row padding stripped).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/headless-adapter-integration-test-slice-4t.md#interface
/// @issue #1738
pub struct HeadlessSmokeRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    size_px: (u32, u32),
    target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    viewport_buffer: wgpu::Buffer,
    viewport_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    /// Bytes-per-row in the readback buffer (target_width × 4,
    /// rounded up to 256). Stored so the readback step can strip
    /// the padding deterministically.
    bytes_per_row: u32,
}

impl HeadlessSmokeRenderer {
    /// Build a smoke renderer against `adapter` with an `Rgba8UnormSrgb`
    /// target of `size_px`. Single-sampled — software adapters
    /// expose patchy MSAA support.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/headless-adapter-integration-test-slice-4t.md#interface
    /// @issue #1738
    pub async fn new(adapter: wgpu::Adapter, size_px: (u32, u32)) -> Result<Self, RendererError> {
        let (width, height) = (size_px.0.max(1), size_px.1.max(1));
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("headless_smoke_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| RendererError::DeviceLost(e.to_string()))?;

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let target_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless_smoke_target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Cell-rect pipeline rebuilt locally against the headless
        // target format. This intentionally duplicates the wiring
        // that WebGpuRenderer does internally — the wrapper is
        // surface-bound so cannot be reused here.
        let cell_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("headless_smoke_cell_shader"),
            source: wgpu::ShaderSource::Wgsl(cell_rect::CELL_RECT_WGSL.into()),
        });
        let viewport_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("headless_smoke_viewport_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("headless_smoke_pipeline_layout"),
            bind_group_layouts: &[&viewport_bgl],
            push_constant_ranges: &[],
        });
        let config = PipelineConfig {
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            blend: wgpu::BlendState::ALPHA_BLENDING,
            vertices_per_instance: 4,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("headless_smoke_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &cell_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            primitive: config.primitive,
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: &cell_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(config.blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless_smoke_viewport_buffer"),
            size: std::mem::size_of::<ViewportUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("headless_smoke_viewport_bg"),
            layout: &viewport_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless_smoke_instance_buffer"),
            size: std::mem::size_of::<CellInstance>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bytes_per_row = align_up(width * RGBA8_BYTES_PER_PIXEL, COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless_smoke_readback_buffer"),
            size: (bytes_per_row * height) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            size_px: (width, height),
            target_texture,
            target_view,
            pipeline,
            viewport_buffer,
            viewport_bind_group,
            instance_buffer,
            readback_buffer,
            bytes_per_row,
        })
    }

    /// Target pixel dimensions.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/headless-adapter-integration-test-slice-4t.md#interface
    /// @issue #1738
    pub fn size_px(&self) -> (u32, u32) {
        self.size_px
    }

    /// Clear to `clear` (straight RGBA), draw one `cell`, then read
    /// back the rendered texture as `Rgba8UnormSrgb` bytes in
    /// row-major top-to-bottom order. The returned vec has length
    /// `width * height * 4`; the 256-byte row padding wgpu requires
    /// for `copy_texture_to_buffer` is stripped.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/headless-adapter-integration-test-slice-4t.md#interface
    /// @issue #1738
    pub async fn render_single_cell(
        &mut self,
        cell: CellInstance,
        clear: [f32; 4],
    ) -> Result<Vec<u8>, RendererError> {
        // Push the viewport uniform (size + zero scroll).
        let uniforms = ViewportUniforms::new(self.size_px.0 as f32, self.size_px.1 as f32);
        self.queue
            .write_buffer(&self.viewport_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Push the single instance.
        self.queue
            .write_buffer(&self.instance_buffer, 0, bytemuck::bytes_of(&cell));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless_smoke_encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("headless_smoke_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear[0] as f64,
                            g: clear[1] as f64,
                            b: clear[2] as f64,
                            a: clear[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.viewport_bind_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            pass.draw(0..4, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.target_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_row),
                    rows_per_image: Some(self.size_px.1),
                },
            },
            wgpu::Extent3d {
                width: self.size_px.0,
                height: self.size_px.1,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map + wait. `Maintain::Wait` blocks the current thread
        // until the GPU finishes — fine for tests, never used on
        // the live render path.
        let slice = self.readback_buffer.slice(..);
        let (tx, rx) = futures_channel_oneshot();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.await
            .map_err(|_| RendererError::DeviceLost("readback channel dropped".into()))?
            .map_err(|e| RendererError::DeviceLost(format!("buffer map failed: {e:?}")))?;

        let data = slice.get_mapped_range();
        let (w, h) = self.size_px;
        let row_payload = (w * RGBA8_BYTES_PER_PIXEL) as usize;
        let mut out = Vec::with_capacity(row_payload * h as usize);
        for y in 0..h {
            let row_start = (y * self.bytes_per_row) as usize;
            out.extend_from_slice(&data[row_start..row_start + row_payload]);
        }
        drop(data);
        self.readback_buffer.unmap();
        Ok(out)
    }
}

/// Minimal oneshot channel — pulled inline to avoid pulling in
/// `futures` for one `map_async` await. The sender is owned by
/// wgpu's mapping callback; the receiver is awaited on the test
/// thread after `device.poll(Wait)` has run the callback.
fn futures_channel_oneshot<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
    let inner = std::sync::Arc::new(std::sync::Mutex::new(OneshotInner {
        value: None,
        waker: None,
    }));
    (
        OneshotSender {
            inner: inner.clone(),
        },
        OneshotReceiver { inner },
    )
}

struct OneshotInner<T> {
    value: Option<T>,
    waker: Option<std::task::Waker>,
}

struct OneshotSender<T> {
    inner: std::sync::Arc<std::sync::Mutex<OneshotInner<T>>>,
}

impl<T> OneshotSender<T> {
    fn send(self, value: T) -> Result<(), ()> {
        let mut guard = self.inner.lock().map_err(|_| ())?;
        guard.value = Some(value);
        if let Some(w) = guard.waker.take() {
            w.wake();
        }
        Ok(())
    }
}

struct OneshotReceiver<T> {
    inner: std::sync::Arc<std::sync::Mutex<OneshotInner<T>>>,
}

impl<T> std::future::Future for OneshotReceiver<T> {
    type Output = Result<T, OneshotRecvError>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => return std::task::Poll::Ready(Err(OneshotRecvError)),
        };
        if let Some(v) = guard.value.take() {
            std::task::Poll::Ready(Ok(v))
        } else {
            guard.waker = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}

#[derive(Debug)]
struct OneshotRecvError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_up_handles_already_aligned() {
        assert_eq!(align_up(256, 256), 256);
        assert_eq!(align_up(512, 256), 512);
    }

    #[test]
    fn align_up_rounds_up() {
        assert_eq!(align_up(1, 256), 256);
        assert_eq!(align_up(255, 256), 256);
        assert_eq!(align_up(257, 256), 512);
    }

    #[test]
    fn align_up_zero_stays_zero() {
        assert_eq!(align_up(0, 256), 0);
    }
}
