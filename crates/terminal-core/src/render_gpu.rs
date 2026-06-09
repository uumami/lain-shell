//! GPU renderer: GridSnapshot -> wgpu + glyphon. Shares the cosmic-text layout
//! (`cell_metrics`, `grid_to_text`) with the CPU backend so both lay out glyphs
//! identically -- the "shared text engine" seam (design §4). glyphon 0.6
//! re-exports cosmic-text 0.12 (the version this crate already uses), so there is
//! one FontSystem/Buffer/Metrics type across both backends.

use crate::render::{cell_metrics, grid_to_text, PixelBuffer};
use glyphon::fontdb;
use glyphon::{
    Attrs, Buffer, Cache, Color as GColor, Family, FontSystem, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use lain_types::GridSnapshot;

const BUNDLED_FONT: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");

/// Try to acquire a headless wgpu device+queue (no surface). Returns `None` if no
/// adapter is available (GPU-less CI, missing drivers) so callers can skip.
pub fn try_headless_gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: None,
    }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("bebop-headless"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ))
    .ok()?;
    Some((device, queue))
}

pub struct GpuRenderer {
    font_system: FontSystem,
    swash: SwashCache,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    viewport: Viewport,
    buffer: Buffer,
    cell_w: u32,
    cell_h: u32,
    format: wgpu::TextureFormat,
}

