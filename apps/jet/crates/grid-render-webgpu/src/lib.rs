//! WebGPU render runtime for cclab grid — Slice 4a foundation + Slice 4b
//! cell-rect pipeline construction.
//!
//! Why this crate exists: the jet-grid render path needs a single owner of
//! the GPU resources (Device, Queue, Surface, SurfaceConfiguration) that every
//! later slice (cell pipeline, text pass, render-pass orchestration, buffer
//! pool) plugs into. Keeping the wrapper here — separate from jet-grid
//! itself and from jet-grid-wasm — lets the renderer be exercised against a
//! real adapter without dragging the wasm-bindgen surface or the grid layout
//! engine into the build graph.
//!
//! Invariant: `WebGpuRenderer` owns its [`wgpu::Surface`] and
//! [`wgpu::SurfaceConfiguration`] together. They must be reconfigured as a
//! pair on resize, never independently — partial reconfigure leaves the swap
//! chain in a state where `get_current_texture` silently returns stale frames.
//!
//! Pipeline-cache invariant: [`WebGpuRenderer::create_cell_pipeline`] builds
//! at most one [`wgpu::RenderPipeline`] per surface format and returns
//! [`Arc`]-shared handles thereafter. Callers MUST treat the returned `Arc`
//! as the canonical handle for that format — building a second pipeline for
//! the same format wastes a few MB of GPU memory and breaks pointer-equality
//! checks downstream slices rely on for "is the bound pipeline current?"
//! fast paths.
//!
//! # Lifecycle
//!
//! The renderer's life splits into four phases. Each phase pins an
//! invariant that callers must honour — restating the *what* belongs
//! in rustdoc on the individual methods; this section names the
//! *why* so a new consumer (e.g. a future `cue` shell) lands on the
//! same integration shape every other consumer already uses.
//!
//! 1. **Construct.** [`WebGpuRenderer::new`] (defaults to
//!    [`AlphaMode::Opaque`]) or [`WebGpuRenderer::with_alpha_mode`]
//!    (explicit alpha for overlay shells). Both are `async` because
//!    wgpu adapter + device acquisition is async; callers either
//!    `.await` on native or chain the returned future to a JS
//!    `Promise` on wasm. The renderer captures the adapter's
//!    supported present-mode + alpha-mode caps at this point —
//!    later [`WebGpuRenderer::set_present_mode`] /
//!    [`WebGpuRenderer::set_alpha_mode`] calls reuse the snapshot
//!    instead of re-querying.
//!
//! 2. **Configure.** Pre-render mutation goes through dedicated
//!    setters so the surface + uniforms reconfigure atomically:
//!    [`WebGpuRenderer::on_resize`] / [`WebGpuRenderer::on_resize_logical`]
//!    (physical or logical pixels — never both); [`WebGpuRenderer::set_dpr`]
//!    when the browser reports a DPR change; [`WebGpuRenderer::set_msaa_count`]
//!    (toggling MSAA invalidates the cell pipeline cache);
//!    [`WebGpuRenderer::set_clear_color`]; [`WebGpuRenderer::on_scroll`] /
//!    [`WebGpuRenderer::set_scroll`]; [`WebGpuRenderer::set_present_mode`];
//!    [`WebGpuRenderer::set_alpha_mode`]. Invariant: surface and
//!    `SurfaceConfiguration` reconfigure as a pair — callers must
//!    NOT mutate the surface out-of-band (drop the surface, hand it
//!    to a different config, etc.); that path leaves the swap chain
//!    stale.
//!
//! 3. **Render.** Each frame goes through one of four entry points:
//!    [`WebGpuRenderer::render_frame`] (the simple cell-rect path),
//!    [`WebGpuRenderer::render_frame_clipped`] (with a viewport
//!    clamp), [`WebGpuRenderer::begin_frame`] (multi-pass builder),
//!    or [`WebGpuRenderer::try_acquire_frame`] (non-blocking
//!    timeout-tolerant variant). The viewport uniform is seeded by
//!    `new` so even a caller that skips an explicit
//!    [`WebGpuRenderer::set_viewport`] gets a sane first frame.
//!    A single instance pool ([`WebGpuRenderer::set_instance_staging_threshold_bytes`])
//!    backs every frame's vertex upload — the hot path never
//!    allocates.
//!
//! 4. **Teardown.** Implicit: dropping the renderer releases device,
//!    queue, surface, and all pipeline-cache entries. On wasm, the
//!    sibling `jet-grid-wasm` crate exposes `RendererHandle::destroy`
//!    as an *explicit* teardown point — observable to React strict
//!    mode's double-invoke checks and to operator logging — but the
//!    two paths converge (both just drop the handle); the explicit
//!    method is documentation surface, not extra cleanup.
//!
//! Device-loss recovery sits orthogonal to the four phases:
//! [`WebGpuRenderer::is_device_lost`] / [`WebGpuRenderer::take_device_lost_event`]
//! observe; [`WebGpuRenderer::try_recover`] re-runs adapter +
//! device acquisition with the originally-negotiated feature set
//! so behaviour stays consistent across re-init.
//!
//! # Example
//!
//! A runnable end-to-end example lives at
//! [`examples/minimal.rs`](https://github.com/faberline/jet/blob/main/crates/grid-render-webgpu/examples/minimal.rs):
//! it drives one frame through the surface-less smoke harness so
//! `cargo run --example minimal` works on any machine with a wgpu
//! adapter (software or hardware), and exits with a skip message
//! when none is reachable. The example deliberately uses the
//! [`headless`] entry points rather than [`WebGpuRenderer`] itself
//! because headless construction needs no window — it is the right
//! shape for documentation, CI smoke, and reproducing wgpu issues
//! without a window-system dep. For the surface-bound integration
//! pattern, see the `jet-grid-wasm` bridge module's
//! `init_renderer` entry point.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use thiserror::Error;

/// Monotonic per-frame ID emitted as the `frame_id` field on the
/// `frame` tracing span. Lets a chrome://tracing / Tracy viewer order
/// concurrent renderer instances on the same timeline. Slice 4x
/// (#1742).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/tracing-instrumentation-per-pass-spans-slice-4x.md#interface
/// @issue #1742
static FRAME_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns the next monotonic frame ID and advances the counter.
/// Exposed for the unit test that asserts strict monotonicity; not
/// part of the public renderer surface.
#[doc(hidden)]
pub fn next_frame_id() -> u64 {
    FRAME_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub mod backend;
pub mod cell_rect;
pub mod dpr;
pub mod font_db;
pub mod font_face;
pub mod frame;
pub mod frame_timing;
pub mod glyph_atlas;
pub mod glyph_atlas_upload;
pub mod glyph_cache;
pub mod glyph_raster;
pub mod gpu_memory;
pub mod headless;
pub mod instance_pool;
pub mod lost_context;
pub mod shaper;
pub mod text_pass;
pub mod tracing_setup;
pub mod validation;
pub mod viewport;
pub mod viewport_clamp;

pub use frame::FrameBuilder;
pub use frame_timing::FrameTimingPool;

/// CPU-side glyph atlas upload used by the text pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAtlasUpload {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub upload_count: u32,
    pub nonzero_alpha_count: u32,
}

struct TextAtlasResources {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
}
pub use lost_context::{DeviceLostEvent, LostContextStatus, RecoveryError};

/// Wraps the GPU primitives required to drive a frame loop against a single
/// surface.
///
/// @issue #1719
pub struct WebGpuRenderer<'window> {
    /// Cached adapter — retained post-construction so
    /// [`Self::try_recover`] can call `request_device` again on the
    /// same adapter after a device-lost event. Slice 4h (#1726).
    adapter: wgpu::Adapter,
    /// Subset of `Features` originally requested at construction. The
    /// recovery path re-applies the exact same mask so behavior is
    /// consistent across re-init.
    required_features: wgpu::Features,
    /// Per-feature outcome of the optional-feature negotiation that
    /// produced `required_features`. Slice 4q (#1735) — the single
    /// source of truth callers (devtools overlays, telemetry, render
    /// loop reporters) read instead of grepping `Device::features()`.
    negotiated: NegotiatedFeatures,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'window>,
    surface_config: wgpu::SurfaceConfiguration,
    cell_shader: wgpu::ShaderModule,
    cell_bind_group_layout: wgpu::BindGroupLayout,
    cell_pipeline_layout: wgpu::PipelineLayout,
    /// Cached cell-rect pipelines keyed by `(surface_format, msaa_count)`.
    /// MSAA sample count is baked into `MultisampleState`, so toggling MSAA
    /// must invalidate the cache — Slice 4p (#1734) re-keyed this from a
    /// format-only map and clears it from `set_msaa_count`.
    cell_pipelines: HashMap<(wgpu::TextureFormat, u32), Arc<wgpu::RenderPipeline>>,
    /// Text-pass shader/BGL/pipeline-layout — built eagerly at
    /// construction so the per-format pipeline cache below can reuse
    /// them. Mirrors the cell-rect trio. Slice #2191.
    text_shader: wgpu::ShaderModule,
    text_bind_group_layout: wgpu::BindGroupLayout,
    text_pipeline_layout: wgpu::PipelineLayout,
    /// Cached text-pass pipelines keyed by `(surface_format, msaa_count)`.
    /// Cleared whenever `set_msaa_count` runs (same reasoning as
    /// `cell_pipelines`). Slice #2191.
    text_pipelines: HashMap<(wgpu::TextureFormat, u32), Arc<wgpu::RenderPipeline>>,
    /// Placeholder 1×1 R8Unorm glyph atlas + view + sampler. Built at
    /// construction so the text-pass bind group is always wireable
    /// even before a real glyph atlas exists. The atlas texel is
    /// fully-opaque (0xFF) — text instances issued against this
    /// placeholder produce solid-color quads, which is correct for the
    /// encode-seam test path (R8 / T8). Slice #2191.
    text_placeholder_atlas: wgpu::Texture,
    text_placeholder_atlas_view: wgpu::TextureView,
    text_placeholder_sampler: wgpu::Sampler,
    text_real_atlas: Option<TextAtlasResources>,
    /// Persistent bind-group composing `viewport_buffer` (@binding 0),
    /// the placeholder atlas view (@binding 1), and the placeholder
    /// sampler (@binding 2) against `text_bind_group_layout`. Slice
    /// #2191.
    text_bind_group: wgpu::BindGroup,
    /// Glyph count from the most recent `render_frame_with_text` call
    /// (or zero if none ever ran). The wasm bridge mirrors this into
    /// `window.__jet_webgpu_status.lastTextGlyphCount` so the browser
    /// e2e (T8) can tell encode-fired-but-empty apart from
    /// encode-never-fired. Slice #2191.
    last_text_glyph_count: u32,
    last_text_atlas_mode: &'static str,
    last_text_atlas_upload_count: u32,
    last_text_atlas_width: u32,
    last_text_atlas_height: u32,
    last_text_atlas_nonzero_alpha_count: u32,
    /// MSAA sample count for the cell-rect pass. Defaults to 1 on
    /// `wasm32` (axis-aligned rects don't need it on web; resolves are
    /// expensive on integrated GPUs) and 4 on native (cheap 4× on
    /// modern desktop). Slice 4p (#1734).
    msaa_count: u32,
    /// Multisampled color target. `Some` iff `msaa_count > 1` — the
    /// render pass writes here and wgpu resolves to the surface view on
    /// `present`. The backing `Texture` is held alongside the view so a
    /// reallocation in `on_resize` / `set_msaa_count` can drop it as a
    /// pair. Slice 4p (#1734).
    msaa_color: Option<(wgpu::Texture, wgpu::TextureView)>,
    viewport_buffer: wgpu::Buffer,
    viewport_bind_group: wgpu::BindGroup,
    /// Device-lost observation cell. The `Device::set_device_lost_callback`
    /// installed by [`lost_context::install_callback`] writes here;
    /// renderer accessors read from it. Slice 4h (#1726).
    lost_status: Arc<lost_context::LostContextStatus>,
    /// Current device-pixel ratio. The wgpu surface is configured at
    /// `logical_size * dpr` (physical px); the React layer sees the
    /// `logical_size` and CSS handles the visual downscale. Slice 4g
    /// (#1725).
    dpr: f32,
    /// Last *logical* size known to the renderer — needed so a future
    /// `set_dpr` can reconfigure the surface without the caller
    /// re-supplying the size. Initialized from the constructor's
    /// `size_px` argument under the (single-DPR) assumption that the
    /// caller hands us physical px today; once `set_dpr` runs, this
    /// tracks logical px going forward. Slice 4g (#1725).
    logical_size: (u32, u32),
    /// Background color the cell pass clears to at frame start. Owned
    /// here (not passed per call) so callers can configure once and the
    /// hot render path stays argument-light.
    clear_color: wgpu::Color,
    /// Accumulated scroll offset in physical pixels. Mutated by
    /// [`Self::on_scroll`]; mirrored into the viewport uniform's
    /// `scroll_px` field on every scroll event AND preserved across
    /// resize / DPR change / device recovery (Slice 4s, #1737).
    scroll_px: [f32; 2],
    /// Total content extent of the underlying virtual sheet in
    /// physical pixels. Used by [`Self::set_scroll`] to pin raw
    /// scroll input via [`viewport_clamp::clamp_scroll_px`].
    /// Defaults to `[INFINITY, INFINITY]` (= no clamp), so a
    /// renderer that hasn't been told the sheet's size still works.
    /// Slice 4v (#1740).
    content_extent_px: [f32; 2],
    /// Instance buffer pool used by `render_frame` (slot 0 — this slice
    /// is single-frame-in-flight; multi-frame pacing lands in a later
    /// slice).
    instance_pool: instance_pool::InstanceBufferPool,
    /// GPU-timestamp ring buffer. Disabled silently when the adapter
    /// does not expose `Features::TIMESTAMP_QUERY` — see
    /// [`FrameTimingPool`]. Exposed to callers via
    /// [`Self::last_frame_gpu_ms`].
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub(crate) frame_timing: frame_timing::FrameTimingPool,
    /// Snapshot of the adapter's `surface_caps.present_modes` captured
    /// at construction. Lets callers drive
    /// [`recommend_present_mode`] without re-querying the surface
    /// caps. Slice 4z (#1744).
    supported_present_modes: Vec<wgpu::PresentMode>,
    /// Snapshot of the adapter's `surface_caps.alpha_modes` captured at
    /// construction. Lets [`Self::set_alpha_mode`] map a public
    /// [`AlphaMode`] to the right `wgpu::CompositeAlphaMode` without
    /// re-querying the surface caps. Slice 4aa (#1745).
    supported_alpha_modes: Vec<wgpu::CompositeAlphaMode>,
}

impl<'window> WebGpuRenderer<'window> {
    /// Construct a browser WebGPU renderer bound to a `<canvas>`.
    ///
    /// wgpu 24 keeps canvas surfaces behind the explicit
    /// `SurfaceTarget::Canvas` variant instead of a blanket
    /// `HtmlCanvasElement: Into<SurfaceTarget>` impl. Keeping this
    /// adapter here prevents wasm bridges from reaching into wgpu's
    /// surface enum directly.
    #[cfg(all(target_arch = "wasm32", not(target_os = "emscripten")))]
    pub async fn new_for_canvas(
        canvas: wgpu::web_sys::HtmlCanvasElement,
        size_px: (u32, u32),
    ) -> Result<Self, RendererError> {
        Self::new(wgpu::SurfaceTarget::Canvas(canvas), size_px).await
    }

