use std::sync::Arc;

use cosmic_text::{Buffer, Color, FontSystem, SwashCache, SwashContent};
use softbuffer::{Context, Surface};
use winit::window::Window;

use super::cell_grid::{CellGrid, CellInfo, CursorState};
use super::text_shaping;
use super::text_shaping::measure_cell_width;

const LOGICAL_FONT_SIZE: f32 = 14.0;
const LOGICAL_LINE_HEIGHT: f32 = 18.0;

pub struct CpuRenderer {
    _context: Context<Arc<Window>>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    swash_cache: SwashCache,
    cached_buffers: Vec<Option<Buffer>>,
    prev_cursor: Option<CursorState>,
    width: u32,
    height: u32,
    font_size: f32,
    line_height: f32,
    cell_width: f32,
    // Persistent pixel buffer for dirty-row optimization
    pixels: Vec<u32>,
}

impl CpuRenderer {
    pub fn new(window: Arc<Window>, scale_factor: f32, font_system: &mut FontSystem) -> Self {
        let context =
            softbuffer::Context::new(window.clone()).expect("failed to create softbuffer context");
        let surface =
            Surface::new(&context, window.clone()).expect("failed to create softbuffer surface");

        let size = window.inner_size();
        let font_size = LOGICAL_FONT_SIZE * scale_factor;
        let line_height = LOGICAL_LINE_HEIGHT * scale_factor;
        let cell_width = measure_cell_width(font_system, font_size);

        Self {
            _context: context,
            surface,
            swash_cache: SwashCache::new(),
            cached_buffers: Vec::new(),
            prev_cursor: None,
            width: size.width.max(1),
            height: size.height.max(1),
            font_size,
            line_height,
            cell_width,
            pixels: Vec::new(),
        }
    }

    pub fn cell_metrics(&self) -> (f32, f32) {
        (self.cell_width, self.line_height)
    }

    /// Update font metrics for a new scale factor without recreating the surface.
    pub fn update_scale(&mut self, scale_factor: f32, font_system: &mut FontSystem) {
        self.font_size = LOGICAL_FONT_SIZE * scale_factor;
        self.line_height = LOGICAL_LINE_HEIGHT * scale_factor;
        self.cell_width = measure_cell_width(font_system, self.font_size);
        self.cached_buffers.clear();
        self.pixels.clear(); // force full repaint at new size
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.cached_buffers.clear();
        self.pixels.clear();
    }

