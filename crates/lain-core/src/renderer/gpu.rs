use std::sync::Arc;

use cosmic_text::{Buffer, FontSystem};
use wgpu::{
    CommandEncoderDescriptor, CurrentSurfaceTexture, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, Surface, SurfaceConfiguration, TextureViewDescriptor,
};
use winit::window::Window;

use super::cell_grid::{CellGrid, CursorState};
use super::glyphon_backend::GlyphonRenderer;
use super::rect::{RectInstance, RectRenderer};

pub struct GpuRenderer {
    surface: Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: SurfaceConfiguration,
    glyphon: GlyphonRenderer,
    rects: RectRenderer,
    cached_buffers: Vec<Option<Buffer>>,
    bg_color: wgpu::Color,
}

impl GpuRenderer {
    /// Try to create a GPU renderer. Returns None if no suitable adapter is available.
    pub fn new(
        instance: wgpu::Instance,
        window: Arc<Window>,
        scale_factor: f32,
        font_system: &mut FontSystem,
    ) -> Option<Self> {
        let surface = instance.create_surface(window.clone()).ok()?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok()?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("lain-shell"),
            ..Default::default()
        }))
        .ok()?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer sRGB formats — glyphon composites text in linear space and
        // expects an sRGB output surface for correct gamma-aware antialiasing.
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        log::info!("present mode: {:?}", config.present_mode);

        let glyphon =
            GlyphonRenderer::new_with_scale(&device, &queue, format, scale_factor, font_system);
        let rects = RectRenderer::new(&device, format);

        Some(Self {
            surface,
            device,
            queue,
            config,
            glyphon,
            rects,
            cached_buffers: Vec::new(),
            // On an sRGB surface wgpu clear color is in linear space.
            // To display rgb(13, 13, 18) sRGB we need the linear equivalents:
            //   linear = ((srgb/255 + 0.055) / 1.055)^2.4
            //   r: 13/255=0.0510 → 0.00400
            //   g: same         → 0.00400
            //   b: 18/255=0.0706 → 0.00601
            bg_color: wgpu::Color {
                r: 0.00400,
                g: 0.00400,
                b: 0.00601,
                a: 1.0,
            },
        })
    }

    pub fn cell_metrics(&self) -> (f32, f32) {
        self.glyphon.cell_metrics()
    }

    /// Update font metrics for a new scale factor without recreating the surface.
    pub fn update_scale(&mut self, scale_factor: f32, font_system: &mut FontSystem) {
        self.glyphon.update_scale(scale_factor, font_system);
        self.cached_buffers.clear();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.cached_buffers.clear();
    }

    /// Returns true if the caller should request another redraw (surface was reconfigured).
    pub fn render_frame(&mut self, grid: &CellGrid, font_system: &mut FontSystem) -> bool {
        let width = self.config.width;
        let height = self.config.height;
        let (cell_w, cell_h) = self.glyphon.cell_metrics();

        // Acquire surface texture.
        // Suboptimal: render with the frame we have, configure AFTER present (no live texture).
        // Outdated/Lost: no texture was acquired, safe to configure immediately and skip frame.
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(frame) => (frame, false),
            CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return true; // request redraw with fresh surface config
            }
            CurrentSurfaceTexture::Timeout
            | CurrentSurfaceTexture::Occluded
            | CurrentSurfaceTexture::Validation => {
                return false;
            }
        };

        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("render"),
            });

        // Ensure cached_buffers has correct size
        let rows = grid.rows as usize;
        let cols = grid.cols;
        if self.cached_buffers.len() != rows {
            self.cached_buffers.clear();
            self.cached_buffers.resize_with(rows, || None);
        }

        // Rebuild only dirty row Buffers
        for row in 0..rows {
            if grid.dirty_rows.get(row).copied().unwrap_or(true) {
                let start = row * cols as usize;
                let end = (start + cols as usize).min(grid.cells.len());
                let row_cells = &grid.cells[start..end];
                let buf = self.glyphon.build_row_buffer(font_system, row_cells, cols);
                self.cached_buffers[row] = Some(buf);
            }
        }

        // Build background rects, then underline/strikethrough on top
        let bg_default = cosmic_text::Color::rgba(0, 0, 0, 0);
        let mut rect_instances: Vec<RectInstance> = Vec::new();
        for row in 0..grid.rows as usize {
            for col in 0..cols as usize {
                let idx = row * cols as usize + col;
                if let Some(cell) = grid.cells.get(idx) {
                    if cell.bg != bg_default {
                        let (r, g, b, a) = (
                            cell.bg.r() as f32 / 255.0,
                            cell.bg.g() as f32 / 255.0,
                            cell.bg.b() as f32 / 255.0,
                            cell.bg.a() as f32 / 255.0,
                        );
                        if a > 0.01 {
                            rect_instances.push(RectInstance {
                                pos: [col as f32 * cell_w, row as f32 * cell_h],
                                size: [cell_w, cell_h],
                                color: [r, g, b, a],
                            });
                        }
                    }
                }
            }
        }

        // Cursor rect
        build_cursor_rect(
            &grid.cursor,
            grid.rows,
            cols,
            cell_w,
            cell_h,
            &mut rect_instances,
        );

        // Underline and strikethrough — added after cursor so they render on top of text
        for row in 0..grid.rows as usize {
            for col in 0..cols as usize {
                let idx = row * cols as usize + col;
                if let Some(cell) = grid.cells.get(idx) {
                    let fg_color = [
                        cell.fg.r() as f32 / 255.0,
                        cell.fg.g() as f32 / 255.0,
                        cell.fg.b() as f32 / 255.0,
                        1.0,
                    ];
                    if cell.underline {
                        rect_instances.push(RectInstance {
                            pos: [col as f32 * cell_w, row as f32 * cell_h + cell_h - 2.0],
                            size: [cell_w, 1.0],
                            color: fg_color,
                        });
                    }
                    if cell.strikethrough {
                        rect_instances.push(RectInstance {
                            pos: [col as f32 * cell_w, row as f32 * cell_h + cell_h * 0.6],
                            size: [cell_w, 1.0],
                            color: fg_color,
                        });
                    }
                }
            }
        }

        // Prepare renderers
        self.rects
            .prepare(&self.device, &self.queue, width, height, &rect_instances);

        let buffer_refs: Vec<&Buffer> = self
            .cached_buffers
            .iter()
            .filter_map(|b| b.as_ref())
            .collect();
        self.glyphon.prepare(
            &self.device,
            &self.queue,
            font_system,
            width,
            height,
            &buffer_refs,
        );

        // Render pass
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("terminal"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(self.bg_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.rects.render(&mut pass);
            self.glyphon.render(&mut pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        // frame is consumed by present(); acquired_texture is now None in wgpu-core.

        if suboptimal {
            // Reconfigure now that no texture is in flight.
            self.surface.configure(&self.device, &self.config);
            return true; // request redraw with fresh config
        }

        false
    }
}

fn build_cursor_rect(
    cursor: &CursorState,
    rows: u16,
    cols: u16,
    cell_w: f32,
    cell_h: f32,
    rects: &mut Vec<RectInstance>,
) {
    if !cursor.visible {
        return;
    }
    if cursor.row < rows as usize && cursor.col < cols as usize {
        use alacritty_terminal::vte::ansi::CursorShape;
        let (cy, ch) = match cursor.shape {
            CursorShape::Block => (cursor.row as f32 * cell_h, cell_h),
            CursorShape::Underline => (cursor.row as f32 * cell_h + cell_h - 2.0, 2.0),
            CursorShape::Beam => (cursor.row as f32 * cell_h, cell_h),
            _ => (cursor.row as f32 * cell_h, cell_h),
        };
        let cw = match cursor.shape {
            CursorShape::Beam => 2.0,
            _ => cell_w,
        };
        rects.push(RectInstance {
            pos: [cursor.col as f32 * cell_w, cy],
            size: [cw, ch],
            color: [0.8, 0.8, 0.8, 0.7],
        });
    }
}