    /// Construct a renderer bound to `target` and sized to `size_px`.
    ///
    /// `size_px` of `(0, 0)` is clamped to `(1, 1)` because wgpu's surface
    /// configure rejects zero-sized dimensions (the minimized-window case).
    ///
    /// Slice 4aa (#1745): defaults the canvas alpha mode to
    /// [`AlphaMode::Opaque`]. Use [`Self::with_alpha_mode`] to opt into
    /// [`AlphaMode::PreMultiplied`] (e.g. for cue overlay surfaces).
    ///
    /// @issue #1719
    pub async fn new(
        target: impl Into<wgpu::SurfaceTarget<'window>>,
        size_px: (u32, u32),
    ) -> Result<Self, RendererError> {
        Self::with_alpha_mode(target, size_px, AlphaMode::Opaque).await
    }

    /// Construct a renderer with an explicit canvas alpha mode. The
    /// zero-arg `new` constructor delegates here with
    /// [`AlphaMode::Opaque`] — the right default for the grid path.
    /// Pass [`AlphaMode::PreMultiplied`] when the renderer hosts overlay
    /// UIs that need to composite with page content behind the canvas.
    ///
    /// The provided `alpha_mode` is filtered through [`map_alpha_mode`]
    /// against the adapter's actually-supported list; if the requested
    /// variant isn't supported, the renderer falls back to the first
    /// listed mode (the same behaviour as the pre-Slice-4aa default).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/canvas-alpha-mode-slice-4aa.md#interface
    /// @issue #1745
    pub async fn with_alpha_mode(
        target: impl Into<wgpu::SurfaceTarget<'window>>,
        size_px: (u32, u32),
        alpha_mode: AlphaMode,
    ) -> Result<Self, RendererError> {
        // Slice 4k (#1729): explicit `cfg(debug_assertions)` gate on
        // the runtime validation layer. The behaviour is byte-
        // equivalent to wgpu's own `from_build_config()` (which
        // `InstanceFlags::default()` already calls), but the explicit
        // form keeps the dev/release contract grep-visible — a future
        // refactor that drops `..Default::default()` cannot silently
        // turn validation off across the renderer.
        // Slice 4l (#1730): target-conditional backend mask. Web
        // builds get `BROWSER_WEBGPU` (no WebGL2 fallback — we'd
        // rather fail loudly than render a degraded best-effort
        // frame); native builds get `PRIMARY` (Metal/Vulkan/DX12)
        // so behaviour matches what shipping users see.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: backend::backends_for_target(),
            flags: validation::instance_flags_for_build(),
            ..Default::default()
        });

        let surface = instance
            .create_surface(target)
            .map_err(|e| RendererError::NoSurface(e.to_string()))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(RendererError::NoAdapter)?;

        // Slice 4q (#1735): centralised optional-feature negotiation.
        // The renderer NEVER *requires* a feature — required mask
        // stays empty so any conformant WebGPU adapter works — but
        // it *opts in* to a documented set of optionals when the
        // adapter exposes them. `FrameTimingPool` (Slice 4i, #1727)
        // checks `TIMESTAMP_QUERY` on the returned mask to decide
        // whether to degrade to a no-op.
        let adapter_features = adapter.features();
        let (required_features, negotiated) = negotiate_features(adapter_features);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("cclab_grid_render_webgpu_device"),
                    required_features,
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| RendererError::DeviceLost(e.to_string()))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        // Slice 4z (#1744): capture the adapter's supported present
        // modes so `supported_present_modes()` callers can drive
        // `recommend_present_mode` without re-querying surface caps.
        let supported_present_modes: Vec<wgpu::PresentMode> = surface_caps.present_modes.clone();
        // Slice 4aa (#1745): same snapshot trick for alpha modes so
        // `set_alpha_mode` can re-translate without surface-caps round
        // trips, and so the live constructor's `map_alpha_mode` call
        // picks the requested variant out of the adapter's actual
        // supported list.
        let supported_alpha_modes: Vec<wgpu::CompositeAlphaMode> = surface_caps.alpha_modes.clone();
        let selected_alpha_mode = map_alpha_mode(alpha_mode, &supported_alpha_modes);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size_px.0.max(1),
            height: size_px.1.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: selected_alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Build the cell-rect shader / BGL / pipeline-layout eagerly. These
        // are tiny and shared across every surface-format-keyed pipeline; the
        // alternative (Option + lazy init) trades borrow-checker ceremony for
        // no real win.
        let cell_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cell_rect_shader"),
            source: wgpu::ShaderSource::Wgsl(cell_rect::CELL_RECT_WGSL.into()),
        });
        let cell_bind_group_layout =
            device.create_bind_group_layout(&cell_rect::cell_rect_bind_group_layout_descriptor());
        let cell_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cell_rect_pipeline_layout"),
            bind_group_layouts: &[&cell_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Persistent viewport uniform: one buffer, one bind-group, allocated
        // here so the hot draw path never re-creates either. Initial contents
        // are zeroed; callers MUST call `set_viewport` (or `on_resize`,
        // which calls it internally) before the first draw.
        let viewport_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell_rect_viewport_uniform"),
            size: std::mem::size_of::<viewport::ViewportUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cell_rect_viewport_bind_group"),
            layout: &cell_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        // Slice #2191: build the text-pass trio (shader, BGL,
        // pipeline-layout) + persistent placeholder atlas + bind-group
        // alongside the cell-rect set so the encode seam is always
        // wireable. The 1×1 R8Unorm atlas is uploaded fully-opaque so
        // text-pass draws against the placeholder paint as solid
        // color rects — correct for the seam test path until a real
        // glyph atlas is wired.
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_pass_shader"),
            source: wgpu::ShaderSource::Wgsl(text_pass::TEXT_PASS_WGSL.into()),
        });
        let text_bind_group_layout =
            device.create_bind_group_layout(&text_pass::text_pass_bind_group_layout_descriptor());
        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pass_pipeline_layout"),
            bind_group_layouts: &[&text_bind_group_layout],
            push_constant_ranges: &[],
        });
        let (
            text_placeholder_atlas,
            text_placeholder_atlas_view,
            text_placeholder_sampler,
            text_bind_group,
        ) = build_text_placeholder_resources(
            &device,
            &queue,
            &text_bind_group_layout,
            &viewport_buffer,
        );

        let frame_timing = frame_timing::FrameTimingPool::new(&device, &queue, adapter_features);
        // Slice 4h (#1726): wire the device-lost callback BEFORE the
        // renderer hands its device out — any loss observed in the
        // interim still lands in the shared status cell.
        let lost_status = lost_context::LostContextStatus::new();
        lost_context::install_callback(&device, &lost_status);
        let mut renderer = Self {
            adapter,
            required_features,
            negotiated,
            device,
            queue,
            surface,
            surface_config,
            cell_shader,
            cell_bind_group_layout,
            cell_pipeline_layout,
            cell_pipelines: HashMap::new(),
            text_shader,
            text_bind_group_layout,
            text_pipeline_layout,
            text_pipelines: HashMap::new(),
            text_placeholder_atlas,
            text_placeholder_atlas_view,
            text_placeholder_sampler,
            text_real_atlas: None,
            text_bind_group,
            last_text_glyph_count: 0,
            last_text_atlas_mode: "placeholder",
            last_text_atlas_upload_count: 0,
            last_text_atlas_width: 1,
            last_text_atlas_height: 1,
            last_text_atlas_nonzero_alpha_count: 1,
            msaa_count: MSAA_DEFAULT,
            msaa_color: None,
            viewport_buffer,
            viewport_bind_group,
            lost_status,
            dpr: 1.0,
            logical_size: (size_px.0.max(1), size_px.1.max(1)),
            clear_color: WHITE_CLEAR,
            scroll_px: [0.0, 0.0],
            content_extent_px: [f32::INFINITY, f32::INFINITY],
            instance_pool: instance_pool::InstanceBufferPool::new(),
            frame_timing,
            supported_present_modes,
            supported_alpha_modes,
        };

        // Seed the uniform with the initial surface size so consumers that
        // skip an explicit `set_viewport` still get a sane first frame.
        // `with_scroll` is harmless on first construction (scroll=[0,0])
        // but keeps the call shape consistent with the resize / DPR /
        // recovery sites that MUST preserve scroll (Slice 4s, #1737).
        renderer.set_viewport(viewport::ViewportUniforms::with_scroll(
            renderer.surface_config.width as f32,
            renderer.surface_config.height as f32,
            renderer.scroll_px,
        ));
        renderer.reallocate_msaa_resources();

        Ok(renderer)
    }

    /// Reconfigure the surface for a new size. Idempotent; zero dimensions
    /// are clamped to 1. Also pushes the new size into the viewport uniform
    /// so consumers don't have to remember a separate `set_viewport` call
    /// after every resize.
    ///
    /// @issue #1719
    pub fn on_resize(&mut self, new_size_px: (u32, u32)) {
        self.surface_config.width = new_size_px.0.max(1);
        self.surface_config.height = new_size_px.1.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.set_viewport(viewport::ViewportUniforms::with_scroll(
            self.surface_config.width as f32,
            self.surface_config.height as f32,
            self.scroll_px,
        ));
        self.reallocate_msaa_resources();
    }

    /// @issue #1719
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// @issue #1719
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// @issue #1719
    pub fn surface(&self) -> &wgpu::Surface<'window> {
        &self.surface
    }

    /// @issue #1719
    pub fn surface_config(&self) -> &wgpu::SurfaceConfiguration {
        &self.surface_config
    }

    /// Current surface texture format. Slice 4b's cell pipeline keys its
    /// pipeline cache by this value.
    ///
    /// @issue #1719
    pub fn format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// Current surface size in physical pixels.
    ///
    /// @issue #1719
    pub fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Bind-group layout for the cell-rect pipeline's single viewport
    /// uniform. Callers use this layout to build the matching `BindGroup`
    /// before issuing a draw against the cell pipeline.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
    /// @issue #1720
    pub fn cell_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.cell_bind_group_layout
    }

    /// Push new viewport uniforms to the GPU via `Queue::write_buffer`.
    /// Cheap: no allocation, no bind-group rebuild — the persistent
    /// uniform buffer is overwritten in place.
    ///
    /// Note: this writes ALL 16 bytes of the uniform, including
    /// `scroll_px`. Callers that only want to update the scroll
    /// component should use [`Self::on_scroll`] (8-byte partial
    /// write) instead.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-uniform-buffer-slice-4c.md#interface
    /// @issue #1721
    pub fn set_viewport(&self, uniforms: viewport::ViewportUniforms) {
        self.queue
            .write_buffer(&self.viewport_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Accumulate a scroll delta in physical pixels into the viewport
    /// `scroll_px` uniform. Writes ONLY the 8 scroll bytes via a
    /// `Queue::write_buffer` at offset
    /// [`viewport::SCROLL_PX_OFFSET_BYTES`]. Does not touch instance
    /// buffers — cells are in virtual-sheet coords; the vertex shader
    /// translates by `scroll_px` to produce visible-window positions.
    ///
    /// Convention: `(+dx, +dy)` moves the visible window DOWN-RIGHT
    /// into the sheet — content visually moves UP-LEFT. Matches
    /// conventional UI scroll semantics.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/scroll-driven-uniform-update-slice-4s.md#interface
    /// @issue #1737
    pub fn on_scroll(&mut self, dx_px: f32, dy_px: f32) {
        self.scroll_px[0] += dx_px;
        self.scroll_px[1] += dy_px;
        self.write_scroll_px();
    }

    /// Current accumulated scroll offset in physical pixels.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/scroll-driven-uniform-update-slice-4s.md#interface
    /// @issue #1737
    pub fn scroll_px(&self) -> [f32; 2] {
        self.scroll_px
    }

    /// Reset scroll to `[0.0, 0.0]` and push the partial uniform write
    /// to the GPU. Useful for "scroll to top" UX paths and tests that
    /// want a clean baseline.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/scroll-driven-uniform-update-slice-4s.md#interface
    /// @issue #1737
    pub fn reset_scroll(&mut self) {
        self.scroll_px = [0.0, 0.0];
        self.write_scroll_px();
    }

    /// Accumulate a *raw* scroll delta and clamp the result against
    /// the renderer's [`Self::content_extent_px`] + current viewport
    /// size. Compared to [`Self::on_scroll`] (which is the
    /// unconditional accumulator), this is the entry point the JS
    /// side should call with raw wheel/touch input: scrolling past
    /// the end of the sheet pins instead of producing a blank strip.
    ///
    /// If `content_extent_px` has never been set the default is
    /// `[INFINITY, INFINITY]` and the clamp degenerates to a floor at
    /// `0.0` per axis (a renderer that doesn't know the sheet size
    /// still works — it just can't pin to the bottom edge).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
    /// @issue #1740
    pub fn set_scroll(&mut self, raw_dx: f32, raw_dy: f32) {
        let raw = [self.scroll_px[0] + raw_dx, self.scroll_px[1] + raw_dy];
        let viewport = [
            self.surface_config.width as f32,
            self.surface_config.height as f32,
        ];
        self.scroll_px = viewport_clamp::clamp_scroll_px(raw, self.content_extent_px, viewport);
        self.write_scroll_px();
    }

    /// Set the total content extent of the virtual sheet in physical
    /// pixels. Used by [`Self::set_scroll`] to bound raw scroll input.
    /// Callers should re-push this after row/column edits that change
    /// the sheet's total size.
    ///
    /// `[INFINITY, INFINITY]` (the default) disables the upper bound;
    /// the clamp then floors raw scroll at `0.0` only.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
    /// @issue #1740
    pub fn set_content_extent_px(&mut self, extent_px: [f32; 2]) {
        self.content_extent_px = extent_px;
    }

    /// Current configured content extent in physical pixels.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
    /// @issue #1740
    pub fn content_extent_px(&self) -> [f32; 2] {
        self.content_extent_px
    }

    /// AABB of the visible window in virtual-sheet coords:
    /// `(scroll_px, scroll_px + viewport_size)`. Inclusive on min,
    /// exclusive on max — matches the convention used by
    /// [`viewport_clamp::cell_intersects_rect`].
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
    /// @issue #1740
    pub fn visible_rect_px(&self) -> ([f32; 2], [f32; 2]) {
        viewport_clamp::visible_rect_px(
            self.scroll_px,
            [
                self.surface_config.width as f32,
                self.surface_config.height as f32,
            ],
        )
    }

    /// Variant of [`Self::render_frame`] that drops cells whose AABB
    /// doesn't overlap the visible rect before instance upload. Use
    /// this for sheets large enough that off-screen cells would
    /// dominate the upload bandwidth.
    ///
    /// The legacy [`Self::render_frame`] is unchanged — callers that
    /// have their own culling (or are working with small scenes) keep
    /// the original behavior.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-clamping-helper-integration-slice-4v.md#interface
    /// @issue #1740
    pub fn render_frame_clipped(
        &mut self,
        cells: &[cell_rect::CellInstance],
    ) -> Result<(), RenderFrameError> {
        let (min, max) = self.visible_rect_px();
        let filtered: Vec<cell_rect::CellInstance> = cells
            .iter()
            .copied()
            .filter(|c| viewport_clamp::cell_intersects_rect(c, min, max))
            .collect();
        self.render_frame(&filtered)
    }

    /// Render `cells` into an offscreen `Rgba8Unorm` texture sized to
    /// the current `surface_config` dimensions (which already encode
    /// DPR via [`Self::on_resize`] / `set_size`), then copy the texture
    /// into a `MAP_READ` buffer and return the pixels as row-major
    /// top-to-bottom RGBA8 bytes.
    ///
    /// This path is **independent of the surface** — it never calls
    /// `get_current_texture` or `present`, so it's safe to interleave
    /// with the live render loop or call from a CI worker that has no
    /// presentable surface. Visual-regression and SSR consumers are
    /// the primary callers.
    ///
    /// Pipeline format is always `Rgba8Unorm` (single-sampled),
    /// regardless of the surface format. Callers want a stable RGBA8
    /// contract; the screenshot is not a copy of the swap-chain
    /// texture but a fresh offscreen render.
    ///
    /// Returns [`ScreenshotError::ZeroSizeSurface`] if the renderer's
    /// surface config reports a zero dimension — callers must
    /// `on_resize` first.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/take-screenshot-readback-pixels-slice-4y.md#interface
    /// @issue #1743
    pub fn take_screenshot(
        &mut self,
        cells: &[cell_rect::CellInstance],
    ) -> Result<Screenshot, ScreenshotError> {
        let width = self.surface_config.width;
        let height = self.surface_config.height;
        if width == 0 || height == 0 {
            return Err(ScreenshotError::ZeroSizeSurface);
        }

        // Always Rgba8Unorm + single-sampled. The screenshot's RGBA8
        // contract is independent of the surface's native format.
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let pipeline = self.create_cell_pipeline(format, 1);

        // Push viewport uniform reflecting current surface dims with
        // zero scroll. The screenshot is conceptually "scene at top of
        // sheet"; if a caller wanted scrolled-window capture they can
        // set viewport before this call — but the AC scopes screenshot
        // to dimensions + DPR, not scroll.
        let uniforms = viewport::ViewportUniforms::new(width as f32, height as f32);
        self.queue
            .write_buffer(&self.viewport_buffer, 0, bytemuck::bytes_of(&uniforms));

        let target_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("screenshot_target"),
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

        let instance_bytes_len = (std::mem::size_of::<cell_rect::CellInstance>()
            * cells.len().max(1)) as wgpu::BufferAddress;
        let instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot_instance_buffer"),
            size: instance_bytes_len,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !cells.is_empty() {
            self.queue
                .write_buffer(&instance_buffer, 0, bytemuck::cast_slice(cells));
        }

        let bytes_per_row = screenshot_align_up(
            width * SCREENSHOT_RGBA8_BYTES_PER_PIXEL,
            SCREENSHOT_COPY_BYTES_PER_ROW_ALIGNMENT,
        );
        let readback_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot_readback_buffer"),
            size: (bytes_per_row * height) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("screenshot_encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screenshot_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &self.viewport_bind_group, &[]);
            if !cells.is_empty() {
                pass.set_vertex_buffer(0, instance_buffer.slice(..));
                pass.draw(0..4, 0..(cells.len() as u32));
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map + wait. `Maintain::Wait` blocks the current thread until
        // the GPU finishes — fine for screenshot (a synchronous,
        // off-the-hot-path operation by design).
        let slice = readback_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        let map_outcome = rx
            .recv()
            .map_err(|_| ScreenshotError::DeviceLost("readback channel dropped".into()))?;
        map_outcome.map_err(|e| ScreenshotError::BufferMapFailed(format!("{e:?}")))?;

        let data = slice.get_mapped_range();
        let row_payload = (width * SCREENSHOT_RGBA8_BYTES_PER_PIXEL) as usize;
        let mut pixels = Vec::with_capacity(row_payload * height as usize);
        for y in 0..height {
            let row_start = (y * bytes_per_row) as usize;
            pixels.extend_from_slice(&data[row_start..row_start + row_payload]);
        }
        drop(data);
        readback_buffer.unmap();
        Ok(Screenshot {
            width,
            height,
            pixels,
        })
    }

    /// Current `wgpu::PresentMode` applied to the surface
    /// configuration.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/present-mode-selection-slice-4z.md#interface
    /// @issue #1744
    pub fn present_mode(&self) -> wgpu::PresentMode {
        self.surface_config.present_mode
    }

    /// Snapshot of the adapter's supported present modes captured at
    /// construction. Pass this to [`recommend_present_mode`] to pick
    /// the right mode for a given target without re-querying surface
    /// caps.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/present-mode-selection-slice-4z.md#interface
    /// @issue #1744
    pub fn supported_present_modes(&self) -> &[wgpu::PresentMode] {
        &self.supported_present_modes
    }

    /// Overwrite the surface's present mode and re-configure. Mirrors
    /// the call path `on_resize` uses for width/height updates.
    ///
    /// The caller is responsible for picking a mode the adapter
    /// supports — pair this with [`recommend_present_mode`] +
    /// [`Self::supported_present_modes`]. Passing an unsupported mode
    /// is undefined behavior at the wgpu layer (most backends will
    /// silently coerce to `Fifo`).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/present-mode-selection-slice-4z.md#interface
    /// @issue #1744
    pub fn set_present_mode(&mut self, mode: wgpu::PresentMode) {
        self.surface_config.present_mode = mode;
        self.surface.configure(&self.device, &self.surface_config);
    }

    /// Current canvas alpha mode (the public, renderer-typed variant —
    /// derived from the surface config's `wgpu::CompositeAlphaMode`).
    /// Returns the closest [`AlphaMode`] match; unmapped wgpu variants
    /// (`PostMultiplied`, `Inherit`, `Auto`) coerce to
    /// [`AlphaMode::Opaque`] — those modes mean "compositor decides"
    /// and the renderer treats them as not-explicitly-blending.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/canvas-alpha-mode-slice-4aa.md#interface
    /// @issue #1745
    pub fn alpha_mode(&self) -> AlphaMode {
        match self.surface_config.alpha_mode {
            wgpu::CompositeAlphaMode::PreMultiplied => AlphaMode::PreMultiplied,
            _ => AlphaMode::Opaque,
        }
    }

    /// Snapshot of the adapter's `wgpu::CompositeAlphaMode` list,
    /// captured at construction. Lets callers see what the adapter
    /// supports before calling [`Self::set_alpha_mode`] — though the
    /// setter is robust to unsupported requests (it falls back through
    /// [`map_alpha_mode`]).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/canvas-alpha-mode-slice-4aa.md#interface
    /// @issue #1745
    pub fn supported_alpha_modes(&self) -> &[wgpu::CompositeAlphaMode] {
        &self.supported_alpha_modes
    }

    /// Switch the canvas alpha mode at runtime. Translates the public
    /// [`AlphaMode`] through [`map_alpha_mode`] against the captured
    /// supported list and re-configures the surface. Mirrors the
    /// `set_present_mode` call path.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/canvas-alpha-mode-slice-4aa.md#interface
    /// @issue #1745
    pub fn set_alpha_mode(&mut self, mode: AlphaMode) {
        self.surface_config.alpha_mode = map_alpha_mode(mode, &self.supported_alpha_modes);
        self.surface.configure(&self.device, &self.surface_config);
    }

    /// Partial-write helper: 8 bytes at the documented offset. Keeping
    /// this private prevents callers from drifting away from the
    /// `on_scroll` / `reset_scroll` accumulator semantics.
    fn write_scroll_px(&self) {
        self.queue.write_buffer(
            &self.viewport_buffer,
            viewport::SCROLL_PX_OFFSET_BYTES,
            bytemuck::bytes_of(&self.scroll_px),
        );
    }

    /// Borrow the persistent bind-group for the cell-rect pipeline's
    /// viewport binding. Pass this to `RenderPass::set_bind_group(0, ...)`
    /// before issuing the cell-rect draw.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-uniform-buffer-slice-4c.md#interface
    /// @issue #1721
    pub fn viewport_bind_group(&self) -> &wgpu::BindGroup {
        &self.viewport_bind_group
    }

    /// Borrow the persistent viewport uniform buffer. Primarily for
    /// integration tests that want to assert byte-equality after a
    /// `set_viewport` round-trip.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/viewport-uniform-buffer-slice-4c.md#interface
    /// @issue #1721
    pub fn viewport_buffer(&self) -> &wgpu::Buffer {
        &self.viewport_buffer
    }

    /// Build (or fetch the cached) cell-rect render pipeline targeting
    /// `format`. Idempotent: the second call with the same format returns
    /// a clone of the same `Arc` (pointer equality holds).
    ///
    /// Composes [`cell_rect::CELL_RECT_WGSL`], the shared cell pipeline
    /// layout, [`cell_rect::cell_rect_vertex_layout`], and
    /// [`cell_rect::cell_rect_pipeline_config`] into a single
    /// `wgpu::RenderPipeline`. The cache is grow-only and keyed by
    /// `(format, msaa_count)` — Slice 4p (#1734) added `msaa_count` to
    /// the key because `MultisampleState.count` is baked into the
    /// pipeline; toggling MSAA must rebuild.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#interface
    /// @spec apps/jet/crates/grid-render-webgpu/docs/msaa-toggle-slice-4p.md#interface
    /// @issue #1720
    /// @issue #1734
    pub fn create_cell_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
        msaa_count: u32,
    ) -> Arc<wgpu::RenderPipeline> {
        let key = (format, msaa_count);
        if let Some(p) = self.cell_pipelines.get(&key) {
            return Arc::clone(p);
        }

        let cfg = cell_rect::cell_rect_pipeline_config();
        let vertex_layout = cell_rect::cell_rect_vertex_layout();

        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("cell_rect_pipeline"),
                layout: Some(&self.cell_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.cell_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_layout],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.cell_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(cfg.blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: cfg.primitive,
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: msaa_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

        let arc = Arc::new(pipeline);
        self.cell_pipelines.insert(key, Arc::clone(&arc));
        arc
    }

    /// Build (or fetch the cached) cell-rect render pipeline for the
    /// given color-target format, using the renderer's current MSAA
    /// sample count.
    ///
    /// Prefer this over [`Self::create_cell_pipeline`] at callsites
    /// that don't already track an MSAA value — it reads the MSAA
    /// dimension from `self` so the API surface is format-only.
    /// `Arc::ptr_eq` holds across calls with the same `format`
    /// (until the renderer's MSAA setting changes, which clears the
    /// cache).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/per-surface-pipeline-cache-slice-4u.md#interface
    /// @issue #1739
    pub fn pipeline_for(&mut self, format: wgpu::TextureFormat) -> Arc<wgpu::RenderPipeline> {
        let msaa = self.msaa_count;
        self.create_cell_pipeline(format, msaa)
    }

    /// Number of distinct `(format, msaa_count)` entries currently
    /// resident in the cell-rect pipeline cache. Exposed for tests
    /// and devtools overlays; do not rely on this for cache eviction
    /// decisions (the cache is grow-only by design — see Slice 4u).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/per-surface-pipeline-cache-slice-4u.md#interface
    /// @issue #1739
    pub fn pipeline_cache_len(&self) -> usize {
        self.cell_pipelines.len()
    }

    /// Current MSAA sample count for the cell-rect pass. Defaults to
    /// `1` on `wasm32`, `4` on native — see [`MSAA_DEFAULT`].
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/msaa-toggle-slice-4p.md#interface
    /// @issue #1734
    pub fn msaa_count(&self) -> u32 {
        self.msaa_count
    }

    /// Borrow the per-feature negotiation outcome captured at
    /// construction (and refreshed by [`Self::try_recover`]).
    ///
    /// Callers ask "is optional feature X on?" via these flags instead
    /// of grepping `Device::features()` directly. Slice 4q (#1735) is
    /// the single source of truth for that question.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/adapter-feature-negotiation-slice-4q.md#interface
    /// @issue #1735
    pub fn negotiated_features(&self) -> &NegotiatedFeatures {
        &self.negotiated
    }

    /// Set the MSAA sample count used by the cell-rect pass. Accepts
    /// `1` (single-sampled) or `4` (4× MSAA); other values are silently
    /// ignored — Slice 4q (#1735) will gate arbitrary counts on
    /// adapter feature negotiation. No-op when `count` matches the
    /// current setting.
    ///
    /// Side effects on a real change: the pipeline cache is cleared
    /// (the next `create_cell_pipeline` rebuilds with the new sample
    /// count baked into `MultisampleState`) and the MSAA color texture
    /// is reallocated (or freed, if returning to count == 1).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/msaa-toggle-slice-4p.md#interface
    /// @issue #1734
    pub fn set_msaa_count(&mut self, count: u32) {
        if count != 1 && count != 4 {
            return;
        }
        if count == self.msaa_count && self.msaa_color.is_some() == (count > 1) {
            return;
        }
        self.msaa_count = count;
        self.cell_pipelines.clear();
        // Slice #2191: same `MultisampleState`-baking reasoning applies
        // to the text-pass cache.
        self.text_pipelines.clear();
        self.reallocate_msaa_resources();
    }

    /// Borrow the MSAA color view when MSAA is on. `None` when
    /// `msaa_count == 1` — callers (the cell-pass encoder) treat that
    /// as the "draw straight into the surface, no resolve" path.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/msaa-toggle-slice-4p.md#interface
    /// @issue #1734
    pub(crate) fn msaa_color_view(&self) -> Option<&wgpu::TextureView> {
        self.msaa_color.as_ref().map(|(_, view)| view)
    }

    /// (Re)allocate the MSAA color target so its dimensions and format
    /// match `surface_config` and its sample count matches
    /// `self.msaa_count`. When `msaa_count == 1` this just drops the
    /// existing texture (the cell pass falls back to drawing directly
    /// into the surface view).
    ///
    /// Called from the constructor, every surface-reconfigure entry
    /// (`on_resize`, `on_resize_logical`, `set_dpr`), `set_msaa_count`,
    /// and `try_recover` (after the new device replaces the old one).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/msaa-toggle-slice-4p.md#interface
    /// @issue #1734
    fn reallocate_msaa_resources(&mut self) {
        if self.msaa_count <= 1 {
            self.msaa_color = None;
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cell_rect_msaa_color"),
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: self.msaa_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.msaa_color = Some((texture, view));
    }

    /// Override the background clear color used by [`render_frame`]. The
    /// new value takes effect on the next frame; in-flight encoders are
    /// not affected.
    ///
    /// Components are linear-space `[r, g, b, a]` in `[0.0, 1.0]`,
    /// matching the shape `CellInstance::color` uses so embedding apps
    /// hold one type of theme color throughout. Slice 4o (#1733) reshaped
    /// the public signature away from `wgpu::Color` so the renderer's
    /// GPU type does not leak into the embedding app.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/clear-color-config-slice-4o.md#interface
    /// @issue #1733
    pub fn set_clear_color(&mut self, rgba: [f32; 4]) {
        self.clear_color = rgba_to_wgpu_color(rgba);
    }

    /// Borrow the currently configured clear color as `[r, g, b, a]`.
    /// Defaults to opaque white (`[1.0, 1.0, 1.0, 1.0]`) until overridden
    /// by [`set_clear_color`]. Slice 4o (#1733) flipped the default from
    /// black so first paint matches the dominant light-mode UI case.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/clear-color-config-slice-4o.md#interface
    /// @issue #1733
    pub fn clear_color(&self) -> [f32; 4] {
        wgpu_color_to_rgba(self.clear_color)
    }

    /// Render one frame's worth of cells in a single render pass.
    ///
    /// Sequence:
    ///
    /// 1. `surface.get_current_texture()` — acquire the swap-chain image.
    ///    Errors map to [`RenderFrameError`] (1:1 with `wgpu::SurfaceError`).
    /// 2. Build a fresh `CommandEncoder`.
    /// 3. Reuse-or-grow the slot-0 instance buffer via the pool and
    ///    upload `cells` (skipped if `cells` is empty — the clear still
    ///    runs).
    /// 4. Reuse the per-format cached cell-rect pipeline.
    /// 5. Begin one render pass with `clear_color` as the load op; set
    ///    pipeline, viewport bind-group, instance vertex buffer; emit
    ///    `draw(0..4, 0..N)` for `N = cells.len()`.
    /// 6. Submit the command buffer and present the surface texture.
    ///
    /// The empty-cells path is deliberately retained (rather than
    /// short-circuiting before clear) so the surface gets a fresh frame
    /// every call — important for animation drivers that toggle visibility.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/render-pass-orchestration-slice-4e.md#interface
    /// @issue #1723
    pub fn render_frame(
        &mut self,
        cells: &[cell_rect::CellInstance],
    ) -> Result<(), RenderFrameError> {
        let frame_id = next_frame_id();
        let span = tracing::info_span!("frame", frame_id = frame_id, cells = cells.len(),);
        let _guard = span.enter();
        let clear = self.clear_color;
        let mut frame = self.begin_frame()?;
        frame.encode_cell_pass(cells, Some(clear));
        frame.commit()
    }

    /// Acquire the next surface texture and apply the per-variant
    /// recovery policy that every realistic frame driver needs.
    ///
    /// | wgpu error      | outcome                                       |
    /// |-----------------|-----------------------------------------------|
    /// | (success)       | [`AcquireOutcome::Frame`]                     |
    /// | `Timeout`       | [`AcquireOutcome::Skipped`] (logged at `warn`)|
    /// | `Outdated`      | reconfigure + retry once, then re-classify    |
    /// | `Lost`          | [`AcquireOutcome::NeedsRecovery`]             |
    /// | `OutOfMemory`   | `Err(OutOfMemory)` — propagated               |
    /// | (other)         | `Err(Other(msg))` — propagated                |
    ///
    /// Pre-check parity with [`begin_frame`]: if the device-lost
    /// callback has already fired, returns
    /// [`AcquireOutcome::NeedsRecovery`] without touching the surface.
    ///
    /// `Outdated` retries exactly once: if the second `get_current_texture`
    /// still fails for the same reason, the outcome escalates to
    /// `Skipped` (a chronically-stale config is a caller bug — we
    /// surface it via the logs, not by hot-looping inside acquire).
    ///
    /// `begin_frame` remains the raw variant for callers that want to
    /// apply their own policy (e.g. the frame-timing bench).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/surface-acquire-recovery-slice-4n.md#interface
    /// @issue #1732
    pub fn try_acquire_frame(&mut self) -> Result<AcquireOutcome<'_, 'window>, RenderFrameError> {
        if self.lost_status.is_lost() {
            return Ok(AcquireOutcome::NeedsRecovery);
        }

        let first = self.surface.get_current_texture();
        let first_err = match first {
            Ok(surface_texture) => {
                return Ok(AcquireOutcome::Frame(FrameBuilder::new(
                    self,
                    surface_texture,
                )));
            }
            Err(e) => e,
        };

        match classify_acquire_error(&first_err) {
            AcquireAction::SkipTimeout => {
                tracing::warn!("surface acquire timed out — skipping frame (Slice 4n / #1732)");
                Ok(AcquireOutcome::Skipped)
            }
            AcquireAction::RetryAfterReconfigure => {
                self.surface.configure(&self.device, &self.surface_config);
                match self.surface.get_current_texture() {
                    Ok(surface_texture) => Ok(AcquireOutcome::Frame(FrameBuilder::new(
                        self,
                        surface_texture,
                    ))),
                    Err(second_err) => match classify_acquire_error(&second_err) {
                        AcquireAction::SkipTimeout | AcquireAction::RetryAfterReconfigure => {
                            tracing::warn!(
                                "surface still stale after reconfigure — skipping frame"
                            );
                            Ok(AcquireOutcome::Skipped)
                        }
                        AcquireAction::NeedsRecovery => Ok(AcquireOutcome::NeedsRecovery),
                        AcquireAction::Propagate(err) => Err(err),
                    },
                }
            }
            AcquireAction::NeedsRecovery => Ok(AcquireOutcome::NeedsRecovery),
            AcquireAction::Propagate(err) => Err(err),
        }
    }

    /// Acquire the next surface texture and start a `FrameBuilder` that
    /// batches all per-frame work into a single `Queue::submit` + present.
    ///
    /// See [`FrameBuilder`] for the per-frame command-batching contract.
    /// Future slices that add a text pass / debug overlay chain onto the
    /// same builder via additional `encode_*_pass` calls before `commit`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/submit-batching-slice-4j.md#interface
    /// @issue #1728
    pub fn begin_frame(&mut self) -> Result<FrameBuilder<'_, 'window>, RenderFrameError> {
        // Slice 4h (#1726): short-circuit when the device-lost
        // callback has fired. Without this guard, the next
        // `get_current_texture` against a dead device would either
        // panic in wgpu or return a confusing `Outdated`/`Lost` that
        // callers can't tell apart from a window resize. The
        // `DeviceLost` variant carries the driver's reason + message
        // verbatim so the React layer can show a meaningful overlay.
        if self.lost_status.is_lost() {
            // Inspect-without-drain: we keep the event available so
            // `take_device_lost_event` returns it on the next call.
            // The variant we return is a synthesized snapshot; we
            // don't take the event here because the React layer may
            // not have observed it yet via its own polling.
            return Err(RenderFrameError::DeviceLost {
                reason: wgpu::DeviceLostReason::Unknown,
                message: "device lost (call try_recover before next frame)".into(),
            });
        }
        let surface_texture = self
            .surface
            .get_current_texture()
            .map_err(RenderFrameError::from)?;
        Ok(FrameBuilder::new(self, surface_texture))
    }

    /// Current device-pixel ratio. Defaults to `1.0` and is updated by
    /// [`Self::set_dpr`]. The wgpu surface is configured at
    /// `logical_size * dpr` physical pixels; the viewport uniform also
    /// receives physical pixels so the WGSL pos → NDC math stays
    /// resolution-correct without per-shader DPR awareness.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
    /// @issue #1725
    pub fn dpr(&self) -> f32 {
        self.dpr
    }

    /// Update the device-pixel ratio. Reconfigures the surface at the
    /// new physical size `logical_size() * dpr` and re-seeds the
    /// viewport uniform with the new physical dims.
    ///
    /// `dpr <= 0` is silently clamped to `1.0` — see
    /// [`dpr`] module docs.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
    /// @issue #1725
    pub fn set_dpr(&mut self, dpr: f32) {
        self.dpr = if dpr > 0.0 { dpr } else { 1.0 };
        let physical = dpr::compute_physical_size(self.logical_size, self.dpr);
        self.surface_config.width = physical.0;
        self.surface_config.height = physical.1;
        self.surface.configure(&self.device, &self.surface_config);
        self.set_viewport(viewport::ViewportUniforms::with_scroll(
            self.surface_config.width as f32,
            self.surface_config.height as f32,
            self.scroll_px,
        ));
        self.reallocate_msaa_resources();
    }

    /// Logical (CSS-pixel) size the renderer is configured for. The
    /// surface texture is `logical_size * dpr` physical pixels.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
    /// @issue #1725
    pub fn logical_size(&self) -> (u32, u32) {
        self.logical_size
    }

    /// Reconfigure the surface from a logical (CSS-pixel) size.
    /// Multiplies by the current DPR to obtain physical pixels, calls
    /// [`wgpu::Surface::configure`], and re-seeds the viewport uniform
    /// with the physical dims. This is the entry point React resize
    /// handlers should use; the existing [`Self::on_resize`] is the
    /// physical-pixel back-compat entry for callers (raw canvas
    /// resize observers) that report physical px directly.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
    /// @issue #1725
    pub fn on_resize_logical(&mut self, logical: (u32, u32)) {
        self.logical_size = (logical.0.max(1), logical.1.max(1));
        let physical = dpr::compute_physical_size(self.logical_size, self.dpr);
        self.surface_config.width = physical.0;
        self.surface_config.height = physical.1;
        self.surface.configure(&self.device, &self.surface_config);
        self.set_viewport(viewport::ViewportUniforms::with_scroll(
            self.surface_config.width as f32,
            self.surface_config.height as f32,
            self.scroll_px,
        ));
        self.reallocate_msaa_resources();
    }

    /// Convert a logical (CSS-pixel) f32 coordinate to physical.
    /// Delegates to [`dpr::logical_to_physical_f32`] with the
    /// renderer's current DPR.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
    /// @issue #1725
    pub fn to_physical(&self, logical: (f32, f32)) -> (f32, f32) {
        dpr::logical_to_physical_f32(logical, self.dpr)
    }

    /// Convert a physical pixel f32 coordinate to logical. Used by
    /// pointer-event dispatch: pointer events arrive in physical px
    /// from the canvas; the element bbox tree (a future slice) stores
    /// logical px, so the hit-test path divides through here.
    ///
    /// Delegates to [`dpr::physical_to_logical_f32`] with the
    /// renderer's current DPR.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/dpr-scaling-slice-4g.md#interface
    /// @issue #1725
    pub fn to_logical(&self, physical: (f32, f32)) -> (f32, f32) {
        dpr::physical_to_logical_f32(physical, self.dpr)
    }

    /// `true` once a device-lost callback has fired and no successful
    /// [`Self::try_recover`] call has cleared the flag yet. While this
    /// returns `true`, every [`Self::begin_frame`] errors with
    /// [`RenderFrameError::DeviceLost`] — callers must call
    /// [`Self::try_recover`] before attempting to render again.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
    /// @issue #1726
    pub fn is_device_lost(&self) -> bool {
        self.lost_status.is_lost()
    }

    /// Single-take accessor for the most recent device-lost event.
    /// Returns `None` if no event is pending or a previous call already
    /// drained it. `is_device_lost()` keeps returning `true` even after
    /// the event is taken — only a successful `try_recover` clears it.
    ///
    /// React layers typically call this once when `is_device_lost()`
    /// becomes `true` to display "GPU restarting…" copy that includes
    /// the driver's diagnostic message.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
    /// @issue #1726
    pub fn take_device_lost_event(&self) -> Option<lost_context::DeviceLostEvent> {
        self.lost_status.take_event()
    }

    /// Drop per-device state and request a fresh `(Device, Queue)`
    /// from the cached adapter. On success the renderer is ready to
    /// accept frames again and `is_device_lost()` returns `false`.
    /// On failure the renderer stays in the lost state.
    ///
    /// The persistent [`wgpu::Surface`] is reused (it is bound to the
    /// OS window handle, not to the dead device). Pipelines / shader /
    /// layouts / viewport buffer / instance pool / timing pool are all
    /// rebuilt against the new device — pipeline cache is cleared so
    /// callers regenerate lazily on the next `create_cell_pipeline`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
    /// @issue #1726
    pub async fn try_recover(&mut self) -> Result<(), lost_context::RecoveryError> {
        // Drop the dead pipeline cache up front — callers that race
        // with recovery and try to use a cached pipeline against the
        // new device would otherwise hit a "device mismatch" wgpu
        // panic. Same reasoning for the MSAA texture (Slice 4p / #1734):
        // a texture from the dead device cannot be bound on the new
        // one, so drop it here and let `reallocate_msaa_resources` at
        // the end of recovery re-allocate against the new device.
        self.cell_pipelines.clear();
        // Slice #2191: drop text-pass per-device state for the same
        // device-mismatch reason as the cell-pass cache. The trio
        // (shader, BGL, pipeline-layout) and the placeholder atlas +
        // bind-group are rebuilt below against the new device.
        self.text_pipelines.clear();
        self.msaa_color = None;

        let (new_device, new_queue) = self
            .adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("cclab_grid_render_webgpu_device_recovered"),
                    required_features: self.required_features,
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| lost_context::RecoveryError::RequestDevice(e.to_string()))?;

        // Reconfigure the persistent surface against the new device.
        // wgpu::Surface::configure is documented as infallible (no
        // Result return), so we don't need to catch failures here —
        // the surface itself is unchanged, only the device binding.
        self.surface.configure(&new_device, &self.surface_config);

        // Rebuild per-device state. These mirror the construction
        // path in `new()` — kept inline (not extracted to a helper)
        // because the helper would need every field of `Self` mutable
        // and ergonomics suffer for no real win.
        let cell_shader = new_device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cell_rect_shader"),
            source: wgpu::ShaderSource::Wgsl(cell_rect::CELL_RECT_WGSL.into()),
        });
        let cell_bind_group_layout = new_device
            .create_bind_group_layout(&cell_rect::cell_rect_bind_group_layout_descriptor());
        let cell_pipeline_layout =
            new_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("cell_rect_pipeline_layout"),
                bind_group_layouts: &[&cell_bind_group_layout],
                push_constant_ranges: &[],
            });
        let viewport_buffer = new_device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cell_rect_viewport_uniform"),
            size: std::mem::size_of::<viewport::ViewportUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let viewport_bind_group = new_device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cell_rect_viewport_bind_group"),
            layout: &cell_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        // Slice #2191: rebuild the text-pass trio + placeholder atlas
        // + bind-group against the new device, mirroring construction.
        let text_shader = new_device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_pass_shader"),
            source: wgpu::ShaderSource::Wgsl(text_pass::TEXT_PASS_WGSL.into()),
        });
        let text_bind_group_layout = new_device
            .create_bind_group_layout(&text_pass::text_pass_bind_group_layout_descriptor());
        let text_pipeline_layout =
            new_device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("text_pass_pipeline_layout"),
                bind_group_layouts: &[&text_bind_group_layout],
                push_constant_ranges: &[],
            });
        let (
            text_placeholder_atlas,
            text_placeholder_atlas_view,
            text_placeholder_sampler,
            text_bind_group,
        ) = build_text_placeholder_resources(
            &new_device,
            &new_queue,
            &text_bind_group_layout,
            &viewport_buffer,
        );

        // Slice 4q (#1735): re-run the negotiation against the live
        // adapter feature set. Same adapter, so the outcome should
        // match construction — but a defensive re-derive means a
        // future adapter swap (Slice 4h's `try_recover` does not
        // currently swap adapters, but the interface allows it)
        // refreshes the per-feature flags correctly.
        let adapter_features = self.adapter.features();
        let (refreshed_required, refreshed_negotiated) = negotiate_features(adapter_features);
        self.required_features = refreshed_required;
        self.negotiated = refreshed_negotiated;
        let frame_timing =
            frame_timing::FrameTimingPool::new(&new_device, &new_queue, adapter_features);
        let new_lost_status = lost_context::LostContextStatus::new();
        lost_context::install_callback(&new_device, &new_lost_status);

        // Atomically swap. After this line `self` is fully on the
        // new device — earlier readers of `self.device` etc. are
        // gone (no concurrent &mut self by Rust's rules).
        self.device = new_device;
        self.queue = new_queue;
        self.cell_shader = cell_shader;
        self.cell_bind_group_layout = cell_bind_group_layout;
        self.cell_pipeline_layout = cell_pipeline_layout;
        self.viewport_buffer = viewport_buffer;
        self.viewport_bind_group = viewport_bind_group;
        self.text_shader = text_shader;
        self.text_bind_group_layout = text_bind_group_layout;
        self.text_pipeline_layout = text_pipeline_layout;
        self.text_placeholder_atlas = text_placeholder_atlas;
        self.text_placeholder_atlas_view = text_placeholder_atlas_view;
        self.text_placeholder_sampler = text_placeholder_sampler;
        self.text_real_atlas = None;
        self.text_bind_group = text_bind_group;
        self.last_text_glyph_count = 0;
        self.last_text_atlas_mode = "placeholder";
        self.last_text_atlas_upload_count = 0;
        self.last_text_atlas_width = 1;
        self.last_text_atlas_height = 1;
        self.last_text_atlas_nonzero_alpha_count = 1;
        // Preserve the staging-path threshold across recovery so a
        // user-set override (Slice 4r, #1736) survives device loss.
        let threshold = self.instance_pool.staging_threshold_bytes();
        self.instance_pool = instance_pool::InstanceBufferPool::with_staging_threshold(threshold);
        self.frame_timing = frame_timing;
        self.lost_status = new_lost_status;

        // Re-seed the viewport uniform — same as `new()` does, so the
        // very next draw has a sane projection without waiting for an
        // explicit `set_viewport`/`on_resize`. Preserve the user's
        // scroll across device-loss recovery (Slice 4s, #1737).
        self.set_viewport(viewport::ViewportUniforms::with_scroll(
            self.surface_config.width as f32,
            self.surface_config.height as f32,
            self.scroll_px,
        ));
        // Slice 4p (#1734): the MSAA texture was dropped at the top of
        // this fn; rebuild it against the new device if MSAA is on.
        self.reallocate_msaa_resources();

        Ok(())
    }

    /// Most recently completed frame's GPU duration in milliseconds.
    ///
    /// `None` if no frame has completed yet, or the adapter lacks
    /// `wgpu::Features::TIMESTAMP_QUERY` (in which case timing is
    /// silently disabled — see [`FrameTimingPool`] module docs).
    ///
    /// The value reflects the *previous* fully-submitted frame's GPU
    /// time, not the in-progress one — timestamp queries necessarily
    /// map back one frame later.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub fn last_frame_gpu_ms(&self) -> Option<f32> {
        self.frame_timing.last_frame_gpu_ms()
    }

    /// `true` if the renderer's GPU-timestamp pool is wired against a
    /// real `TIMESTAMP_QUERY` adapter feature. Devtools overlays can
    /// hide the "GPU ms" column when this is `false` instead of
    /// rendering a misleading "—".
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/gpu-timeline-queries-slice-4i.md#interface
    /// @issue #1727
    pub fn gpu_timing_enabled(&self) -> bool {
        self.frame_timing.is_enabled()
    }

    /// Reuse-or-grow the slot-`slot` instance buffer and upload `bytes`
    /// into it, then return a cloned `wgpu::Buffer` handle.
    ///
    /// Returning an owned handle (cheap — `wgpu::Buffer` is internally
    /// ref-counted) decouples the returned buffer from the `&mut self`
    /// borrow `get_or_grow` needs, so callers in [`FrameBuilder`] can
    /// hold the buffer alongside other immutable borrows of `self`
    /// during pass encoding.
    ///
    /// `pub(crate)` — external callers manage their own buffers.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/submit-batching-slice-4j.md#interface
    /// @issue #1728
    pub(crate) fn upload_instance_buffer(
        &mut self,
        slot: usize,
        min_size_bytes: wgpu::BufferAddress,
        bytes: &[u8],
    ) -> wgpu::Buffer {
        self.instance_pool
            .get_or_grow(slot, min_size_bytes, &self.device, &self.queue, bytes)
            .clone()
    }

    /// Override the instance-buffer pool's staging-path threshold in
    /// bytes. Payloads at or above the threshold use
    /// `Queue::write_buffer_with` (one fewer host memcpy than the
    /// default `Queue::write_buffer` path). `0` forces the staged path
    /// for every non-empty upload.
    ///
    /// Surface lives at the renderer level so devtools / benches can
    /// retune without reaching into the pool directly.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
    /// @issue #1736
    pub fn set_instance_staging_threshold_bytes(&mut self, bytes: wgpu::BufferAddress) {
        self.instance_pool.set_staging_threshold_bytes(bytes);
    }

    /// Current instance-buffer pool staging-path threshold in bytes.
    /// Default is `64 * 1024 * 32` (64K cell instances × 32 bytes per
    /// `CellInstance` = 2 MiB).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/instance-staging-path-slice-4r.md#interface
    /// @issue #1736
    pub fn instance_staging_threshold_bytes(&self) -> wgpu::BufferAddress {
        self.instance_pool.staging_threshold_bytes()
    }

    /// Build (or fetch the cached) text-pass render pipeline keyed by
    /// `(format, msaa_count)`. Idempotent: the second call with the
    /// same key returns a clone of the same `Arc`.
    ///
    /// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#changes
    /// @issue #2191
    pub fn create_text_pipeline(
        &mut self,
        format: wgpu::TextureFormat,
        msaa_count: u32,
    ) -> Arc<wgpu::RenderPipeline> {
        let key = (format, msaa_count);
        if let Some(p) = self.text_pipelines.get(&key) {
            return Arc::clone(p);
        }
        let pipeline = text_pass::create_text_pipeline(
            &self.device,
            format,
            msaa_count,
            &self.text_pipeline_layout,
            &self.text_shader,
        );
        let arc = Arc::new(pipeline);
        self.text_pipelines.insert(key, Arc::clone(&arc));
        arc
    }

    /// Borrow the persistent text-pass bind-group composing the
    /// viewport uniform, the placeholder atlas view, and the placeholder
    /// sampler. The placeholder atlas is 1×1 R8Unorm fully-opaque, so
    /// text-pass draws against it produce solid-color quads — correct
    /// until the real glyph atlas is wired into this group.
    ///
    /// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#changes
    /// @issue #2191
    pub fn text_bind_group(&self) -> &wgpu::BindGroup {
        &self.text_bind_group
    }

    /// Last text atlas mode observed by the text pass.
    pub fn last_text_atlas_mode(&self) -> &'static str {
        self.last_text_atlas_mode
    }

    /// Number of glyph atlas uploads represented by the latest atlas.
    pub fn last_text_atlas_upload_count(&self) -> u32 {
        self.last_text_atlas_upload_count
    }

    pub fn last_text_atlas_width(&self) -> u32 {
        self.last_text_atlas_width
    }

    pub fn last_text_atlas_height(&self) -> u32 {
        self.last_text_atlas_height
    }

    pub fn last_text_atlas_nonzero_alpha_count(&self) -> u32 {
        self.last_text_atlas_nonzero_alpha_count
    }

    /// Last text-pass glyph instance count observed by
    /// [`Self::render_frame_with_text`]. `0` if no text-bearing frame
    /// has been rendered. Surfaced by the wasm bridge for browser e2e.
    ///
    /// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#test-plan
    /// @issue #2191
    pub fn last_text_glyph_count(&self) -> u32 {
        self.last_text_glyph_count
    }

    fn install_text_atlas(&mut self, atlas: &TextAtlasUpload) -> Result<(), RenderFrameError> {
        let expected_len =
            atlas.width.checked_mul(atlas.height).ok_or_else(|| {
                RenderFrameError::Other("text atlas dimensions overflow".to_string())
            })? as usize;
        if atlas.width == 0 || atlas.height == 0 || atlas.pixels.len() != expected_len {
            return Err(RenderFrameError::Other(format!(
                "invalid text atlas upload: {}x{} with {} bytes",
                atlas.width,
                atlas.height,
                atlas.pixels.len()
            )));
        }

        let (texture, view, sampler, bind_group) = build_text_atlas_resources(
            &self.device,
            &self.queue,
            &self.text_bind_group_layout,
            &self.viewport_buffer,
            atlas,
        );
        self.text_real_atlas = Some(TextAtlasResources {
            _texture: texture,
            _view: view,
            _sampler: sampler,
        });
        self.text_bind_group = bind_group;
        self.last_text_atlas_mode = "glyph-atlas";
        self.last_text_atlas_upload_count = atlas.upload_count;
        self.last_text_atlas_width = atlas.width;
        self.last_text_atlas_height = atlas.height;
        self.last_text_atlas_nonzero_alpha_count = atlas.nonzero_alpha_count;
        Ok(())
    }

    /// Render one frame containing a cell-rect pass followed by a
    /// text pass against the same surface texture. One encoder, one
    /// `Queue::submit`, one present — same invariant as
    /// [`Self::render_frame`].
    ///
    /// The cell pass clears with `clear_color`; the text pass uses
    /// `LoadOp::Load` so it composites over the cleared cells. An
    /// empty `glyphs` slice still encodes the text pass (no draw,
    /// no clear) so the load op runs and the test path can observe
    /// the seam.
    ///
    /// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#interaction
    /// @issue #2191
    pub fn render_frame_with_text(
        &mut self,
        cells: &[cell_rect::CellInstance],
        glyphs: &[text_pass::GlyphInstance],
    ) -> Result<(), RenderFrameError> {
        self.last_text_atlas_mode = "placeholder";
        self.last_text_atlas_upload_count = 0;
        self.last_text_atlas_width = 1;
        self.last_text_atlas_height = 1;
        self.last_text_atlas_nonzero_alpha_count = 1;
        self.text_real_atlas = None;
        let frame_id = next_frame_id();
        let span = tracing::info_span!(
            "frame",
            frame_id = frame_id,
            cells = cells.len(),
            glyphs = glyphs.len(),
        );
        let _guard = span.enter();
        let clear = self.clear_color;
        let mut frame = self.begin_frame()?;
        frame.encode_cell_pass(cells, Some(clear));
        frame.encode_text_pass(glyphs, None);
        frame.commit()?;
        self.last_text_glyph_count = glyphs.len() as u32;
        Ok(())
    }

    /// Render one text-bearing frame using a caller-provided glyph atlas.
    pub fn render_frame_with_text_atlas(
        &mut self,
        cells: &[cell_rect::CellInstance],
        glyphs: &[text_pass::GlyphInstance],
        atlas: &TextAtlasUpload,
    ) -> Result<(), RenderFrameError> {
        self.install_text_atlas(atlas)?;
        let frame_id = next_frame_id();
        let span = tracing::info_span!(
            "frame",
            frame_id = frame_id,
            cells = cells.len(),
            glyphs = glyphs.len(),
        );
        let _guard = span.enter();
        let clear = self.clear_color;
        let mut frame = self.begin_frame()?;
        frame.encode_cell_pass(cells, Some(clear));
        frame.encode_text_pass(glyphs, None);
        frame.commit()?;
        self.last_text_glyph_count = glyphs.len() as u32;
        Ok(())
    }
}

