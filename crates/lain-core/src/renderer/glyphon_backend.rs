use cosmic_text::{Buffer, Color, FontSystem};
use glyphon::{SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};

use super::cell_grid::CellInfo;
use super::text_shaping::measure_cell_width;

const LOGICAL_FONT_SIZE: f32 = 14.0;
const LOGICAL_LINE_HEIGHT: f32 = 18.0;

/// Renders terminal cells using glyphon (cosmic-text + wgpu).
pub struct GlyphonRenderer {
    pub swash_cache: SwashCache,
    pub atlas: TextAtlas,
    pub text_renderer: TextRenderer,
    pub viewport: Viewport,
    font_size: f32,
    line_height: f32,
    cell_width: f32,
}

impl GlyphonRenderer {
    pub fn new_with_scale(
        device: &Device,
        queue: &Queue,
        format: TextureFormat,
        scale_factor: f32,
        font_system: &mut FontSystem,
    ) -> Self {
        let swash_cache = SwashCache::new();
        let cache = glyphon::Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let viewport = Viewport::new(device, &cache);

        let font_size = LOGICAL_FONT_SIZE * scale_factor;
        let line_height = LOGICAL_LINE_HEIGHT * scale_factor;
        let cell_width = measure_cell_width(font_system, font_size);

        Self {
            swash_cache,
            atlas,
            text_renderer,
            viewport,
            font_size,
            line_height,
            cell_width,
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
    }

    /// Rebuild a cosmic-text Buffer for a single row from CellInfo data.
    pub fn build_row_buffer(
        &self,
        font_system: &mut FontSystem,
        cells: &[CellInfo],
        cols: u16,
    ) -> Buffer {
        super::text_shaping::build_row_buffer(
            font_system,
            cells,
            cols,
            self.font_size,
            self.line_height,
            self.cell_width,
        )
    }

    /// Prepare text rendering from pre-built Buffers.
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        font_system: &mut FontSystem,
        width: u32,
        height: u32,
        buffers: &[&Buffer],
    ) {
        self.viewport
            .update(queue, glyphon::Resolution { width, height });

        let text_areas: Vec<TextArea<'_>> = buffers
            .iter()
            .enumerate()
            .map(|(row, buf)| {
                // Round row Y to the nearest pixel so sub-pixel line_height values
                // don't cause glyphs to drift from their background rects over time.
                let row_top = (row as f32 * self.line_height).round() as i32;
                let row_bot = ((row as f32 + 1.0) * self.line_height).round() as i32;
                TextArea {
                    buffer: *buf,
                    left: 0.0,
                    top: row_top as f32,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: 0,
                        top: row_top,
                        right: width as i32,
                        bottom: row_bot,
                    },
                    default_color: Color::rgb(204, 204, 204),
                    custom_glyphs: &[],
                }
            })
            .collect();

        let _ = self.text_renderer.prepare(
            device,
            queue,
            font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        );
    }

    pub fn render<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let _ = self.text_renderer.render(&self.atlas, &self.viewport, pass);
    }
}
