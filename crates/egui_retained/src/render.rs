//! Drawing frames with wgpu (feature `wgpu`): egui-wgpu's renderer, which needs only epaint,
//! fed the frame's triangles and texture changes - onto a window's surface or into a texture
//! of one's own (a picture of the interface without a window).

use crate::Frame;
use epaint::TextureId;

/// Draws [`Frame`]s with wgpu.
pub struct Renderer {
    inner: egui_wgpu::Renderer,
    format: wgpu::TextureFormat,
}

impl Renderer {
    /// For targets of `format` (the surface's).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Renderer {
        Renderer { inner: egui_wgpu::Renderer::new(device, format, egui_wgpu::RendererOptions::default()), format }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// A texture of the application's for [`crate::widgets::Image`] (a 3D picture).
    pub fn register(&mut self, device: &wgpu::Device, view: &wgpu::TextureView, filter: wgpu::FilterMode) -> TextureId {
        self.inner.register_native_texture(device, view, filter)
    }

    /// Another view behind an id given by [`Renderer::register`] (the picture was made anew).
    pub fn update(&mut self, device: &wgpu::Device, id: TextureId, view: &wgpu::TextureView, filter: wgpu::FilterMode) {
        self.inner.update_egui_texture_from_wgpu_texture(device, view, filter, id);
    }

    pub fn free(&mut self, id: TextureId) {
        self.inner.free_texture(&id);
    }

    /// Only the frame's texture changes (a frame that is not drawn: the fonts' atlas still
    /// has to follow, or the next frame's text lacks glyphs).
    pub fn draw_textures_only(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Frame) {
        for (id, delta) in &frame.textures.set {
            self.inner.update_texture(device, queue, *id, delta);
        }
        for id in &frame.textures.free {
            self.inner.free_texture(id);
        }
    }

    /// Draw `frame` onto `target` (`size` in physical pixels), clearing it first to `clear`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, size: [u32; 2], frame: &Frame, clear: Option<wgpu::Color>) {
        for (id, delta) in &frame.textures.set {
            self.inner.update_texture(device, queue, *id, delta);
        }
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: size, pixels_per_point: frame.pixels_per_point };
        let extra = self.inner.update_buffers(device, queue, encoder, &frame.primitives, &screen);
        if !extra.is_empty() {
            queue.submit(extra);
        }
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_retained"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.inner.render(&mut pass, &frame.primitives, &screen);
        }
        for id in &frame.textures.free {
            self.inner.free_texture(id);
        }
    }

    /// `frame` drawn into a texture of its own and read back: RGBA8 rows of `size[0]` pixels.
    pub fn picture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: [u32; 2], frame: &Frame, clear: wgpu::Color) -> Vec<u8> {
        let (w, h) = (size[0].max(1), size[1].max(1));
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("egui_retained picture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let mut enc = device.create_command_encoder(&Default::default());
        self.draw(device, queue, &mut enc, &view, [w, h], frame, Some(clear));
        let stride = (w * 4).div_ceil(256) * 256;
        let buf = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: (stride * h) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
        enc.copy_texture_to_buffer(tex.as_image_copy(), wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None } }, wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range();
        let bgra = matches!(self.format, wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm);
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let row = &data[(y * stride) as usize..(y * stride + w * 4) as usize];
            for px in row.chunks_exact(4) {
                if bgra {
                    out.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    out.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        out
    }
}