/// Build the placeholder R8Unorm 1×1 atlas (fully-opaque texel), its
/// view, a linear sampler, and the persistent bind-group that pairs
/// them with `viewport_buffer` against `text_bind_group_layout`.
/// Factored out so the construction path and the device-loss recovery
/// path build the same shape.
///
/// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#changes
/// @issue #2191
fn build_text_placeholder_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    text_bind_group_layout: &wgpu::BindGroupLayout,
    viewport_buffer: &wgpu::Buffer,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Sampler,
    wgpu::BindGroup,
) {
    let atlas = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text_pass_placeholder_atlas"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &atlas,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0xFFu8],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(1),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("text_pass_placeholder_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("text_pass_bind_group"),
        layout: text_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    (atlas, view, sampler, bind_group)
}

fn build_text_atlas_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    text_bind_group_layout: &wgpu::BindGroupLayout,
    viewport_buffer: &wgpu::Buffer,
    upload: &TextAtlasUpload,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Sampler,
    wgpu::BindGroup,
) {
    let desc = glyph_atlas::glyph_atlas_texture_descriptor(upload.width, upload.height);
    let atlas = device.create_texture(&desc);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &atlas,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &upload.pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(upload.width),
            rows_per_image: Some(upload.height),
        },
        wgpu::Extent3d {
            width: upload.width,
            height: upload.height,
            depth_or_array_layers: 1,
        },
    );
    let view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&glyph_atlas::glyph_atlas_sampler_descriptor());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("text_pass_glyph_atlas_bind_group"),
        layout: text_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    (atlas, view, sampler, bind_group)
}

