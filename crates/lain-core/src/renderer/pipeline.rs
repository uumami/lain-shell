use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use std::sync::Arc;
use wgpu::{
    CommandEncoder, Device, Operations, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, TextureFormat, TextureView,
};

use crate::terminal::JsonLessListener;
use super::glyphon_backend::{ansi_to_color, CellInfo, GlyphonRenderer};
use super::rect::{RectInstance, RectRenderer};

/// Orchestrates the full render pipeline: backgrounds → text → cursor.
pub struct RenderPipeline {
    pub glyphon: GlyphonRenderer,
    pub rects: RectRenderer,
    bg_color: wgpu::Color,
}

impl RenderPipeline {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        Self {
            glyphon: GlyphonRenderer::new(device, queue, format),
            rects: RectRenderer::new(device, format),
            bg_color: wgpu::Color {
                r: 0.05,
                g: 0.05,
                b: 0.07,
                a: 1.0,
            },
        }
    }

    pub fn cell_metrics(&self) -> (f32, f32) {
        self.glyphon.cell_metrics()
    }

    pub fn render_frame(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        width: u32,
        height: u32,
        term: &Arc<FairMutex<Term<JsonLessListener>>>,
    ) {
        let (cell_w, cell_h) = self.glyphon.cell_metrics();
        let cols = (width as f32 / cell_w).max(1.0) as u16;
        let rows = (height as f32 / cell_h).max(1.0) as u16;

        // Extract cell data while holding the lock
        let (cells, cursor_point, cursor_shape) = {
            let term = term.lock();
            let content = term.renderable_content();

            let mut cells = vec![
                CellInfo {
                    c: ' ',
                    fg: cosmic_text::Color::rgb(204, 204, 204),
                    bg: cosmic_text::Color::rgba(0, 0, 0, 0),
                    bold: false,
                };
                cols as usize * rows as usize
            ];

            let colors = content.colors;
            for indexed in content.display_iter {
                let col = indexed.point.column.0;
                let line = indexed.point.line.0;
                if line < 0 {
                    continue;
                }
                let row = line as usize;
                if row < rows as usize && col < cols as usize {
                    let idx = row * cols as usize + col;
                    let cell = &indexed.cell;
                    cells[idx] = CellInfo {
                        c: cell.c,
                        fg: ansi_to_color(&cell.fg, colors),
                        bg: ansi_to_color(&cell.bg, colors),
                        bold: cell
                            .flags
                            .contains(alacritty_terminal::term::cell::Flags::BOLD),
                    };
                }
            }

            let cursor = content.cursor;
            (cells, cursor.point, cursor.shape)
        };
        // Term lock released here

        // Build background rects
        let mut rect_instances: Vec<RectInstance> = Vec::new();
        let bg_default = cosmic_text::Color::rgba(0, 0, 0, 0);
        for row in 0..rows as usize {
            for col in 0..cols as usize {
                let idx = row * cols as usize + col;
                let cell = &cells[idx];
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

        // Cursor rect
        let cursor_col = cursor_point.column.0 as usize;
        let cursor_row = cursor_point.line.0;
        if cursor_row >= 0 {
            let cursor_row = cursor_row as usize;
            if cursor_row < rows as usize && cursor_col < cols as usize {
                let (cy, ch) = match cursor_shape {
                    alacritty_terminal::vte::ansi::CursorShape::Block => {
                        (cursor_row as f32 * cell_h, cell_h)
                    }
                    alacritty_terminal::vte::ansi::CursorShape::Underline => {
                        (cursor_row as f32 * cell_h + cell_h - 2.0, 2.0)
                    }
                    alacritty_terminal::vte::ansi::CursorShape::Beam => {
                        (cursor_row as f32 * cell_h, cell_h)
                    }
                    _ => (cursor_row as f32 * cell_h, cell_h),
                };
                let cw = match cursor_shape {
                    alacritty_terminal::vte::ansi::CursorShape::Beam => 2.0,
                    _ => cell_w,
                };
                rect_instances.push(RectInstance {
                    pos: [cursor_col as f32 * cell_w, cy],
                    size: [cw, ch],
                    color: [0.8, 0.8, 0.8, 0.7],
                });
            }
        }

        // Prepare renderers
        self.rects.prepare(device, queue, width, height, &rect_instances);
        self.glyphon.prepare(device, queue, width, height, &cells, cols, rows);

        // Render pass
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("terminal"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
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

            // Draw backgrounds first, then text on top
            self.rects.render(&mut pass);
            self.glyphon.render(&mut pass);
        }
    }
}
