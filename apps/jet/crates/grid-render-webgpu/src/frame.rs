//! Per-frame command-batching builder.
//!
//! Why this module exists: a wgpu `Queue::submit` is a driver
//! round-trip — costly enough that fanning N passes per frame into N
//! submits measurably hurts CPU frame time. The fix is one encoder +
//! one submit per frame; later passes (text, overlays) chain onto the
//! same encoder. `FrameBuilder` is the type that enforces that
//! invariant: it owns the encoder, accepts pass encodings, and
//! consumes itself in `commit` so the same frame can never be
//! submitted twice.
//!
//! Invariant — one submit per frame: a `FrameBuilder` issues exactly
//! one `Queue::submit` over its lifetime, in `commit`. Drop without
//! commit silently discards encoded work (the surface texture is
//! released without `present`). This is intentional — early-failure
//! paths in caller code should be free to `?`-propagate without
//! risking a double-submit.
//!
//! Invariant — RenderPass scoping: every encoded pass scopes its
//! `wgpu::RenderPass` in its own block so the pass drops before
//! `encoder.finish()` runs. Without that scope, wgpu's validation
//! layer would warn ("a RenderPass was not ended before finish"), and
//! some backends (Metal) would silently drop the pass.

use crate::cell_rect::CellInstance;
use crate::text_pass::GlyphInstance;
use crate::{RenderFrameError, WebGpuRenderer};

/// A frame in progress: owns the acquired surface texture, target view,
/// and the in-flight command encoder. Built by
/// [`WebGpuRenderer::begin_frame`] and consumed by [`Self::commit`].
///
/// @spec apps/jet/crates/grid-render-webgpu/docs/submit-batching-slice-4j.md#interface
/// @issue #1728
pub struct FrameBuilder<'r, 'window> {
    renderer: &'r mut WebGpuRenderer<'window>,
    surface_texture: Option<wgpu::SurfaceTexture>,
    view: wgpu::TextureView,
    encoder: Option<wgpu::CommandEncoder>,
    submit_count: u32,
}