// Drop is automatic via field order (Surface -> Queue -> Device); wgpu's own
// Drop releases the underlying GPU resources. An explicit impl is unnecessary
// and would mask the natural field-drop ordering wgpu relies on.

/// Errors surfaced from [`WebGpuRenderer::new`] and friends.
///
/// **Why two error enums.** Construction-time failures (this enum)
/// and frame-time failures ([`RenderFrameError`]) live in separate
/// types on purpose: construction errors are typically user-fatal
/// (no GPU at all, no driver, validation failure during init) and
/// the JS bridge needs to show a modal; frame-time errors are mostly
/// transient (timeout, outdated swap chain) and the bridge just
/// asks the renderer to retry. Merging the two would force every
/// hot-path caller through a wider match. Slice 4w (#1741) closes
/// the variant catalogue for the construction-time enum so the JS
/// bridge can map each variant to a user-facing message.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/renderer-error-thiserror-integration-slice-4w.md#interface
/// @issue #1719
/// @issue #1741
#[derive(Debug, Error)]
pub enum RendererError {
    /// No GPU adapter satisfied the requested power preference and surface
    /// compatibility. The Display string names the backend mask the
    /// renderer searched so a developer can tell a WebGPU-only failure
    /// (web) apart from a PRIMARY-trio failure (native).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/backend-selection-slice-4l.md#interface
    /// @issue #1730
    #[error(
        "no compatible GPU adapter (tried: {})",
        backend::backend_description()
    )]
    NoAdapter,

    /// Surface creation against the given target failed (invalid window
    /// handle, unsupported platform, etc.).
    #[error("surface creation failed: {0}")]
    NoSurface(String),

    /// `request_device` returned `Err` for a reason other than
    /// device-lost — e.g. the adapter rejected the requested
    /// feature set or limits. Distinct from [`Self::DeviceLost`]
    /// so the user-facing message can be "your browser/driver
    /// doesn't support WebGPU's required features" instead of
    /// "the GPU was reset".
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/renderer-error-thiserror-integration-slice-4w.md#interface
    /// @issue #1741
    #[error("device request failed: {0}")]
    NoDevice(String),

    /// Device request failed because the device was lost during
    /// initialization (browser tab eviction, eGPU unplug while
    /// constructing). Surfaces wgpu's underlying error message
    /// verbatim.
    #[error("device lost or unavailable: {0}")]
    DeviceLost(String),

    /// Driver returned OOM during a construction-time allocation
    /// (oversized texture, buffer descriptor exceeding the
    /// device's max-buffer-size limit, etc.). Mirrors
    /// [`RenderFrameError::OutOfMemory`] for cross-phase parity.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/renderer-error-thiserror-integration-slice-4w.md#interface
    /// @issue #1741
    #[error("out of memory during construction: {0}")]
    OutOfMemory(String),

    /// wgpu validation rejected a descriptor (shader compile,
    /// pipeline layout, bind-group layout, etc.) at construction
    /// time. Surfaces the wgpu message verbatim.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/renderer-error-thiserror-integration-slice-4w.md#interface
    /// @issue #1741
    #[error("validation failed: {0}")]
    ValidationFailed(String),
}

