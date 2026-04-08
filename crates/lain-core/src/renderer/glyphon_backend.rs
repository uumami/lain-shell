use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Weight};
use glyphon::{SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};

/// Renders terminal cells using glyphon (cosmic-text + wgpu).
pub struct GlyphonRenderer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub atlas: TextAtlas,
    pub text_renderer: TextRenderer,
    pub viewport: Viewport,
    font_size: f32,
    line_height: f32,
    cell_width: f32,
}

impl GlyphonRenderer {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        let cache = glyphon::Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let viewport = Viewport::new(device, &cache);

        let font_size = 14.0;
        let line_height = 18.0;
        // Approximate monospace cell width
        let cell_width = font_size * 0.6;

        Self {
            font_system,
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

    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        width: u32,
        height: u32,
        cells: &[CellInfo],
        cols: u16,
        rows: u16,
    ) {
        self.viewport
            .update(queue, glyphon::Resolution { width, height });

        // Build one cosmic-text buffer per row, with each cell's character colored
        let metrics = Metrics::new(self.font_size, self.line_height);
        let mut buffers: Vec<Buffer> = Vec::with_capacity(rows as usize);

        for row in 0..rows as usize {
            let mut buf = Buffer::new(&mut self.font_system, metrics);
            buf.set_size(&mut self.font_system, Some(self.cell_width * cols as f32), Some(self.line_height));

            // Build the line text and spans for this row
            let mut line_text = String::with_capacity(cols as usize);
            let mut attrs_list: Vec<(usize, usize, Attrs)> = Vec::new();

            for col in 0..cols as usize {
                let idx = row * cols as usize + col;
                let cell = cells.get(idx);

                let ch = cell.map(|c| c.c).unwrap_or(' ');
                let start = line_text.len();
                line_text.push(ch);
                let end = line_text.len();

                let fg = cell.map(|c| c.fg).unwrap_or(Color::rgb(204, 204, 204));
                let weight = if cell.map(|c| c.bold).unwrap_or(false) {
                    Weight::BOLD
                } else {
                    Weight::NORMAL
                };

                let attrs = Attrs::new()
                    .family(Family::Monospace)
                    .color(fg)
                    .weight(weight);
                attrs_list.push((start, end, attrs));
            }

            buf.set_rich_text(
                &mut self.font_system,
                attrs_list
                    .iter()
                    .map(|(s, e, a)| (&line_text[*s..*e], a.clone())),
                &Attrs::new().family(Family::Monospace),
                Shaping::Advanced,
                None,
            );
            buf.shape_until_scroll(&mut self.font_system, false);
            buffers.push(buf);
        }

        let text_areas: Vec<TextArea<'_>> = buffers
            .iter()
            .enumerate()
            .map(|(row, buf)| TextArea {
                buffer: buf,
                left: 0.0,
                top: row as f32 * self.line_height,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: width as i32,
                    bottom: height as i32,
                },
                default_color: Color::rgb(204, 204, 204),
                custom_glyphs: &[],
            })
            .collect();

        let _ = self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            text_areas,
            &mut self.swash_cache,
        );
    }

    pub fn render<'pass>(
        &'pass self,
        pass: &mut RenderPass<'pass>,
    ) {
        let _ = self.text_renderer.render(&self.atlas, &self.viewport, pass);
    }
}

/// Flattened cell info extracted from alacritty_terminal's Term.
#[derive(Clone)]
pub struct CellInfo {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
}

/// Convert alacritty ANSI color to cosmic-text Color.
pub fn ansi_to_color(color: &AnsiColor, colors: &alacritty_terminal::term::color::Colors) -> Color {
    match color {
        AnsiColor::Named(named) => {
            // Map named colors to default terminal palette
            let (r, g, b) = match named {
                NamedColor::Black => (0, 0, 0),
                NamedColor::Red => (204, 0, 0),
                NamedColor::Green => (78, 154, 6),
                NamedColor::Yellow => (196, 160, 0),
                NamedColor::Blue => (52, 101, 164),
                NamedColor::Magenta => (117, 80, 123),
                NamedColor::Cyan => (6, 152, 154),
                NamedColor::White => (211, 215, 207),
                NamedColor::BrightBlack => (85, 87, 83),
                NamedColor::BrightRed => (239, 41, 41),
                NamedColor::BrightGreen => (138, 226, 52),
                NamedColor::BrightYellow => (252, 233, 79),
                NamedColor::BrightBlue => (114, 159, 207),
                NamedColor::BrightMagenta => (173, 127, 168),
                NamedColor::BrightCyan => (52, 226, 226),
                NamedColor::BrightWhite => (238, 238, 236),
                NamedColor::Foreground => (204, 204, 204),
                NamedColor::Background => (13, 13, 18),
                _ => (204, 204, 204),
            };
            Color::rgb(r, g, b)
        }
        AnsiColor::Spec(rgb) => Color::rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(idx) => {
            // Use the color from the palette if available
            if let Some(rgb) = colors[*idx as usize] {
                Color::rgb(rgb.r, rgb.g, rgb.b)
            } else {
                // Standard 256-color palette
                xterm_256_color(*idx)
            }
        }
    }
}

fn xterm_256_color(idx: u8) -> Color {
    if idx < 16 {
        // Standard colors handled by Named above, but just in case:
        Color::rgb(204, 204, 204)
    } else if idx < 232 {
        // 6x6x6 color cube
        let idx = idx - 16;
        let r = (idx / 36) % 6;
        let g = (idx / 6) % 6;
        let b = idx % 6;
        let to_val = |c: u8| if c == 0 { 0u8 } else { 55 + 40 * c };
        Color::rgb(to_val(r), to_val(g), to_val(b))
    } else {
        // Grayscale ramp
        let val = 8 + 10 * (idx - 232);
        Color::rgb(val, val, val)
    }
}
