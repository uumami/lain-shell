## MODIFIED Requirements

### Requirement: Text rendering behind internal trait
The text rendering implementation SHALL be wrapped inside a `GpuRenderer` struct that is one variant of the `Renderer` enum. The `GpuRenderer` SHALL contain `GlyphonRenderer` and `RectRenderer` as internal implementation details not exposed outside the renderer module.

#### Scenario: GpuRenderer wraps glyphon and rect renderers
- **WHEN** inspecting lain-core's renderer module
- **THEN** `GpuRenderer` struct exists containing `GlyphonRenderer` and `RectRenderer`, and neither is publicly accessible outside the module

#### Scenario: Swapping renderer requires no changes outside renderer module
- **WHEN** replacing `GlyphonRenderer` with a different text rendering implementation inside `GpuRenderer`
- **THEN** only files within `crates/lain-core/src/renderer/` need to change

## ADDED Requirements

### Requirement: GpuRenderer caches cosmic-text Buffers per row
The `GpuRenderer` SHALL maintain a `Vec<Buffer>` with one cosmic-text Buffer per terminal row. On each frame, only Buffers for rows marked dirty in `CellGrid.dirty_rows` SHALL be rebuilt (via `set_rich_text` + `shape_until_scroll`). Unchanged rows SHALL reuse their existing Buffer.

#### Scenario: Cached buffers reused for unchanged rows
- **WHEN** a frame has 24 rows and only 2 rows are dirty
- **THEN** only 2 Buffers are rebuilt; the other 22 are reused from the previous frame

#### Scenario: All buffers rebuilt on resize
- **WHEN** the terminal grid size changes
- **THEN** the Buffer cache is cleared and all Buffers are rebuilt on the next frame

### Requirement: GpuRenderer consumes CellGrid
The `GpuRenderer::render_frame()` method SHALL accept a `&CellGrid` (with dirty flags and cursor state) instead of directly locking and extracting from `Arc<FairMutex<Term>>`. Cell extraction is performed by CellGrid before the render call.

#### Scenario: Render receives pre-extracted grid
- **WHEN** `GpuRenderer::render_frame()` is called
- **THEN** it receives a `&CellGrid` with cells, dirty_rows, and cursor already populated — no Term lock is acquired during rendering