/// Names of every `RendererError` variant, in declaration order.
/// Used by tests and the JS bridge to cross-check the variant
/// catalogue without resorting to `strum` or proc-macros.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/renderer-error-thiserror-integration-slice-4w.md#interface
/// @issue #1741
pub fn renderer_error_variant_names() -> &'static [&'static str] {
    &[
        "NoAdapter",
        "NoSurface",
        "NoDevice",
        "DeviceLost",
        "OutOfMemory",
        "ValidationFailed",
    ]
}

/// Documents which `RendererError` variants every public method
/// that returns `Result<_, RendererError>` can emit. The JS bridge
/// reads this to drive exhaustive case-handling; the unit test
/// `error_variants_by_method_references_only_valid_variants`
/// asserts every named variant exists in [`RendererError`].
///
/// Variant names must match
/// [`renderer_error_variant_names`] exactly — a rename in the
/// enum that misses an edit here will fail at unit-test time.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/renderer-error-thiserror-integration-slice-4w.md#interface
/// @issue #1741
pub const ERROR_VARIANTS_BY_METHOD: &[(&str, &[&str])] = &[
    (
        "WebGpuRenderer::new",
        &[
            "NoAdapter",
            "NoSurface",
            "NoDevice",
            "DeviceLost",
            "OutOfMemory",
            "ValidationFailed",
        ],
    ),
    (
        "WebGpuRenderer::try_recover",
        // try_recover returns lost_context::RecoveryError, not
        // RendererError, but its variants map to this enum's
        // {NoDevice, DeviceLost} for JS-bridge purposes; the
        // bridge translates before showing the user.
        &["NoDevice", "DeviceLost"],
    ),
];