impl GpuRenderer {
    /// Build a GPU renderer targeting `format` (must equal the format of every
    /// view passed to `render_to_view`). Bundled font only -> no system enumeration.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        font_size: f32,
    ) -> Self {
        let mut db = fontdb::Database::new();
        db.load_font_data(BUNDLED_FONT.to_vec());
        let fam = db
            .faces()
            .next()
            .map(|f| f.families[0].0.clone())
            .expect("bundled font has a family");
        db.set_monospace_family(fam.clone());
        db.set_sans_serif_family(fam.clone());
        db.set_serif_family(fam);
        let mut font_system = FontSystem::new_with_locale_and_db("en-US".into(), db);

        let (metrics, cell_w, cell_h) = cell_metrics(font_size);
        let swash = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        let buffer = Buffer::new(&mut font_system, metrics);

        GpuRenderer {
            font_system,
            swash,
            atlas,
            text_renderer,
            viewport,
            buffer,
            cell_w,
            cell_h,
            format,
        }
    }

    /// Convenience: a headless renderer targeting `Rgba8UnormSrgb`, for offscreen
    /// readback (parity/golden tests). Callers need not name a wgpu format.
    pub fn new_offscreen(device: &wgpu::Device, queue: &wgpu::Queue, font_size: f32) -> Self {
        Self::new(device, queue, wgpu::TextureFormat::Rgba8UnormSrgb, font_size)
    }

    /// Pixel size of the canvas for `grid` (cols*cell_w x lines*cell_h, min 1x1).
    pub fn canvas_size(&self, grid: &GridSnapshot) -> (u32, u32) {
        (
            (grid.cols as u32 * self.cell_w).max(1),
            (grid.lines as u32 * self.cell_h).max(1),
        )
    }

    /// Lay out `grid` and render it into `view` (an RGBA target of `width`x`height`
    /// whose format == the one passed to `new`). Clears to the BEBOP background,
    /// issues its own encoder + submit. Does NOT present — the caller presents a
    /// surface frame (window) or copies the texture (offscreen).
    pub fn render_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid: &GridSnapshot,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let text = grid_to_text(grid);
        self.buffer
            .set_size(&mut self.font_system, Some(width as f32), Some(height as f32));
        self.buffer.set_text(
            &mut self.font_system,
            &text,
            Attrs::new().family(Family::Monospace),
            Shaping::Basic,
        );
        self.buffer.shape_until_scroll(&mut self.font_system, false);

        self.viewport.update(queue, Resolution { width, height });
        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                [TextArea {
                    buffer: &self.buffer,
                    left: 0.0,
                    top: 0.0,
                    scale: 1.0,
                    bounds: TextBounds { left: 0, top: 0, right: width as i32, bottom: height as i32 },
                    default_color: GColor::rgb(220, 220, 220),
                    custom_glyphs: &[],
                }],
                &mut self.swash,
            )
            .expect("glyphon prepare");

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("bebop-gpu") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bebop-text"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Clear to the CPU backend's 0x0d0d0f background. The target is an
                        // sRGB format, so wgpu sRGB-encodes the (linear) clear value on write;
                        // these are the linear pre-images of sRGB bytes 13,13,15, chosen so
                        // the stored pixels read back as ~0x0d0d0f (verified by the offscreen
                        // readback test).
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.004025,
                            g: 0.004025,
                            b: 0.004777,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .expect("glyphon render");
        }
        queue.submit(std::iter::once(encoder.finish()));
        self.atlas.trim();
    }

    /// Headless: render `grid` to an offscreen texture and read it back as a
    /// `PixelBuffer` (0x00RRGGBB). No window. Used by parity/golden tests.
    pub fn render_offscreen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid: &GridSnapshot,
    ) -> PixelBuffer {
        let (width, height) = self.canvas_size(grid);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bebop-offscreen"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.render_to_view(device, queue, grid, &view, width, height);

        // copy_texture_to_buffer requires bytes_per_row aligned to 256.
        let bpp = 4u32;
        let unpadded = width * bpp;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let out_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bebop-readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("bebop-readback") });
        enc.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &out_buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));

        let slice = out_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv().expect("map channel").expect("buffer map");
        let mapped = slice.get_mapped_range();

        let mut data = vec![0u32; (width * height) as usize];
        for y in 0..height {
            let row = &mapped[(y * padded) as usize..];
            for x in 0..width {
                let i = (x * bpp) as usize;
                let r = row[i] as u32;
                let g = row[i + 1] as u32;
                let b = row[i + 2] as u32;
                data[(y * width + x) as usize] = (r << 16) | (g << 8) | b;
            }
        }
        drop(mapped);
        out_buf.unmap();
        PixelBuffer { width, height, data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lain_types::Cursor;

    fn snapshot(cols: usize, lines: usize, fill: &[(usize, char)]) -> GridSnapshot {
        let mut cells = vec![' '; cols * lines];
        for &(i, c) in fill {
            cells[i] = c;
        }
        GridSnapshot { cols, lines, cells, cursor: Cursor { line: 0, col: 0 } }
    }

    #[test]
    fn gpu_blank_is_uniform_and_glyph_paints() {
        let Some((device, queue)) = try_headless_gpu() else {
            eprintln!("SKIP gpu_blank_is_uniform_and_glyph_paints: no wgpu adapter");
            return;
        };
        let mut r = GpuRenderer::new_offscreen(&device, &queue, 14.0);

        let blank = r.render_offscreen(&device, &queue, &snapshot(10, 3, &[]));
        let bg = blank.at(blank.width - 1, blank.height - 1);
        assert!(blank.data.iter().all(|&p| p == bg), "blank grid must be uniform background");

        let g = r.render_offscreen(&device, &queue, &snapshot(10, 3, &[(0, 'X')]));
        let gbg = g.at(g.width - 1, g.height - 1);
        assert!(g.data.iter().any(|&p| p != gbg), "glyph must paint non-background pixels");
        // The entire bottom (blank) row band must stay background -> glyph stayed in its row.
        let (_m, _cw, ch) = cell_metrics(14.0);
        let band = (2 * ch * g.width) as usize;
        assert!(
            g.data[band..].iter().all(|&p| p == gbg),
            "blank bottom row must stay background"
        );
    }

    #[test]
    fn gpu_render_is_deterministic() {
        let Some((device, queue)) = try_headless_gpu() else {
            eprintln!("SKIP gpu_render_is_deterministic: no wgpu adapter");
            return;
        };
        let mut r = GpuRenderer::new_offscreen(&device, &queue, 14.0);
        let grid = snapshot(8, 2, &[(0, 'h'), (1, 'i')]);
        let a = r.render_offscreen(&device, &queue, &grid);
        let b = r.render_offscreen(&device, &queue, &grid);
        assert_eq!(a.data, b.data, "same grid must rasterize identically on the GPU");
    }
}
