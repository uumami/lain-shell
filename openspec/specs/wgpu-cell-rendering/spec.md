## Purpose
Define how lain-shell renders terminal cells through the wgpu-backed renderer.
## Requirements
### Requirement: Cell grid renders via wgpu
lain-core SHALL render the terminal cell grid to a wgpu surface. Each cell's character SHALL be shaped via cosmic-text, rasterized into a GPU texture atlas (managed by glyphon), and drawn as an instanced quad.

#### Scenario: ASCII text renders correctly
- **WHEN** the shell outputs plain ASCII text (e.g., `ls` output)
- **THEN** each character appears at the correct grid position with correct spacing

#### Scenario: ANSI colors render correctly
- **WHEN** the shell outputs text with ANSI color codes (e.g., colored `ls`, colored prompt)
- **THEN** foreground and background colors match the terminal's 256-color and true-color palette

#### Scenario: Bold and italic render
- **WHEN** text has bold or italic attributes
- **THEN** the corresponding font variant is used (bold weight, italic slant)

### Requirement: Background colors render as filled quads
Cell background colors SHALL be rendered as filled rectangles via a wgpu quad pipeline, separate from the glyph rendering pass.

#### Scenario: Selection or highlighted backgrounds
- **WHEN** cells have non-default background colors
- **THEN** a colored rectangle fills the cell area behind the glyph

### Requirement: Cursor renders visibly
The terminal cursor SHALL be rendered at the correct grid position with a visible indicator (block, underline, or bar depending on cursor shape mode).

#### Scenario: Cursor position tracks input
- **WHEN** the user types characters
- **THEN** the cursor advances one cell per character and the rendered cursor position matches

#### Scenario: Cursor blink (stretch goal)
- **WHEN** the terminal is idle
- **THEN** the cursor blinks at a visible interval (optional for spike)

### Requirement: Text rendering behind internal trait
The text rendering implementation SHALL be behind an internal `TextRenderer` trait within lain-core. This trait SHALL define `prepare`, `render`, and `resize` methods. The glyphon-based implementation SHALL be one implementor of this trait.

#### Scenario: GlyphonRenderer implements TextRenderer
- **WHEN** inspecting lain-core's renderer module
- **THEN** a `TextRenderer` trait exists and `GlyphonRenderer` implements it

#### Scenario: Swapping renderer requires no changes outside renderer module
- **WHEN** replacing `GlyphonRenderer` with a different `TextRenderer` implementation
- **THEN** only files within `crates/lain-core/src/renderer/` need to change

### Requirement: Window resize triggers re-render
When the window resizes, the wgpu surface SHALL be reconfigured, the cell grid dimensions SHALL be recalculated, and the content SHALL re-render at the new size.

#### Scenario: Resize produces correct layout
- **WHEN** the window is resized
- **THEN** the number of visible columns and rows adjusts to fill the new window size with correctly sized cells

### Requirement: Surface uses vsync presentation mode
The wgpu surface SHALL be configured with `PresentMode::AutoVsync` instead of `PresentMode::AutoNoVsync`. Frame presentation SHALL be synchronized to the display refresh cycle to eliminate tearing and provide consistent frame timing.

#### Scenario: Surface configured at initialization
- **WHEN** `GpuRenderer::new()` runs
- **THEN** `SurfaceConfiguration::present_mode` is `PresentMode::AutoVsync`

#### Scenario: Surface reconfigured after resize
- **WHEN** `GpuRenderer::resize()` is called
- **THEN** the reconfigured surface retains `PresentMode::AutoVsync`

### Requirement: Cell dimensions come from font-metrics measurement
The `GlyphonRenderer` SHALL NOT compute `cell_width` as `font_size * 0.6`. Instead, `cell_width` SHALL be set from the measured font advance as defined in the `font-metrics` capability.

#### Scenario: Cell width at initialization
- **WHEN** `GlyphonRenderer::new()` is called
- **THEN** `cell_width` is the measured advance of 'M' at the physical font size, rounded to integer pixels