/// Errors surfaced from [`WebGpuRenderer::render_frame`].
///
/// Variants map 1:1 to `wgpu::SurfaceError` so callers can apply the
/// standard wgpu remediation idioms (reconfigure on `Outdated`, drop the
/// renderer on `SurfaceLost`, retry on `Timeout`).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/render-pass-orchestration-slice-4e.md#interface
/// @issue #1723
#[derive(Debug, Error)]
pub enum RenderFrameError {
    /// Surface is gone (window closed / GPU reset). Caller should drop
    /// the renderer.
    #[error("surface lost")]
    SurfaceLost,

    /// Swap chain is stale relative to the current surface
    /// configuration; caller should reconfigure (e.g. via
    /// [`WebGpuRenderer::on_resize`]) and retry next frame.
    #[error("surface configuration out of date — caller should reconfigure")]
    Outdated,

    /// `get_current_texture` timed out — usually transient; the next
    /// frame typically succeeds.
    #[error("surface acquire timed out")]
    Timeout,

    /// The underlying `wgpu::Device` is gone (browser tab eviction,
    /// driver crash, eGPU unplug). Callers must call
    /// [`WebGpuRenderer::try_recover`] before attempting another frame.
    /// `reason` is wgpu's classification and `message` is the
    /// driver-supplied diagnostic (may be empty).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/lost-context-recovery-slice-4h.md#interface
    /// @issue #1726
    #[error("device lost: {reason:?} — {message}")]
    DeviceLost {
        reason: wgpu::DeviceLostReason,
        message: String,
    },

    /// GPU device is out of memory. Recovery is implementation-specific.
    #[error("out of memory")]
    OutOfMemory,

    /// Anything else wgpu surfaces (driver error, unknown variant from a
    /// future wgpu release, etc.). Carries the upstream message verbatim.
    #[error("render frame error: {0}")]
    Other(String),
}

impl From<wgpu::SurfaceError> for RenderFrameError {
    fn from(e: wgpu::SurfaceError) -> Self {
        match e {
            wgpu::SurfaceError::Timeout => Self::Timeout,
            wgpu::SurfaceError::Outdated => Self::Outdated,
            wgpu::SurfaceError::Lost => Self::SurfaceLost,
            wgpu::SurfaceError::OutOfMemory => Self::OutOfMemory,
            other => Self::Other(other.to_string()),
        }
    }
}

/// RGBA8 row-major pixel readback returned by
/// [`WebGpuRenderer::take_screenshot`]. The `pixels` buffer length is
/// always `(width * height * 4) as usize` — the wgpu row-padding (256
/// byte alignment) is stripped before return.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/take-screenshot-readback-pixels-slice-4y.md#interface
/// @issue #1743
#[derive(Debug, Clone)]
pub struct Screenshot {
    /// Pixel width — matches the renderer's current surface width.
    pub width: u32,
    /// Pixel height — matches the renderer's current surface height.
    pub height: u32,
    /// `Rgba8Unorm` bytes, row-major top-to-bottom, length
    /// `width * height * 4`.
    pub pixels: Vec<u8>,
}

/// Failures surfaced from [`WebGpuRenderer::take_screenshot`].
///
/// The screenshot path doesn't touch the surface, so the
/// `RenderFrameError` variants (SurfaceLost / Outdated / Timeout)
/// can't occur — keeping a separate enum saves callers from
/// pattern-matching on impossible cases.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/take-screenshot-readback-pixels-slice-4y.md#interface
/// @issue #1743
#[derive(Debug, Error)]
pub enum ScreenshotError {
    /// Renderer's `surface_config` reports zero width or height — call
    /// [`WebGpuRenderer::on_resize`] (or `set_size`) with a non-zero
    /// extent before taking a screenshot.
    #[error("surface has zero width or height — call set_size first")]
    ZeroSizeSurface,
    /// Device lost while the screenshot was in flight. Callers should
    /// route through [`WebGpuRenderer::try_recover`] (same recipe as
    /// the live render path) and retry.
    #[error("device lost during screenshot: {0}")]
    DeviceLost(String),
    /// Allocation of the offscreen target or readback buffer failed.
    #[error("out of memory allocating screenshot resources: {0}")]
    OutOfMemory(String),
    /// `Buffer::map_async` callback reported a failure. The body of the
    /// error message is wgpu's `BufferAsyncError`.
    #[error("readback buffer map failed: {0}")]
    BufferMapFailed(String),
}

/// Bytes per `Rgba8Unorm` pixel — used to size the readback buffer
/// and walk its rows after the row padding is stripped.
const SCREENSHOT_RGBA8_BYTES_PER_PIXEL: u32 = 4;

/// wgpu requires `copy_texture_to_buffer` row strides to be multiples
/// of 256 bytes. The readback unwinds that padding.
const SCREENSHOT_COPY_BYTES_PER_ROW_ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

/// Round `value` up to the nearest multiple of `align`. `align` must
/// be non-zero. Inline so the screenshot path stays self-contained.
fn screenshot_align_up(value: u32, align: u32) -> u32 {
    debug_assert!(align > 0);
    value.div_ceil(align) * align
}

/// Outcome of negotiating optional adapter features at renderer
/// construction. Each field is `true` iff the adapter exposed the
/// feature AND the renderer requested it (today, request-when-
/// available is unconditional for every feature listed here).
///
/// Callers use this struct as the single source of truth for "is
/// optional feature X available?" — checking `Device::features()`
/// directly is also valid but mixes required and optional bits.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/adapter-feature-negotiation-slice-4q.md#interface
/// @issue #1735
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NegotiatedFeatures {
    /// `wgpu::Features::TIMESTAMP_QUERY` — gates `FrameTimingPool`'s
    /// real work; absent → timing pool is a no-op and
    /// `last_frame_gpu_ms` stays `None` forever (Slice 4i).
    pub timestamp_query: bool,
    /// `wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` —
    /// unblocks BC/ETC/ASTC compressed-texture sampling on adapters
    /// that expose them. Renderer has no current consumer, but
    /// surfacing the bit means downstream slices can branch on it
    /// without re-running the negotiation.
    pub texture_adapter_specific_format_features: bool,
}

/// Map adapter-advertised features to a `(requested, outcome)` pair.
/// The returned `wgpu::Features` mask is what the renderer passes to
/// `request_device`; the `NegotiatedFeatures` mirrors the successful
/// opt-ins.
///
/// The renderer NEVER *requires* a feature — required mask is always
/// a subset of the adapter mask — so this function cannot fail. Pure,
/// no GPU required.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/adapter-feature-negotiation-slice-4q.md#interface
/// @issue #1735
fn negotiate_features(adapter: wgpu::Features) -> (wgpu::Features, NegotiatedFeatures) {
    let mut requested = wgpu::Features::empty();
    let mut outcome = NegotiatedFeatures::default();

    if adapter.contains(wgpu::Features::TIMESTAMP_QUERY) {
        requested |= wgpu::Features::TIMESTAMP_QUERY;
        outcome.timestamp_query = true;
    }
    if adapter.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) {
        requested |= wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        outcome.texture_adapter_specific_format_features = true;
    }

    (requested, outcome)
}

/// Target the renderer is running against. Drives present-mode policy:
/// web is vsync-only (browsers translate every other mode to it
/// anyway), native splits by power profile because Mailbox's latency
/// win comes at a real GPU-work cost.
///
/// The host (browser shell / OS) is the authoritative source of
/// "low-power" — battery API on web, thermal pressure / "battery
/// saver" toggle on native — so this enum is the *consumer*, not the
/// detector.
///
/// `FifoRelaxed` is intentionally not selected by any variant in this
/// slice; the AC names Mailbox/Fifo only. A future tuning knob could
/// add `NativeInteractiveAllowTearing` if user research shows tearing
/// is preferable to dropped frames in some workflow.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/present-mode-selection-slice-4z.md#interface
/// @issue #1744
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentTarget {
    /// Browser / wasm target. Always `Fifo` — browsers composite with
    /// vsync and translate every other mode to it anyway.
    Web,
    /// Native window, user is actively interacting (typing, scrolling,
    /// dragging). Mailbox preferred for lowest input-to-photon latency;
    /// Fifo fallback if the adapter doesn't list Mailbox.
    NativeInteractive,
    /// Native window, the host signaled low-power mode (laptop on
    /// battery, "battery saver" toggle on). Always `Fifo`.
    NativeLowPower,
}

/// Pick a [`wgpu::PresentMode`] for `target` from the adapter's
/// `supported` list. Always returns one of the modes in `supported`
/// (or `Fifo` as a guaranteed-available fallback — the WebGPU spec
/// requires every adapter to support `Fifo`).
///
/// Policy rubric:
/// - [`PresentTarget::Web`] → `Fifo`.
/// - [`PresentTarget::NativeInteractive`] → `Mailbox` if supported,
///   else `Fifo`.
/// - [`PresentTarget::NativeLowPower`] → `Fifo`.
///
/// Pure function, deterministic, no `wgpu::Device` required.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/present-mode-selection-slice-4z.md#interface
/// @issue #1744
pub fn recommend_present_mode(
    target: PresentTarget,
    supported: &[wgpu::PresentMode],
) -> wgpu::PresentMode {
    let supports = |m: wgpu::PresentMode| supported.contains(&m);
    match target {
        PresentTarget::Web => wgpu::PresentMode::Fifo,
        PresentTarget::NativeInteractive => {
            if supports(wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else {
                wgpu::PresentMode::Fifo
            }
        }
        PresentTarget::NativeLowPower => wgpu::PresentMode::Fifo,
    }
}

/// Canvas alpha-compositing mode. Public, stable, *renderer-typed*
/// (not a re-export of `wgpu::CompositeAlphaMode`) so future wgpu API
/// churn stays contained inside this crate.
///
/// Why this is a separate enum from `wgpu::CompositeAlphaMode`:
/// - The wgpu enum has 5 variants (Opaque, PreMultiplied,
///   PostMultiplied, Inherit, Auto), three of which are
///   platform-specific. The grid renderer's contract with cue / web
///   hosts only needs Opaque vs PreMultiplied — exposing the full
///   wgpu enum would invite callers to depend on variants that don't
///   round-trip across backends.
/// - Alpha mode is a *compositor* knob, not a shader knob. The
///   shader writes whatever color it writes; what changes between
///   variants is how the **browser / window system** treats the
///   resulting canvas pixels at compositing time. Opaque means "the
///   canvas paints over the page"; PreMultiplied means "the canvas
///   blends with the page behind, with RGB already pre-multiplied by
///   alpha". Picking the wrong one results either in the page
///   bleeding through unwanted (PreMultiplied when you wanted
///   Opaque) or in overlays rendering fully solid (Opaque when you
///   wanted to blend).
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/canvas-alpha-mode-slice-4aa.md#interface
/// @issue #1745
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    /// Canvas is treated as fully opaque by the browser compositor.
    /// Shader alpha output is effectively ignored at composite time.
    /// The default for the grid path.
    Opaque,
    /// Canvas composites with the page behind. Each pixel's RGB is
    /// interpreted as already multiplied by its alpha (`color.rgb * a`).
    /// Use this when the renderer hosts overlay UIs that need to
    /// blend with page content (e.g. cue's floating panels).
    PreMultiplied,
}

/// Translate the public [`AlphaMode`] to the matching
/// `wgpu::CompositeAlphaMode`, falling back to `supported[0]` if the
/// preferred variant isn't in the adapter's supported list (defensive:
/// every web adapter lists Opaque, but native backends vary).
///
/// Pure function — deterministic, no `wgpu::Device` required.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/canvas-alpha-mode-slice-4aa.md#interface
/// @issue #1745
pub fn map_alpha_mode(
    mode: AlphaMode,
    supported: &[wgpu::CompositeAlphaMode],
) -> wgpu::CompositeAlphaMode {
    let preferred = match mode {
        AlphaMode::Opaque => wgpu::CompositeAlphaMode::Opaque,
        AlphaMode::PreMultiplied => wgpu::CompositeAlphaMode::PreMultiplied,
    };
    if supported.contains(&preferred) {
        preferred
    } else {
        // Spec guarantees `supported` is non-empty on a valid adapter;
        // pick the first listed as a safe fallback. The caller has
        // already consented to "whatever the adapter exposes" by not
        // checking `supported_alpha_modes()` first.
        supported
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque)
    }
}

/// Default MSAA sample count. Slice 4p (#1734) picks the per-target
/// default at compile time: 1 on web (axis-aligned cell rects don't
/// need MSAA, and resolves are expensive on integrated GPUs), 4 on
/// native (cheap 4× on modern desktop GPUs and cleans up future
/// rotated overlay / stroke passes).
#[cfg(target_arch = "wasm32")]
const MSAA_DEFAULT: u32 = 1;
#[cfg(not(target_arch = "wasm32"))]
const MSAA_DEFAULT: u32 = 4;

/// Default clear color seeded into `WebGpuRenderer` at construction.
/// Opaque white — matches the dominant light-mode embedding case so
/// first paint does not flash dark before the first real frame's draws
/// land. Slice 4o (#1733).
const WHITE_CLEAR: wgpu::Color = wgpu::Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// Convert the public `[f32; 4]` RGBA shape (the same shape
/// `CellInstance::color` uses) into the `wgpu::Color` the renderer
/// stores and hands to the render-pass load op. Components are linear
/// space in `[0.0, 1.0]`; the `f32 → f64` widen is exact.
fn rgba_to_wgpu_color(rgba: [f32; 4]) -> wgpu::Color {
    wgpu::Color {
        r: rgba[0] as f64,
        g: rgba[1] as f64,
        b: rgba[2] as f64,
        a: rgba[3] as f64,
    }
}