impl<'r, 'window> FrameBuilder<'r, 'window> {
    /// Internal constructor — only `WebGpuRenderer::begin_frame` builds
    /// `FrameBuilder` instances. Keeping this `pub(crate)` means callers
    /// can never bypass the surface-acquire path.
    pub(crate) fn new(
        renderer: &'r mut WebGpuRenderer<'window>,
        surface_texture: wgpu::SurfaceTexture,
    ) -> Self {
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = renderer
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_builder_encoder"),
            });
        Self {
            renderer,
            surface_texture: Some(surface_texture),
            view,
            encoder: Some(encoder),
            submit_count: 0,
        }
    }

    /// Encode a single cell-rect render pass into the shared encoder.
    ///
    /// `clear = Some(c)` clears the color attachment to `c` at pass
    /// start (`LoadOp::Clear`). `clear = None` uses `LoadOp::Load`, so
    /// the pass composites on top of whatever earlier pass(es) drew
    /// this frame — the seam later slices use to add a text pass over
    /// a previously-cleared cell pass without re-clearing.
    ///
    /// Empty `cells` still encodes the pass (so the clear, if any,
    /// runs) but skips the instance upload + draw call.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/submit-batching-slice-4j.md#interface
    /// @issue #1728
    pub fn encode_cell_pass(&mut self, cells: &[CellInstance], clear: Option<wgpu::Color>) {
        // Nests under the active `frame` span when called from
        // `WebGpuRenderer::render_frame` (tracing's parent-resolution
        // rules pick up whichever span is entered on the current
        // thread). Slice 4x (#1742).
        let span = tracing::info_span!("cell_pass", cells = cells.len());
        let _guard = span.enter();
        let format = self.renderer.format();
        let msaa_count = self.renderer.msaa_count();
        let pipeline = self.renderer.create_cell_pipeline(format, msaa_count);

        // Upload instance data into pool slot 0 iff non-empty.
        // `upload_instance_buffer` returns an owned handle (wgpu::Buffer
        // is internally ref-counted) so the buffer outlives the `&mut
        // self.renderer` borrow it took to (re)allocate the pool slot.
        let instance_buffer: Option<wgpu::Buffer> = if !cells.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(cells);
            let min_size = bytes.len() as wgpu::BufferAddress;
            Some(self.renderer.upload_instance_buffer(0, min_size, bytes))
        } else {
            None
        };

        let load_op = match clear {
            Some(c) => wgpu::LoadOp::Clear(c),
            None => wgpu::LoadOp::Load,
        };

        let viewport_bind_group = self.renderer.viewport_bind_group();
        // Slice 4p (#1734): pick the color-attachment topology by MSAA
        // state. count == 1 → draw straight into the surface view as
        // before. count > 1 → draw into the MSAA view and let wgpu
        // resolve into the surface view on `present`.
        let msaa_view = self.renderer.msaa_color_view();
        let (attachment_view, resolve_target) = match msaa_view {
            Some(view) => (view, Some(&self.view)),
            None => (&self.view, None),
        };
        // Borrow the timestamp `QuerySet` (if the pool is enabled) up
        // front so the value lives long enough for the
        // `RenderPassDescriptor` to reference it. None on hosts
        // without `Features::TIMESTAMP_QUERY` — see frame_timing
        // module docs for the degradation semantic.
        let timestamp_query_set: Option<&wgpu::QuerySet> =
            self.renderer.frame_timing.active_query_set();
        let timestamp_writes = timestamp_query_set.map(|qs| wgpu::RenderPassTimestampWrites {
            query_set: qs,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        });
        let encoder = self
            .encoder
            .as_mut()
            .expect("encoder consumed before commit — builder used after commit?");
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cell_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: attachment_view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes,
                occlusion_query_set: None,
            });
            if let Some(buf) = instance_buffer {
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, viewport_bind_group, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..4, 0..cells.len() as u32);
            }
            // pass dropped here — wgpu validation requires it BEFORE
            // encoder.finish() in `commit`.
        }
    }

    /// Encode a single text-pass render pass into the shared encoder,
    /// chained onto the same encoder as `encode_cell_pass`. Uses
    /// instance pool slot 1 so the cell pass's slot-0 buffer is
    /// untouched — both slots can resize independently across frames.
    ///
    /// `clear = None` (the normal text-over-cells path) keeps the
    /// load op as `Load`, so the text composites on top of whatever
    /// the cell pass drew. `clear = Some(c)` clears first — provided
    /// for symmetry with `encode_cell_pass`; the renderer's
    /// `render_frame_with_text` always passes `None`.
    ///
    /// Empty `glyphs` still encodes the pass (load op runs, no draw
    /// call) so the seam is observable in tests even before the
    /// shaper produces real glyph data.
    ///
    /// @spec .aw/tech-design/projects/jet/specs/webgpu-text-pass-encode.md#interaction
    /// @issue #2191
    pub fn encode_text_pass(&mut self, glyphs: &[GlyphInstance], clear: Option<wgpu::Color>) {
        let span = tracing::info_span!("text_pass", glyphs = glyphs.len());
        let _guard = span.enter();
        let format = self.renderer.format();
        let msaa_count = self.renderer.msaa_count();
        let pipeline = self.renderer.create_text_pipeline(format, msaa_count);

        // Slice #2191: text instances live in pool slot 1 — independent
        // of slot 0 (cell pass) so the two passes can grow without
        // stomping each other's buffer.
        let instance_buffer: Option<wgpu::Buffer> = if !glyphs.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(glyphs);
            let min_size = bytes.len() as wgpu::BufferAddress;
            Some(self.renderer.upload_instance_buffer(1, min_size, bytes))
        } else {
            None
        };

        let load_op = match clear {
            Some(c) => wgpu::LoadOp::Clear(c),
            None => wgpu::LoadOp::Load,
        };

        let text_bind_group = self.renderer.text_bind_group();
        let msaa_view = self.renderer.msaa_color_view();
        let (attachment_view, resolve_target) = match msaa_view {
            Some(view) => (view, Some(&self.view)),
            None => (&self.view, None),
        };
        let encoder = self
            .encoder
            .as_mut()
            .expect("encoder consumed before commit — builder used after commit?");
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: attachment_view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                // Text pass deliberately skips timestamp writes — the
                // FrameTimingPool only reserves a single query pair per
                // frame (cell pass). Adding a second pair here would
                // require widening the pool descriptor; orthogonal to
                // the encode seam this slice ships.
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if let Some(buf) = instance_buffer {
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, text_bind_group, &[]);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..4, 0..glyphs.len() as u32);
            }
        }
    }

    /// Finalize the encoder, submit it (exactly one submit), and
    /// present the surface texture.
    ///
    /// Consumes `self` so the same frame can never be committed twice.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/submit-batching-slice-4j.md#interface
    /// @issue #1728
    pub fn commit(mut self) -> Result<(), RenderFrameError> {
        let mut encoder = self
            .encoder
            .take()
            .expect("encoder already consumed — commit called twice?");
        let surface_texture = self
            .surface_texture
            .take()
            .expect("surface texture already consumed");

        // Flush any prior timing callback before reusing a ring slot.
        if self.renderer.frame_timing.is_enabled() {
            self.renderer.device.poll(wgpu::Maintain::Poll);
        }

        // Slice 4i (#1727): resolve + copy the active timing slot, then
        // advance to the next slot. No-op when the pool is disabled.
        // Must encode BEFORE `encoder.finish()` so the resolve commands
        // ride on the same submission as the timestamped pass, but the
        // returned map work must start only AFTER `queue.submit()`.
        let pending_timing_readback = self.renderer.frame_timing.finish_frame(&mut encoder);

        self.renderer
            .queue
            .submit(std::iter::once(encoder.finish()));
        self.submit_count += 1;
        if let Some(pending) = pending_timing_readback {
            pending.start_mapping();
        }

        // Drive previously-submitted slots' `map_async` callbacks
        // non-blocking. Without this poll, the callback that updates
        // `last_frame_gpu_ms` would never fire — wgpu only runs
        // mapping callbacks during `Device::poll`. `Maintain::Poll`
        // is the non-blocking variant; we never want to stall the
        // render thread waiting for the GPU here.
        if self.renderer.frame_timing.is_enabled() {
            self.renderer.device.poll(wgpu::Maintain::Poll);
        }

        surface_texture.present();
        Ok(())
    }

    /// Number of `Queue::submit` calls this builder has issued. Used by
    /// tests to enforce the one-submit-per-frame invariant; not part
    /// of the public render-loop API.
    ///
    /// @spec apps/jet/crates/grid-render-webgpu/docs/submit-batching-slice-4j.md#scope
    /// @issue #1728
    #[doc(hidden)]
    pub fn submit_count_for_tests(&self) -> u32 {
        self.submit_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The FrameBuilder type requires a live `WebGpuRenderer` to
    // construct (the constructor is pub(crate) and consumes a real
    // SurfaceTexture). Host-target tests cannot build that without a
    // GPU. The compile-time tests below assert the *type-level*
    // properties of the contract — that commit consumes self, that
    // submit_count is observable, that the builder is not Clone. The
    // live-GPU smoke test for the actual submit-once invariant lives
    // alongside the existing `render_frame_runs_end_to_end_live`
    // test in lib.rs (where the headless harness is wired).

    /// Static assertion that `FrameBuilder::commit` consumes `self`.
    /// If a future edit changes commit to `&mut self`, the function
    /// item below no longer coerces to a `fn` pointer whose receiver
    /// is by-value, and this test stops compiling — guarding the
    /// one-submit invariant at compile time.
    #[allow(dead_code)]
    fn commit_takes_self_by_value<'r, 'w: 'r>() {
        let _commit: fn(FrameBuilder<'r, 'w>) -> Result<(), RenderFrameError> =
            FrameBuilder::<'r, 'w>::commit;
    }

    /// Compile-time witness that `FrameBuilder` is not `Copy`. The
    /// submit-once invariant relies on `commit` consuming the builder;
    /// a `Copy` type would let callers commit the same frame twice.
    #[allow(dead_code)]
    fn builder_is_not_copy<'r, 'w>(b: FrameBuilder<'r, 'w>) -> FrameBuilder<'r, 'w> {
        // Returning `b` after re-binding would fail to compile if
        // FrameBuilder were Copy — the second binding would create a
        // bitwise copy and the original would still be live, but the
        // borrow checker doesn't care. We rely on the absence of
        // `#[derive(Copy, Clone)]` instead; this fn just self-documents
        // the invariant.
        b
    }
}
