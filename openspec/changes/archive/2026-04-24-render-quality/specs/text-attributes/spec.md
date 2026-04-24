## ADDED Requirements

### Requirement: CellInfo carries italic, underline, strikethrough, and dim flags
The `CellInfo` struct SHALL include four new boolean fields: `italic`, `underline`, `strikethrough`, and `dim`. These SHALL be populated from `alacritty_terminal::term::cell::Flags` during `CellGrid::extract()`.

#### Scenario: Terminal cell with italic flag set
- **WHEN** `Flags::ITALIC` is set on an alacritty_terminal cell
- **THEN** `CellInfo::italic` is true for that cell after extraction

#### Scenario: Terminal cell with no special flags
- **WHEN** a cell has no attribute flags set
- **THEN** all four new CellInfo fields are false

### Requirement: Italic text is rendered using the italic font variant
When `CellInfo::italic` is true, the corresponding text run SHALL be shaped with `cosmic_text::Style::Italic`. This applies in both the CPU and GPU text shaping paths.

#### Scenario: Italic attribute rendered differently from normal
- **WHEN** a cell has `italic = true`
- **THEN** the glyph is drawn with the italic style from the monospace font family

### Requirement: Dim text uses reduced fg brightness
When `CellInfo::dim` is true, the foreground color SHALL be reduced to 60% of its original brightness before rendering. The background color is unaffected.

#### Scenario: Dim cell has muted foreground
- **WHEN** a cell has `dim = true` and fg `Color::rgb(204, 204, 204)`
- **THEN** the rendered fg color is approximately `Color::rgb(122, 122, 122)`

### Requirement: Underline is drawn as a 1px rect at the cell baseline
When `CellInfo::underline` is true, a 1-pixel horizontal rect SHALL be drawn at `y = row * cell_h + cell_h - 2` with width `cell_w`, using the cell's fg color. This SHALL be rendered in both CPU and GPU paths.

#### Scenario: Underline rect position
- **WHEN** a cell at row 3, col 5 has `underline = true`, cell_h = 18, cell_w = 8
- **THEN** a rect is drawn at pixel (40, 70) with size (8, 1)

### Requirement: Strikethrough is drawn as a 1px rect at the cell midline
When `CellInfo::strikethrough` is true, a 1-pixel horizontal rect SHALL be drawn at `y = row * cell_h + (cell_h as f32 * 0.6) as u32` with width `cell_w`, using the cell's fg color. This SHALL be rendered in both CPU and GPU paths.

#### Scenario: Strikethrough rect position
- **WHEN** a cell at row 0, col 0 has `strikethrough = true`, cell_h = 18, cell_w = 8
- **THEN** a rect is drawn at pixel (0, 10) with size (8, 1)

### Requirement: Underline and strikethrough rects are drawn after glyphs
In both renderers, underline and strikethrough rects SHALL be drawn after (on top of) the glyph pixels. In the GPU renderer this means they are added as `RectInstance` entries rendered after text. In the CPU renderer they are filled after the glyph pass for that row.

#### Scenario: Rect visible over glyph descenders
- **WHEN** a cell has both a glyph and `underline = true`
- **THEN** the underline rect is not obscured by the glyph's descender pixels
