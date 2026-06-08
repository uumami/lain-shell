//! CPU/headless rasterizer: GridSnapshot -> in-memory PixelBuffer.
//! Shared text path (cosmic-text + swash) that the GPU backend (Plan 3) reuses.

use cosmic_text::{fontdb, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use lain_types::GridSnapshot;

const BUNDLED_FONT: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");

/// Packed 0x00RRGGBB, opaque. Row-major, len == width * height.
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>,
}

impl PixelBuffer {
    fn filled(width: u32, height: u32, color: u32) -> Self {
        PixelBuffer { width, height, data: vec![color; (width * height) as usize] }
    }
    /// Pixel at (x, y); panics if out of range (test helper / internal use).
    pub fn at(&self, x: u32, y: u32) -> u32 {
        self.data[(y * self.width + x) as usize]
    }
}

pub trait Renderer {
    fn render(&mut self, grid: &GridSnapshot) -> PixelBuffer;
}

pub struct CpuRenderer {
    font_system: FontSystem,
    swash: SwashCache,
    metrics: Metrics,
    cell_w: u32,
    cell_h: u32,
    fg: Color,
    bg: u32,
}

impl CpuRenderer {
    pub fn new(font_size: f32) -> Self {
        // Bundled font only -> skip system enumeration (~18 MB PSS + ~110 ms saved).
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
        let font_system = FontSystem::new_with_locale_and_db("en-US".into(), db);

        let line_height = (font_size * 1.2).ceil();
        CpuRenderer {
            font_system,
            swash: SwashCache::new(),
            metrics: Metrics::new(font_size, line_height),
            cell_w: (font_size * 0.6).ceil() as u32,
            cell_h: line_height as u32,
            fg: Color::rgb(220, 220, 220),
            bg: 0x0d0d0f,
        }
    }

    fn canvas_size(&self, grid: &GridSnapshot) -> (u32, u32) {
        (
            (grid.cols as u32 * self.cell_w).max(1),
            (grid.lines as u32 * self.cell_h).max(1),
        )
    }
}

impl Renderer for CpuRenderer {
    fn render(&mut self, grid: &GridSnapshot) -> PixelBuffer {
        let (width, height) = self.canvas_size(grid);
        PixelBuffer::filled(width, height, self.bg)
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
    fn blank_grid_is_all_background() {
        let mut r = CpuRenderer::new(14.0);
        let pb = r.render(&snapshot(10, 3, &[]));
        assert_eq!(pb.width, 10 * r.cell_w);
        assert_eq!(pb.height, 3 * r.cell_h);
        assert!(pb.data.iter().all(|&p| p == 0x0d0d0f), "blank grid must be all bg");
    }
}