    pub fn render_frame(&mut self, grid: &CellGrid, font_system: &mut FontSystem) {
        let width = self.width;
        let height = self.height;
        let pixel_count = (width * height) as usize;

        // Initialize pixel buffer if needed
        if self.pixels.len() != pixel_count {
            // Background color: rgb(13, 13, 18) → 0x00_0D_0D_12
            let bg = pack_rgb(13, 13, 18);
            self.pixels = vec![bg; pixel_count];
        }

        let (cell_w, cell_h) = (self.cell_width, self.line_height);
        let cols = grid.cols as usize;
        let rows = grid.rows as usize;

        // Ensure cached_buffers has correct size
        if self.cached_buffers.len() != rows {
            self.cached_buffers.clear();
            self.cached_buffers.resize_with(rows, || None);
        }

        // Rebuild only dirty row Buffers
        for row in 0..rows {
            if grid.dirty_rows.get(row).copied().unwrap_or(true) {
                let start = row * cols;
                let end = (start + cols).min(grid.cells.len());
                let row_cells = &grid.cells[start..end];
                let buf = text_shaping::build_row_buffer(
                    font_system,
                    row_cells,
                    grid.cols,
                    self.font_size,
                    self.line_height,
                    self.cell_width,
                );
                self.cached_buffers[row] = Some(buf);

                // Clear this row's pixel region to background
                let py_start = (row as f32 * cell_h) as u32;
                let py_end = ((row as f32 + 1.0) * cell_h) as u32;
                let bg = pack_rgb(13, 13, 18);
                for py in py_start..py_end.min(height) {
                    let line_start = (py * width) as usize;
                    let line_end = ((py + 1) * width) as usize;
                    if line_end <= self.pixels.len() {
                        self.pixels[line_start..line_end].fill(bg);
                    }
                }

                // Render cell backgrounds for this row.
                // Derive each cell's pixel x/width from adjacent rounded boundaries so
                // fractional cell_w values tile seamlessly (no single-pixel gaps).
                let row_y = (row as f32 * cell_h).round() as u32;
                let row_h = ((row as f32 + 1.0) * cell_h).round() as u32 - row_y;
                for col in 0..cols {
                    let idx = row * cols + col;
                    if let Some(cell) = grid.cells.get(idx) {
                        let bg_default = Color::rgba(0, 0, 0, 0);
                        if cell.bg != bg_default && cell.bg.a() > 2 {
                            let px_x = (col as f32 * cell_w).round() as u32;
                            let px_w = ((col as f32 + 1.0) * cell_w).round() as u32 - px_x;
                            let color = pack_rgb(cell.bg.r(), cell.bg.g(), cell.bg.b());
                            fill_rect(
                                &mut self.pixels,
                                width,
                                height,
                                px_x,
                                row_y,
                                px_w,
                                row_h,
                                color,
                            );
                        }
                    }
                }

                // Render glyphs for this row
                if let Some(buf) = &self.cached_buffers[row] {
                    render_buffer_glyphs(
                        buf,
                        font_system,
                        &mut self.swash_cache,
                        &grid.cells[start..end],
                        row,
                        cell_w,
                        cell_h,
                        &mut self.pixels,
                        width,
                        height,
                    );
                }

                // Underline and strikethrough — drawn after glyphs so they're on top
                for col in 0..cols {
                    let idx = row * cols + col;
                    if let Some(cell) = grid.cells.get(idx) {
                        if cell.underline || cell.strikethrough {
                            let px_x = (col as f32 * cell_w) as u32;
                            let fg = pack_rgb(cell.fg.r(), cell.fg.g(), cell.fg.b());
                            if cell.underline {
                                let px_y = (row as f32 * cell_h + cell_h - 2.0) as u32;
                                fill_rect(
                                    &mut self.pixels,
                                    width,
                                    height,
                                    px_x,
                                    px_y,
                                    cell_w as u32,
                                    1,
                                    fg,
                                );
                            }
                            if cell.strikethrough {
                                let px_y = (row as f32 * cell_h + cell_h * 0.6) as u32;
                                fill_rect(
                                    &mut self.pixels,
                                    width,
                                    height,
                                    px_x,
                                    px_y,
                                    cell_w as u32,
                                    1,
                                    fg,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Restore previous cursor cell (if cursor moved)
        if let Some(prev) = &self.prev_cursor {
            if prev.row != grid.cursor.row || prev.col != grid.cursor.col {
                // Re-render the cell at the old cursor position
                if prev.row < rows && prev.col < cols {
                    let idx = prev.row * cols + prev.col;
                    if let Some(cell) = grid.cells.get(idx) {
                        let px_x = (prev.col as f32 * cell_w).round() as u32;
                        let px_y = (prev.row as f32 * cell_h).round() as u32;
                        let px_w = ((prev.col as f32 + 1.0) * cell_w).round() as u32 - px_x;
                        let px_h = ((prev.row as f32 + 1.0) * cell_h).round() as u32 - px_y;

                        // Clear with background
                        let bg_color = if cell.bg.a() > 2 {
                            pack_rgb(cell.bg.r(), cell.bg.g(), cell.bg.b())
                        } else {
                            pack_rgb(13, 13, 18)
                        };
                        fill_rect(
                            &mut self.pixels,
                            width,
                            height,
                            px_x,
                            px_y,
                            px_w,
                            px_h,
                            bg_color,
                        );

                        // Re-render the glyph
                        if let Some(buf) = &self.cached_buffers[prev.row] {
                            render_buffer_glyphs(
                                buf,
                                font_system,
                                &mut self.swash_cache,
                                &grid.cells[prev.row * cols
                                    ..(prev.row * cols + cols).min(grid.cells.len())],
                                prev.row,
                                cell_w,
                                cell_h,
                                &mut self.pixels,
                                width,
                                height,
                            );
                        }
                    }
                }
            }
        }

        // Draw cursor
        if grid.cursor.visible && grid.cursor.row < rows && grid.cursor.col < cols {
            use alacritty_terminal::vte::ansi::CursorShape;
            let cursor_color = pack_rgba(204, 204, 204, 178); // ~0.7 alpha
            let px_x = (grid.cursor.col as f32 * cell_w).round() as u32;
            let cell_px_y = (grid.cursor.row as f32 * cell_h).round() as u32;
            let cell_px_h = ((grid.cursor.row as f32 + 1.0) * cell_h).round() as u32 - cell_px_y;
            let cell_px_w = ((grid.cursor.col as f32 + 1.0) * cell_w).round() as u32 - px_x;
            let (px_y, px_h) = match grid.cursor.shape {
                CursorShape::Block => (cell_px_y, cell_px_h),
                CursorShape::Underline => (cell_px_y + cell_px_h.saturating_sub(2), 2u32),
                CursorShape::Beam => (cell_px_y, cell_px_h),
                _ => (cell_px_y, cell_px_h),
            };
            let px_w = match grid.cursor.shape {
                CursorShape::Beam => 2u32,
                _ => cell_px_w,
            };
            blend_rect(
                &mut self.pixels,
                width,
                height,
                px_x,
                px_y,
                px_w,
                px_h,
                cursor_color,
            );
        }
        self.prev_cursor = Some(grid.cursor.clone());

        // Present
        self.surface
            .resize(
                std::num::NonZeroU32::new(width).unwrap(),
                std::num::NonZeroU32::new(height).unwrap(),
            )
            .expect("failed to resize softbuffer surface");

        let mut buffer = self
            .surface
            .buffer_mut()
            .expect("failed to get softbuffer buffer");
        let len = buffer.len().min(self.pixels.len());
        buffer[..len].copy_from_slice(&self.pixels[..len]);
        buffer
            .present()
            .expect("failed to present softbuffer buffer");
    }
}

fn render_buffer_glyphs(
    buf: &Buffer,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    _cells: &[CellInfo],
    row: usize,
    _cell_w: f32,
    cell_h: f32,
    pixels: &mut [u32],
    width: u32,
    height: u32,
) {
    let base_y = (row as f32 * cell_h) as i32;

    for run in buf.layout_runs() {
        for glyph in run.glyphs.iter() {
            let physical = glyph.physical((0.0, run.line_y), 1.0);

            let Some(image) = swash_cache.get_image(font_system, physical.cache_key) else {
                continue;
            };

            let glyph_x = physical.x + image.placement.left;
            let glyph_y = base_y + physical.y - image.placement.top;
            let glyph_w = image.placement.width as i32;
            let glyph_h = image.placement.height as i32;

            // Get foreground color from the glyph's color attribute
            let fg_color = glyph.color_opt.unwrap_or(Color::rgb(204, 204, 204));
            let fg_r = fg_color.r();
            let fg_g = fg_color.g();
            let fg_b = fg_color.b();

            match image.content {
                SwashContent::Mask => {
                    // Alpha mask — composite with foreground color
                    for gy in 0..glyph_h {
                        for gx in 0..glyph_w {
                            let px = glyph_x + gx;
                            let py = glyph_y + gy;
                            if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                                continue;
                            }
                            let alpha = image.data[(gy * glyph_w + gx) as usize];
                            if alpha == 0 {
                                continue;
                            }
                            let idx = (py as u32 * width + px as u32) as usize;
                            if idx < pixels.len() {
                                let bg = pixels[idx];
                                pixels[idx] = alpha_blend(bg, fg_r, fg_g, fg_b, alpha);
                            }
                        }
                    }
                }
                SwashContent::Color => {
                    // Full color glyph (e.g., emoji)
                    for gy in 0..glyph_h {
                        for gx in 0..glyph_w {
                            let px = glyph_x + gx;
                            let py = glyph_y + gy;
                            if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                                continue;
                            }
                            let offset = (gy * glyph_w + gx) as usize * 4;
                            if offset + 3 < image.data.len() {
                                let r = image.data[offset];
                                let g = image.data[offset + 1];
                                let b = image.data[offset + 2];
                                let a = image.data[offset + 3];
                                let idx = (py as u32 * width + px as u32) as usize;
                                if idx < pixels.len() {
                                    pixels[idx] = alpha_blend(pixels[idx], r, g, b, a);
                                }
                            }
                        }
                    }
                }
                SwashContent::SubpixelMask => {
                    // Subpixel: treat as mask for now (simplification)
                    for gy in 0..glyph_h {
                        for gx in 0..glyph_w {
                            let px = glyph_x + gx;
                            let py = glyph_y + gy;
                            if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                                continue;
                            }
                            let offset = (gy * glyph_w + gx) as usize * 3;
                            if offset + 2 < image.data.len() {
                                // Use green channel as alpha (middle subpixel)
                                let alpha = image.data[offset + 1];
                                if alpha == 0 {
                                    continue;
                                }
                                let idx = (py as u32 * width + px as u32) as usize;
                                if idx < pixels.len() {
                                    pixels[idx] = alpha_blend(pixels[idx], fg_r, fg_g, fg_b, alpha);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[inline]
fn pack_rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

#[inline]
fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
}

fn fill_rect(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: u32,
) {
    for py in y..(y + h).min(height) {
        for px in x..(x + w).min(width) {
            let idx = (py * width + px) as usize;
            if idx < pixels.len() {
                pixels[idx] = color;
            }
        }
    }
}

fn blend_rect(
    pixels: &mut [u32],
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: u32,
) {
    let a = ((color >> 24) & 0xFF) as u8;
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    for py in y..(y + h).min(height) {
        for px in x..(x + w).min(width) {
            let idx = (py * width + px) as usize;
            if idx < pixels.len() {
                pixels[idx] = alpha_blend(pixels[idx], r, g, b, a);
            }
        }
    }
}

#[inline]
fn alpha_blend(bg: u32, fg_r: u8, fg_g: u8, fg_b: u8, alpha: u8) -> u32 {
    if alpha == 255 {
        return pack_rgb(fg_r, fg_g, fg_b);
    }
    let bg_r = ((bg >> 16) & 0xFF) as u16;
    let bg_g = ((bg >> 8) & 0xFF) as u16;
    let bg_b = (bg & 0xFF) as u16;
    let a = alpha as u16;
    let inv_a = 255 - a;
    let r = ((fg_r as u16 * a + bg_r * inv_a) / 255) as u8;
    let g = ((fg_g as u16 * a + bg_g * inv_a) / 255) as u8;
    let b = ((fg_b as u16 * a + bg_b * inv_a) / 255) as u8;
    pack_rgb(r, g, b)
}