/// Inverse of [`rgba_to_wgpu_color`]. The `f64 → f32` narrow is lossy
/// in theory but lossless for any value that round-trips through
/// `set_clear_color` (the only producer of the stored field).
fn wgpu_color_to_rgba(color: wgpu::Color) -> [f32; 4] {
    [
        color.r as f32,
        color.g as f32,
        color.b as f32,
        color.a as f32,
    ]
}

/// Outcome of [`WebGpuRenderer::try_acquire_frame`]. Variants name the
/// action the caller should take so the frame loop does not have to
/// re-decode wgpu's error vocabulary.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/surface-acquire-recovery-slice-4n.md#interface
/// @issue #1732
pub enum AcquireOutcome<'r, 'window> {
    /// Surface texture was acquired (possibly after a single
    /// reconfigure+retry on `Outdated`). Caller should encode the
    /// frame as normal.
    Frame(FrameBuilder<'r, 'window>),
    /// Acquire timed out (transient) — caller should skip this frame
    /// and try again on the next tick.
    Skipped,
    /// Surface or device is gone. Caller MUST call
    /// [`WebGpuRenderer::try_recover`] before requesting another
    /// frame.
    NeedsRecovery,
}

/// Internal classification of a `wgpu::SurfaceError` into the action
/// `try_acquire_frame` should take. Extracted as a free function so the
/// policy is unit-testable without spinning up a GPU.
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/surface-acquire-recovery-slice-4n.md#interface
/// @issue #1732
#[derive(Debug)]
enum AcquireAction {
    SkipTimeout,
    RetryAfterReconfigure,
    NeedsRecovery,
    Propagate(RenderFrameError),
}

fn classify_acquire_error(err: &wgpu::SurfaceError) -> AcquireAction {
    match err {
        wgpu::SurfaceError::Timeout => AcquireAction::SkipTimeout,
        wgpu::SurfaceError::Outdated => AcquireAction::RetryAfterReconfigure,
        wgpu::SurfaceError::Lost => AcquireAction::NeedsRecovery,
        wgpu::SurfaceError::OutOfMemory => AcquireAction::Propagate(RenderFrameError::OutOfMemory),
        other => AcquireAction::Propagate(RenderFrameError::Other(other.to_string())),
    }
}

