## ADDED Requirements

### Requirement: CpuRenderer renders terminal cells to a pixel buffer
The `CpuRenderer` SHALL render the terminal cell grid to a `softbuffer::Surface` pixel buffer. Characters SHALL be shaped via cosmic-text and rasterized via `SwashCache::get_image()` to produce glyph alpha bitmaps (`SwashImage`), which are composited with foreground colors and blitted into the softbuffer `Buffer`'s `&mut [Pixel]` slice (accessed via `buffer.pixels()`).

#### Scenario: ASCII text renders correctly via CPU
- **WHEN** the shell outputs plain ASCII text
- **THEN** each character appears at the correct grid position with correct monospace spacing in the softbuffer pixel buffer

#### Scenario: ANSI colors render correctly via CPU
- **WHEN** the shell outputs text with ANSI color codes
- **THEN** foreground and background colors in the pixel buffer match the terminal's color palette

### Requirement: CpuRenderer renders background colors as filled rectangles
Cell background colors SHALL be rendered by filling the cell's pixel region in the buffer with `Pixel::new_rgb(r, g, b)` values. No GPU pipeline is involved.

#### Scenario: Non-default background fills cell region
- **WHEN** a cell has a non-default background color
- **THEN** the corresponding pixel rectangle in the softbuffer buffer is filled with that color using `Pixel::new_rgb()`

### Requirement: CpuRenderer renders cursor
The terminal cursor SHALL be rendered as a filled or partial rectangle in the pixel buffer at the correct grid position, matching the cursor shape (Block, Underline, Beam).

#### Scenario: Block cursor renders as filled cell
- **WHEN** the cursor shape is Block
- **THEN** the entire cell region at the cursor position is filled with the cursor color

### Requirement: CpuRenderer only re-renders dirty rows
The `CpuRenderer` SHALL use `CellGrid.dirty_rows` to skip rows that have not changed since the previous frame. Only dirty rows SHALL have their backgrounds cleared and glyphs re-rasterized.

#### Scenario: Unchanged rows are not re-rendered
- **WHEN** only one row has changed between frames
- **THEN** only that row's pixel region is updated in the buffer; other rows retain their previous pixel data

### Requirement: CpuRenderer handles cursor movement independently of dirty rows
The `CpuRenderer` SHALL track the previous cursor position. When the cursor moves without content changes, the renderer SHALL restore the glyph at the old cursor position (re-blit from cached row data) and draw the cursor at the new position. This SHALL NOT mark the affected rows as dirty.

#### Scenario: Cursor moves without content change
- **WHEN** the cursor moves from row 5 col 10 to row 5 col 15 via arrow keys (no content change)
- **THEN** the old cursor cell is restored to show the underlying glyph, and the new cursor is drawn at col 15 — without re-shaping row 5

#### Scenario: Cursor moves to a different row
- **WHEN** the cursor moves from row 5 to row 10
- **THEN** the old cursor cell on row 5 is restored and the new cursor is drawn on row 10 — without marking either row dirty

### Requirement: CpuRenderer handles window resize
When the window resizes, the `CpuRenderer` SHALL resize the softbuffer surface (via `surface.resize()`), recalculate grid dimensions, and re-render all content.

#### Scenario: Resize produces correct layout
- **WHEN** the window is resized
- **THEN** the softbuffer buffer dimensions match the new window size and the cell grid fills the window correctly

### Requirement: CpuRenderer presents frames via softbuffer
The `CpuRenderer` SHALL acquire a frame buffer via `surface.next_buffer()`, write pixel data, and call `buffer.present()` to display the frame.

#### Scenario: Frame is presented after rendering
- **WHEN** a frame has been rendered to the pixel buffer
- **THEN** `buffer.present()` is called to display the frame on the window surface
