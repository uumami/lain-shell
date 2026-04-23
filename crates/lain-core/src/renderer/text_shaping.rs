use cosmic_text::{Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, Weight};

use super::cell_grid::CellInfo;

/// Measure the actual advance width of the monospace font at the given physical
/// font size by laying out 10 'M' characters and dividing. Returns the width
/// rounded to the nearest integer pixel. Falls back to `font_size * 0.6` if
/// measurement fails (e.g., font not found).
pub fn measure_cell_width(font_system: &mut FontSystem, font_size: f32) -> f32 {
    let line_height = font_size * 1.3; // generous height for the probe buffer
    let metrics = Metrics::new(font_size, line_height);
    let mut buf = Buffer::new(font_system, metrics);
    buf.set_size(font_system, Some(font_size * 20.0), Some(line_height));
    let probe_attrs = Attrs::new().family(Family::Monospace);
    buf.set_text(
        font_system,
        "MMMMMMMMMM",
        &probe_attrs,
        Shaping::Basic,
        None,
    );
    buf.shape_until_scroll(font_system, false);
    let measured = buf
        .layout_runs()
        .next()
        .map(|run| run.line_w / 10.0)
        .unwrap_or(0.0);
    // Return the fractional advance width directly — rounding here causes cols to be
    // over-counted (when rounding down) which makes the row buffer narrower than the
    // text the font actually produces, truncating rightmost columns in htop etc.
    if measured > 0.0 {
        measured
    } else {
        font_size * 0.6
    }
}

/// Returns true if any cell in the row contains a character that requires
/// HarfBuzz shaping (complex scripts, bidirectional text, combining marks).
/// Returns false for ASCII, Latin, CJK, box-drawing, and other simple scripts
/// where Shaping::Basic is correct and significantly faster.
pub(crate) fn needs_advanced_shaping(cells: &[CellInfo]) -> bool {
    cells.iter().any(|cell| {
        let c = cell.c as u32;
        // Combining diacritical marks
        (c >= 0x0300 && c <= 0x036F)
            // Hebrew
            || (c >= 0x0590 && c <= 0x05FF)
            // Arabic
            || (c >= 0x0600 && c <= 0x06FF)
            // Syriac / Thaana
            || (c >= 0x0700 && c <= 0x07BF)
            // Devanagari and other Indic scripts (Bengali, Gurmukhi, Gujarati,
            // Oriya, Tamil, Telugu, Kannada, Malayalam, Sinhala)
            || (c >= 0x0900 && c <= 0x0DFF)
            // Thai, Lao, Tibetan
            || (c >= 0x0E00 && c <= 0x0FFF)
            // Emoji, ZWJ sequences, and supplementary multilingual plane
            || c >= 0x1F000
    })
}

/// Build a cosmic-text Buffer for a single row from CellInfo data.
/// Automatically selects Shaping::Basic for simple-script rows and
/// Shaping::Advanced for rows containing complex scripts.
pub(crate) fn build_row_buffer(
    font_system: &mut FontSystem,
    cells: &[CellInfo],
    cols: u16,
    font_size: f32,
    line_height: f32,
    cell_width: f32,
) -> Buffer {
    let shaping = if needs_advanced_shaping(cells) {
        Shaping::Advanced
    } else {
        Shaping::Basic
    };

    let metrics = Metrics::new(font_size, line_height);
    let mut buf = Buffer::new(font_system, metrics);
    buf.set_size(
        font_system,
        Some(cell_width * cols as f32),
        Some(line_height),
    );

    let mut line_text = String::with_capacity(cols as usize);
    let mut attrs_list: Vec<(usize, usize, Attrs)> = Vec::new();

    for cell in cells {
        let start = line_text.len();
        // Null bytes (wide-char placeholders, uninitialized cells) must not reach
        // cosmic-text — it renders them as visible replacement-character tofu boxes.
        line_text.push(if cell.c == '\0' { ' ' } else { cell.c });
        let end = line_text.len();

        let weight = if cell.bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        };
        let style = if cell.italic {
            Style::Italic
        } else {
            Style::Normal
        };
        let fg = if cell.dim {
            // Dim: reduce brightness to 60%
            Color::rgba(
                (cell.fg.r() as f32 * 0.6) as u8,
                (cell.fg.g() as f32 * 0.6) as u8,
                (cell.fg.b() as f32 * 0.6) as u8,
                cell.fg.a(),
            )
        } else {
            cell.fg
        };

        let attrs = Attrs::new()
            .family(Family::Monospace)
            .color(fg)
            .weight(weight)
            .style(style);
        attrs_list.push((start, end, attrs));
    }

    buf.set_rich_text(
        font_system,
        attrs_list
            .iter()
            .map(|(s, e, a)| (&line_text[*s..*e], a.clone())),
        &Attrs::new().family(Family::Monospace),
        shaping,
        None,
    );
    buf.shape_until_scroll(font_system, false);
    buf
}