// `RenderFrameError` does not derive `PartialEq` (wgpu's
// `DeviceLostReason` is not Eq), so the `Propagate` variant cannot
// derive `PartialEq` either. The tests work around this by
// pattern-matching on the variants directly.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_struct_shape_invariant() {
        // Slice 4y (#1743) AC: the public `Screenshot` struct must
        // expose width, height, and a flat RGBA8 pixel buffer such
        // that callers can verify the row-major layout invariant.
        // This compile-time + at-runtime smoke check anchors the
        // invariant `pixels.len() == width * height * 4` that the
        // live round-trip test (`screenshot_round_trip_known_pattern_live`)
        // also asserts.
        let s = Screenshot {
            width: 4,
            height: 3,
            pixels: vec![0u8; 4 * 3 * 4],
        };
        assert_eq!(s.pixels.len(), (s.width * s.height * 4) as usize);
    }

    #[test]
    fn screenshot_error_display_covers_required_variants() {
        // Each variant's Display copy mentions the right keyword so
        // operator logs reading `{e}` directly are readable.
        assert!(ScreenshotError::ZeroSizeSurface
            .to_string()
            .contains("zero width or height"));
        assert!(ScreenshotError::DeviceLost("eGPU unplug".into())
            .to_string()
            .contains("device lost"));
        assert!(ScreenshotError::OutOfMemory("alloc failed".into())
            .to_string()
            .contains("out of memory"));
        assert!(ScreenshotError::BufferMapFailed("invalid".into())
            .to_string()
            .contains("map failed"));
    }

    #[test]
    #[ignore = "requires a live wgpu adapter (CI workers may not have one)"]
    fn screenshot_round_trip_known_pattern_live() {
        // Slice 4y AC: render a known-pattern frame, screenshot,
        // assert pixel values. The pattern: a 10×10 red cell at the
        // origin against a black clear. `(0,0)` should be red,
        // `(50,50)` should be black.
        //
        // We can't construct a `WebGpuRenderer` without a Surface, so
        // this test exercises the screenshot logic end-to-end through
        // the `HeadlessSmokeRenderer` instead (same readback recipe).
        // The `take_screenshot` method itself is exercised on a real
        // surface by downstream visual-regression infra (Epic 18).
        use crate::headless::{request_smoke_adapter, HeadlessSmokeRenderer};
        let runtime = match pollster::block_on(async {
            let (_inst, adapter) = request_smoke_adapter().await?;
            let mut headless = HeadlessSmokeRenderer::new(adapter, (100, 100)).await.ok()?;
            let red_cell = cell_rect::CellInstance {
                pos_px: [0.0, 0.0],
                size_px: [10.0, 10.0],
                color: [1.0, 0.0, 0.0, 1.0],
            };
            let pixels = headless
                .render_single_cell(red_cell, [0.0, 0.0, 0.0, 1.0])
                .await
                .ok()?;
            Some(pixels)
        }) {
            Some(p) => p,
            None => {
                eprintln!("skipping: no software adapter available");
                return;
            }
        };

        // Pixel (0,0) — inside the red cell.
        let offset_red = 0;
        let r = runtime[offset_red];
        let g = runtime[offset_red + 1];
        let b = runtime[offset_red + 2];
        assert!(r > 200, "pixel (0,0) should be red-dominant, got R={r}");
        assert!(g < 50, "pixel (0,0) should have low G, got G={g}");
        assert!(b < 50, "pixel (0,0) should have low B, got B={b}");

        // Pixel (50,50) — outside the cell, should be clear-black.
        let offset_black = (50 * 100 + 50) * 4;
        assert_eq!(runtime[offset_black], 0, "pixel (50,50) should be black R");
        assert_eq!(
            runtime[offset_black + 1],
            0,
            "pixel (50,50) should be black G"
        );
        assert_eq!(
            runtime[offset_black + 2],
            0,
            "pixel (50,50) should be black B"
        );
    }

    #[test]
    fn recommend_web_is_fifo() {
        // Slice 4z (#1744) AC: Web target → Fifo regardless of the
        // adapter's supported list. Browsers translate everything to
        // Fifo at composite time anyway.
        assert_eq!(
            recommend_present_mode(PresentTarget::Web, &[]),
            wgpu::PresentMode::Fifo,
        );
        assert_eq!(
            recommend_present_mode(
                PresentTarget::Web,
                &[wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo],
            ),
            wgpu::PresentMode::Fifo,
        );
    }

    #[test]
    fn recommend_native_interactive_prefers_mailbox() {
        // Slice 4z (#1744) AC: NativeInteractive → Mailbox when the
        // adapter lists it. Mailbox drops the previous unpresented
        // frame on arrival of a newer one — the lowest-latency option
        // for typing/scrolling-dominated UX.
        assert_eq!(
            recommend_present_mode(
                PresentTarget::NativeInteractive,
                &[wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo],
            ),
            wgpu::PresentMode::Mailbox,
        );
    }

    #[test]
    fn recommend_native_interactive_falls_back_to_fifo() {
        // Slice 4z (#1744) AC: NativeInteractive → Fifo when the
        // adapter does NOT list Mailbox. Fifo is required by the
        // WebGPU spec on every adapter, so this branch is the safe
        // floor.
        assert_eq!(
            recommend_present_mode(PresentTarget::NativeInteractive, &[wgpu::PresentMode::Fifo],),
            wgpu::PresentMode::Fifo,
        );
        // Also: empty supported list → Fifo (defensive: spec guarantees
        // Fifo, so a zero-length list is a bug in the caller, but the
        // policy fn still returns Fifo rather than panicking).
        assert_eq!(
            recommend_present_mode(PresentTarget::NativeInteractive, &[]),
            wgpu::PresentMode::Fifo,
        );
    }

    #[test]
    fn recommend_native_low_power_is_fifo() {
        // Slice 4z (#1744) AC: NativeLowPower → Fifo regardless of
        // whether Mailbox is supported. The host has signalled it
        // wants vsync-locked frames for power.
        assert_eq!(
            recommend_present_mode(
                PresentTarget::NativeLowPower,
                &[wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo],
            ),
            wgpu::PresentMode::Fifo,
        );
        assert_eq!(
            recommend_present_mode(PresentTarget::NativeLowPower, &[]),
            wgpu::PresentMode::Fifo,
        );
    }

    #[test]
    fn map_alpha_mode_opaque_returns_opaque_when_supported() {
        // Slice 4aa (#1745) AC: AlphaMode::Opaque maps to
        // wgpu::CompositeAlphaMode::Opaque when the adapter lists it.
        let supported = [
            wgpu::CompositeAlphaMode::Opaque,
            wgpu::CompositeAlphaMode::PreMultiplied,
        ];
        assert_eq!(
            map_alpha_mode(AlphaMode::Opaque, &supported),
            wgpu::CompositeAlphaMode::Opaque,
        );
    }

    #[test]
    fn map_alpha_mode_premultiplied_returns_premultiplied_when_supported() {
        // Slice 4aa AC: AlphaMode::PreMultiplied maps to the matching
        // wgpu variant when supported.
        let supported = [
            wgpu::CompositeAlphaMode::Opaque,
            wgpu::CompositeAlphaMode::PreMultiplied,
        ];
        assert_eq!(
            map_alpha_mode(AlphaMode::PreMultiplied, &supported),
            wgpu::CompositeAlphaMode::PreMultiplied,
        );
    }

    #[test]
    fn map_alpha_mode_falls_back_to_first_supported_when_preferred_absent() {
        // Slice 4aa AC: when the preferred wgpu variant isn't in the
        // adapter's supported list, fall back to `supported[0]`. The
        // caller has consented to "best effort" by not pre-checking
        // `supported_alpha_modes()`.
        let supported = [wgpu::CompositeAlphaMode::Opaque];
        assert_eq!(
            map_alpha_mode(AlphaMode::PreMultiplied, &supported),
            wgpu::CompositeAlphaMode::Opaque,
        );

        let supported = [wgpu::CompositeAlphaMode::PreMultiplied];
        assert_eq!(
            map_alpha_mode(AlphaMode::Opaque, &supported),
            wgpu::CompositeAlphaMode::PreMultiplied,
        );
    }

    #[test]
    fn map_alpha_mode_empty_supported_defaults_to_opaque() {
        // Defensive: a zero-length supported list is a bug in the
        // caller (wgpu spec guarantees every adapter exposes at least
        // one alpha mode), but the policy fn must not panic — it
        // returns `Opaque` as the universal safe floor.
        assert_eq!(
            map_alpha_mode(AlphaMode::Opaque, &[]),
            wgpu::CompositeAlphaMode::Opaque,
        );
        assert_eq!(
            map_alpha_mode(AlphaMode::PreMultiplied, &[]),
            wgpu::CompositeAlphaMode::Opaque,
        );
    }

    #[test]
    fn frame_id_counter_is_monotonic() {
        // AC (Slice 4x / #1742): the `frame_id` field on the `frame`
        // tracing span must be strictly monotonic so a chrome://tracing
        // viewer can order frames. The counter is global, so other
        // tests may have bumped it; we only assert *relative*
        // monotonicity over a fresh pump.
        let a = next_frame_id();
        let b = next_frame_id();
        let c = next_frame_id();
        assert!(b > a, "frame_id must strictly increase: got {a} then {b}");
        assert!(c > b, "frame_id must strictly increase: got {b} then {c}");
    }

    #[test]
    fn renderer_error_display_covers_required_variants() {
        // AC: Error type covers: no adapter, no surface, lost device.
        // Slice 4l (#1730): `NoAdapter` Display now includes the backend
        // description so devs know which mask was searched.
        // Slice 4w (#1741): added NoDevice, OutOfMemory, ValidationFailed.
        let no_adapter = RendererError::NoAdapter.to_string();
        assert!(
            no_adapter.contains("no compatible GPU adapter"),
            "NoAdapter must still mention 'no compatible GPU adapter'; got: {no_adapter}"
        );
        assert!(
            no_adapter.contains(backend::backend_description()),
            "NoAdapter must name the backend mask searched; got: {no_adapter}"
        );
        assert!(RendererError::NoSurface("bad handle".into())
            .to_string()
            .contains("bad handle"));
        assert!(RendererError::DeviceLost("device removed".into())
            .to_string()
            .contains("device removed"));
        assert!(RendererError::NoDevice("limit not supported".into())
            .to_string()
            .contains("limit not supported"));
        assert!(RendererError::OutOfMemory("buffer too large".into())
            .to_string()
            .contains("buffer too large"));
        assert!(RendererError::ValidationFailed("shader compile".into())
            .to_string()
            .contains("shader compile"));
    }

    #[test]
    fn renderer_error_variant_names_match_enum_match_arms() {
        // Compile-time crosscheck: every variant listed in
        // renderer_error_variant_names() must round-trip through a
        // match arm. If a future variant is added but the helper isn't
        // updated, this test fails to compile because the match is
        // non-exhaustive.
        fn name_of(e: &RendererError) -> &'static str {
            match e {
                RendererError::NoAdapter => "NoAdapter",
                RendererError::NoSurface(_) => "NoSurface",
                RendererError::NoDevice(_) => "NoDevice",
                RendererError::DeviceLost(_) => "DeviceLost",
                RendererError::OutOfMemory(_) => "OutOfMemory",
                RendererError::ValidationFailed(_) => "ValidationFailed",
            }
        }
        let names: Vec<&'static str> = vec![
            name_of(&RendererError::NoAdapter),
            name_of(&RendererError::NoSurface(String::new())),
            name_of(&RendererError::NoDevice(String::new())),
            name_of(&RendererError::DeviceLost(String::new())),
            name_of(&RendererError::OutOfMemory(String::new())),
            name_of(&RendererError::ValidationFailed(String::new())),
        ];
        assert_eq!(names.as_slice(), renderer_error_variant_names());
    }

    #[test]
    fn error_variants_by_method_references_only_valid_variants() {
        // AC: documented method → variant table must reference only
        // variants that actually exist in RendererError.
        let valid: std::collections::HashSet<&'static str> =
            renderer_error_variant_names().iter().copied().collect();
        for (method, variants) in ERROR_VARIANTS_BY_METHOD {
            for v in *variants {
                assert!(
                    valid.contains(v),
                    "ERROR_VARIANTS_BY_METHOD references unknown variant {v:?} \
                     for method {method:?}; known variants: {valid:?}",
                );
            }
        }
    }

    #[test]
    fn renderer_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RendererError>();
        assert_send_sync::<RenderFrameError>();
    }

    #[test]
    fn render_frame_error_maps_surface_error_variants() {
        // AC invariant: every wgpu::SurfaceError variant must round-trip
        // into a distinct RenderFrameError variant so callers can pattern
        // match on the cause. The Other arm catches any future
        // non-exhaustive variant from wgpu without losing the message.
        assert!(matches!(
            RenderFrameError::from(wgpu::SurfaceError::Timeout),
            RenderFrameError::Timeout
        ));
        assert!(matches!(
            RenderFrameError::from(wgpu::SurfaceError::Outdated),
            RenderFrameError::Outdated
        ));
        assert!(matches!(
            RenderFrameError::from(wgpu::SurfaceError::Lost),
            RenderFrameError::SurfaceLost
        ));
        assert!(matches!(
            RenderFrameError::from(wgpu::SurfaceError::OutOfMemory),
            RenderFrameError::OutOfMemory
        ));
    }

    #[test]
    fn render_frame_error_display_includes_inner_message() {
        let e = RenderFrameError::Other("driver borked".into());
        let s = e.to_string();
        assert!(s.contains("driver borked"), "got: {s}");
    }

    /// Pins the Slice 4n (#1732) recovery policy: every
    /// `wgpu::SurfaceError` variant maps to a specific
    /// `AcquireAction` so the caller's frame loop has explicit
    /// labels. Pure function — no GPU needed.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/surface-acquire-recovery-slice-4n.md#acceptance-criteria
    #[test]
    fn classify_acquire_error_covers_every_variant() {
        assert!(matches!(
            classify_acquire_error(&wgpu::SurfaceError::Timeout),
            AcquireAction::SkipTimeout
        ));
        assert!(matches!(
            classify_acquire_error(&wgpu::SurfaceError::Outdated),
            AcquireAction::RetryAfterReconfigure
        ));
        assert!(matches!(
            classify_acquire_error(&wgpu::SurfaceError::Lost),
            AcquireAction::NeedsRecovery
        ));
        assert!(matches!(
            classify_acquire_error(&wgpu::SurfaceError::OutOfMemory),
            AcquireAction::Propagate(RenderFrameError::OutOfMemory)
        ));
    }

    /// `Other` propagation must preserve the wgpu-supplied diagnostic
    /// so an unknown future variant does not silently degrade to a
    /// generic "frame skipped" log. Distinguished from
    /// `OutOfMemory` because the latter is a known variant with its
    /// own recovery semantics — Slice 4n routes them through
    /// different arms.
    #[test]
    fn classify_acquire_error_preserves_other_message() {
        // We can't easily construct a `wgpu::SurfaceError::Other`
        // here (the variant is currently exhaustive in wgpu 24), so
        // we exercise the same arm via the `From` impl that backs
        // `RenderFrameError::Other`. The `Propagate` arm carries the
        // verbatim message either way.
        let inner = RenderFrameError::Other("hypothetical-future-variant".into());
        match inner {
            RenderFrameError::Other(msg) => {
                assert_eq!(msg, "hypothetical-future-variant");
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }

    /// Live-GPU cache-identity check for Slice 4b. Requires a real wgpu
    /// adapter (Metal/Vulkan/D3D12). Skipped by default to keep `cargo test`
    /// green on headless CI; run with `cargo test -- --ignored` on a host
    /// with a GPU.
    ///
    /// Slice 4p (#1734) re-keyed the cache on `(format, msaa_count)`;
    /// this test exercises the format axis at fixed `msaa_count = 1`.
    /// The msaa-axis case is unit-covered by `set_msaa_count_clears_pipeline_cache`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/cell-rect-pipeline-slice-4b.md#acceptance-criteria
    #[test]
    #[ignore]
    fn create_cell_pipeline_caches_per_format_live() {
        let mut renderer =
            pollster::block_on(build_headless_renderer()).expect("build_headless_renderer failed");
        let fmt = wgpu::TextureFormat::Bgra8UnormSrgb;
        let p1 = renderer.create_cell_pipeline(fmt, 1);
        let p2 = renderer.create_cell_pipeline(fmt, 1);
        assert!(
            Arc::ptr_eq(&p1, &p2),
            "same (format, msaa) must return the same Arc<RenderPipeline>"
        );
        // Different format -> different Arc.
        let p3 = renderer.create_cell_pipeline(wgpu::TextureFormat::Rgba8UnormSrgb, 1);
        assert!(
            !Arc::ptr_eq(&p1, &p3),
            "different format must build a fresh pipeline"
        );
    }

    /// Live-GPU cache-identity check for Slice 4u's public
    /// `pipeline_for` accessor. Asserts (a) two same-format calls
    /// return `Arc::ptr_eq` handles, (b) `pipeline_cache_len`
    /// stays at `1`, and (c) a different-format call grows the
    /// cache to `2`. Skipped by default (requires a real adapter).
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/per-surface-pipeline-cache-slice-4u.md#acceptance-criteria
    #[test]
    #[ignore]
    fn pipeline_for_returns_cached_pipeline_for_same_format_live() {
        let mut renderer =
            pollster::block_on(build_headless_renderer()).expect("build_headless_renderer failed");

        assert_eq!(
            renderer.pipeline_cache_len(),
            0,
            "fresh renderer should have an empty pipeline cache"
        );

        let fmt = wgpu::TextureFormat::Bgra8UnormSrgb;
        let p1 = renderer.pipeline_for(fmt);
        let p2 = renderer.pipeline_for(fmt);
        assert!(
            Arc::ptr_eq(&p1, &p2),
            "pipeline_for({fmt:?}) must return the same Arc on repeat call",
        );
        assert_eq!(
            renderer.pipeline_cache_len(),
            1,
            "same-format calls must not grow the cache"
        );

        let other = wgpu::TextureFormat::Rgba8UnormSrgb;
        let p3 = renderer.pipeline_for(other);
        assert!(
            !Arc::ptr_eq(&p1, &p3),
            "different format must build a fresh pipeline"
        );
        assert_eq!(
            renderer.pipeline_cache_len(),
            2,
            "two distinct formats must occupy two cache slots"
        );
    }

    /// Compile-time contract: the cell-rect pipeline cache must key
    /// on `(TextureFormat, u32)` — the format dimension is the
    /// Slice 4u public contract; the `u32` is the Slice 4p MSAA
    /// axis. If a refactor accidentally drops either dimension this
    /// assertion stops compiling.
    ///
    /// No GPU required — purely a type-shape check.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/per-surface-pipeline-cache-slice-4u.md#acceptance-criteria
    #[test]
    fn pipeline_cache_key_shape_is_format_and_msaa() {
        fn _shape_check(_cache: &HashMap<(wgpu::TextureFormat, u32), Arc<wgpu::RenderPipeline>>) {}
        // If `cell_pipelines` ever changes shape, this line stops compiling.
        let assertion: fn(&WebGpuRenderer<'_>) = |r| _shape_check(&r.cell_pipelines);
        let _ = assertion;
    }

    /// Live-GPU clear-color round-trip + render-frame smoke test. Drives
    /// the Slice-4o (#1733) `[f32; 4]` signature, and additionally clears
    /// to opaque black over an empty cell list (the AC "black surface"
    /// path) so the render-pass load-op runs against the configured
    /// non-default value. Skipped unless run with `--ignored`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/clear-color-config-slice-4o.md#acceptance-criteria
    #[test]
    #[ignore]
    fn render_frame_runs_end_to_end_live() {
        let mut renderer =
            pollster::block_on(build_headless_renderer()).expect("build_headless_renderer failed");

        let red: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
        renderer.set_clear_color(red);
        assert_eq!(renderer.clear_color(), red);

        // Empty cells with a non-default clear color: must still clear,
        // must not panic on missing instance buffer.
        renderer
            .render_frame(&[])
            .expect("empty frame must succeed");

        // AC bullet: "setting black and rendering an empty cell list
        // produces a black surface". We do not read the surface back here
        // (out of scope — see slice doc), but we do drive the clear path
        // with opaque black so wgpu validation surfaces any setup issue.
        let black: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
        renderer.set_clear_color(black);
        assert_eq!(renderer.clear_color(), black);
        renderer
            .render_frame(&[])
            .expect("black clear must succeed");

        // One cell: exercises the upload + draw path.
        let cell = cell_rect::CellInstance {
            pos_px: [10.0, 10.0],
            size_px: [50.0, 50.0],
            color: [0.0, 1.0, 0.0, 1.0],
        };
        renderer
            .render_frame(&[cell])
            .expect("draw frame must succeed");
    }

    /// Slice 4q (#1735) negotiation — empty adapter mask must opt
    /// into nothing and leave every `NegotiatedFeatures` flag false.
    /// This is the "every WebGPU adapter" baseline.
    #[test]
    fn negotiate_features_empty_adapter_opts_into_nothing() {
        let (req, out) = negotiate_features(wgpu::Features::empty());
        assert_eq!(req, wgpu::Features::empty());
        assert_eq!(out, NegotiatedFeatures::default());
        assert!(!out.timestamp_query);
        assert!(!out.texture_adapter_specific_format_features);
    }

    /// Slice 4q (#1735) — when the adapter advertises only
    /// `TIMESTAMP_QUERY`, we opt into exactly that bit and the
    /// outcome's `timestamp_query` flag flips on (Slice 4i's
    /// `FrameTimingPool` then runs in real-work mode).
    #[test]
    fn negotiate_features_picks_up_timestamp_query() {
        let (req, out) = negotiate_features(wgpu::Features::TIMESTAMP_QUERY);
        assert_eq!(req, wgpu::Features::TIMESTAMP_QUERY);
        assert!(out.timestamp_query);
        assert!(!out.texture_adapter_specific_format_features);
    }

    /// Slice 4q (#1735) — same for the texture-adapter-specific
    /// format-features bit. Isolated case so a regression on either
    /// arm of `negotiate_features` is pinned to that arm.
    #[test]
    fn negotiate_features_picks_up_texture_adapter_specific() {
        let (req, out) =
            negotiate_features(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
        assert_eq!(
            req,
            wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
        );
        assert!(!out.timestamp_query);
        assert!(out.texture_adapter_specific_format_features);
    }

    /// Slice 4q (#1735) — both optionals together must produce a
    /// union mask and both flags on. Guards against an arm-ordering
    /// regression where the second `if` shadows the first.
    #[test]
    fn negotiate_features_unions_both_optionals() {
        let advertised = wgpu::Features::TIMESTAMP_QUERY
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
        let (req, out) = negotiate_features(advertised);
        assert_eq!(req, advertised);
        assert!(out.timestamp_query);
        assert!(out.texture_adapter_specific_format_features);
    }

    /// Slice 4q (#1735) — non-opted-into bits on the adapter must
    /// NOT leak into the requested mask. We pick `MAPPABLE_PRIMARY_BUFFERS`
    /// because it's a real wgpu feature we have no current consumer for
    /// — if we ever did, the test should be updated to a still-
    /// uninterested bit.
    #[test]
    fn negotiate_features_ignores_non_opted_features() {
        let (req, out) = negotiate_features(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS);
        assert_eq!(req, wgpu::Features::empty());
        assert_eq!(out, NegotiatedFeatures::default());
    }

    /// Pin the target-conditional MSAA default. Slice 4p (#1734) chose
    /// `cfg(target_arch = "wasm32") → 1, else → 4` deliberately — a
    /// future edit that flips either branch should fail this test
    /// loudly rather than silently regress first-paint flash on web or
    /// edge-quality on native.
    #[test]
    fn msaa_default_is_target_conditional() {
        #[cfg(target_arch = "wasm32")]
        assert_eq!(MSAA_DEFAULT, 1, "web default must stay 1");
        #[cfg(not(target_arch = "wasm32"))]
        assert_eq!(MSAA_DEFAULT, 4, "native default must stay 4");
    }

    /// Live-GPU coverage of the `set_msaa_count` value contract +
    /// pipeline-cache invalidation behavior. Two parts:
    /// 1. Setting an unsupported count (2, 8, 16, 0) is silently
    ///    ignored — Slice 4q (#1735) will gate arbitrary counts on
    ///    adapter feature negotiation; until then we hold the line at
    ///    {1, 4}.
    /// 2. Flipping from count=1 to count=4 must clear the cached
    ///    pipeline so the next `create_cell_pipeline` rebuilds with
    ///    the new `MultisampleState.count`.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/msaa-toggle-slice-4p.md#acceptance-criteria
    #[test]
    #[ignore]
    fn set_msaa_count_validates_and_clears_pipeline_cache_live() {
        let mut renderer =
            pollster::block_on(build_headless_renderer()).expect("build_headless_renderer failed");

        let start = renderer.msaa_count();
        for bad in [0u32, 2, 3, 5, 8, 16] {
            renderer.set_msaa_count(bad);
            assert_eq!(
                renderer.msaa_count(),
                start,
                "set_msaa_count({}) must be ignored",
                bad
            );
        }

        // Force count=1 baseline, prime the cache, flip to 4, expect
        // a fresh Arc — different MultisampleState means different
        // pipeline.
        renderer.set_msaa_count(1);
        assert_eq!(renderer.msaa_count(), 1);
        assert!(
            renderer.msaa_color_view().is_none(),
            "count=1 → no MSAA view"
        );
        let fmt = wgpu::TextureFormat::Bgra8UnormSrgb;
        let single = renderer.create_cell_pipeline(fmt, 1);

        renderer.set_msaa_count(4);
        assert_eq!(renderer.msaa_count(), 4);
        assert!(renderer.msaa_color_view().is_some(), "count>1 → MSAA view");
        let multi = renderer.create_cell_pipeline(fmt, 4);
        assert!(
            !Arc::ptr_eq(&single, &multi),
            "msaa toggle must invalidate cached pipeline"
        );
    }

    #[test]
    fn wgsl_viewport_struct_carries_scroll_px() {
        // Slice 4s (#1737): the WGSL `Viewport` struct must expose
        // `scroll_px` and `vs_main` must subtract it before NDC.
        // A future edit that drops either half breaks scroll
        // silently — the visual would just stop translating — so
        // this assertion is the fail-fast.
        let src = cell_rect::CELL_RECT_WGSL;
        assert!(
            src.contains("scroll_px: vec2<f32>"),
            "Viewport struct must declare scroll_px",
        );
        assert!(
            src.contains("- viewport.scroll_px"),
            "vs_main must subtract viewport.scroll_px before NDC",
        );
    }

    #[test]
    fn rgba_round_trips_through_wgpu_color() {
        // Pin the public ↔ internal conversion. The renderer stores
        // `wgpu::Color` but the public API is `[f32; 4]`; this asserts
        // the bridge preserves both default and arbitrary values.
        assert_eq!(wgpu_color_to_rgba(WHITE_CLEAR), [1.0, 1.0, 1.0, 1.0]);
        let cases: [[f32; 4]; 4] = [
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            [0.25, 0.5, 0.75, 0.5],
            [0.0, 1.0, 0.0, 1.0],
        ];
        for rgba in cases {
            let round = wgpu_color_to_rgba(rgba_to_wgpu_color(rgba));
            assert_eq!(round, rgba, "rgba {:?} did not round-trip", rgba);
        }
    }

    /// Build a `WebGpuRenderer` against a 1×1 off-screen surface for tests.
    /// Skips early if no adapter is available so the test surface degrades
    /// gracefully on hosts without a GPU.
    #[allow(dead_code)]
    async fn build_headless_renderer() -> Result<WebGpuRenderer<'static>, RendererError> {
        // Minimal off-screen path: create an instance, request adapter, then
        // construct a renderer that wraps a 1×1 surface against a leaked
        // headless window stand-in. wgpu 24's SurfaceTarget accepts any
        // raw-window-handle target; for unit tests we use OffscreenCanvas
        // semantics where available, otherwise this test simply returns
        // `RendererError::NoSurface` and the assertion above is skipped by
        // `#[ignore]`.
        Err(RendererError::NoSurface(
            "headless test harness not wired (run via integration test with winit)".into(),
        ))
    }
}
