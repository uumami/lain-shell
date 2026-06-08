## 1. CellGrid Extraction Layer

- [x] 1.1 Create `crates/lain-core/src/renderer/cell_grid.rs` with `CellGrid` struct: `cells: Vec<CellInfo>`, `prev_cells: Vec<CellInfo>`, `dirty_rows: Vec<bool>`, `cursor: CursorState`, `cols: u16`, `rows: u16`, `font_system: FontSystem`
- [x] 1.2 Move `CellInfo` and `ansi_to_color()` from `glyphon_backend.rs` to `cell_grid.rs` — add `#[derive(Clone, PartialEq)]` to `CellInfo` (PartialEq needed for dirty detection; cosmic_text::Color implements PartialEq)
- [x] 1.3 Implement `CellGrid::new()` — create FontSystem, initialize empty grid
- [x] 1.4 Implement `CellGrid::extract(&mut self, term: &Arc<FairMutex<Term<JsonLessListener>>>)` — lock Term, iterate renderable_content, populate cells + cursor, compute dirty_rows by comparing cells vs prev_cells row-by-row (cursor changes do NOT dirty rows), swap prev_cells
- [x] 1.5 Implement `CellGrid::resize(&mut self, cols, rows)` — resize grid, mark all rows dirty, reset prev_cells
- [x] 1.6 Add `CursorState` struct: `col: usize`, `row: usize`, `shape: CursorShape`, `visible: bool`

## 2. GpuRenderer Refactor

- [x] 2.1 Create `crates/lain-core/src/renderer/gpu.rs` with `GpuRenderer` struct containing `surface: wgpu::Surface`, `device: wgpu::Device`, `queue: wgpu::Queue`, `config: wgpu::SurfaceConfiguration`, `glyphon: GlyphonRenderer`, `rects: RectRenderer`, `cached_buffers: Vec<Option<Buffer>>`
- [x] 2.2 Implement `GpuRenderer::new(instance, window, adapter)` — create device/queue, configure surface, create GlyphonRenderer + RectRenderer (absorbs current GpuState + RenderPipeline::new)
- [x] 2.3 Implement `GpuRenderer::render_frame(&mut self, grid: &CellGrid, font_system: &mut FontSystem)` — get_current_texture, create encoder, build rects from grid.cells, build cursor rect from grid.cursor, prepare + render pass, submit + present (entire surface lifecycle encapsulated)
- [x] 2.4 Add Buffer caching: maintain `Vec<Option<Buffer>>`, only rebuild Buffers where `grid.dirty_rows[row]` is true, reuse cached Buffers for clean rows
- [x] 2.5 Move background rect building from pipeline.rs into GpuRenderer::render_frame
- [x] 2.6 Move cursor rect building from pipeline.rs into GpuRenderer::render_frame
- [x] 2.7 Update GlyphonRenderer::prepare to accept pre-built Buffers (Vec<&Buffer>) instead of rebuilding from CellInfo slice
- [x] 2.8 Implement `GpuRenderer::resize(&mut self, width, height)` — reconfigure surface, clear cached_buffers
- [x] 2.9 Implement `GpuRenderer::cell_metrics(&self) -> (f32, f32)` — delegates to GlyphonRenderer

## 3. CpuRenderer Backend

- [x] 3.1 Add `softbuffer = "0.4"` to `crates/lain-core/Cargo.toml`
- [x] 3.2 Create `crates/lain-core/src/renderer/cpu.rs` with `CpuRenderer` struct containing `context: softbuffer::Context`, `surface: softbuffer::Surface`, `swash_cache: cosmic_text::SwashCache`, `cached_buffers: Vec<Option<Buffer>>`, `prev_cursor: Option<CursorState>`
- [x] 3.3 Implement `CpuRenderer::new(window: Arc<Window>)` — create Context via `Context::new(display_handle)`, create Surface via `Surface::new(&context, window)`
- [x] 3.4 Implement background rendering: for each dirty row, fill pixel rectangles with `Pixel::new_rgb(r, g, b)` for cells with non-default backgrounds
- [x] 3.5 Implement glyph rasterization: for each dirty row, iterate cosmic-text layout runs, convert to PhysicalGlyph → CacheKey, call `SwashCache::get_image()` to get SwashImage (alpha bitmap), composite with foreground color, blit to pixel buffer
- [x] 3.6 Implement cursor rendering: fill pixel rectangle at cursor position; track prev_cursor to restore old cursor cell by re-blitting the glyph from cached row data (cursor movement without content change should NOT re-shape rows)
- [x] 3.7 Implement dirty-row optimization: only clear + re-render rows where dirty_rows[row] is true
- [x] 3.8 Implement frame presentation: acquire buffer via `surface.next_buffer()`, write pixels, call `buffer.present()`
- [x] 3.9 Implement `CpuRenderer::resize(&mut self, width, height)` — call `surface.resize(width, height)`, clear cached_buffers, mark full re-render
- [x] 3.10 Implement `CpuRenderer::cell_metrics(&self) -> (f32, f32)` — compute from FontSystem metrics (same font_size=14.0, line_height=18.0, cell_width=font_size*0.6)

## 4. Renderer Enum and Dispatch

- [x] 4.1 Define `enum Renderer { Gpu(GpuRenderer), Cpu(CpuRenderer) }` in `crates/lain-core/src/renderer/mod.rs`, re-export CellGrid
- [x] 4.2 Implement dispatch methods on Renderer: `render_frame(&mut self, grid: &CellGrid, font_system: &mut FontSystem)`, `resize(&mut self, width, height)`, `cell_metrics(&self) -> (f32, f32)`
- [x] 4.3 Update `src/main.rs` App struct: replace `gpu: Option<GpuState>` + `render_pipeline: Option<RenderPipeline>` with `renderer: Option<Renderer>` + `cell_grid: Option<CellGrid>`
- [x] 4.4 Update `src/main.rs` resumed(): add backend selection — check `LAIN_RENDERER` env var, try GPU adapter (request_adapter), fall back to CPU if None or if LAIN_RENDERER=cpu
- [x] 4.5 Update `src/main.rs` RedrawRequested: call `cell_grid.extract(&terminal.term)` then `renderer.render_frame(&grid, &mut grid.font_system)` via match dispatch
- [x] 4.6 Update `src/main.rs` Resized: call `cell_grid.resize()` then `renderer.resize()` via match dispatch
- [x] 4.7 Remove old `pipeline.rs` — functionality split into cell_grid.rs + gpu.rs

## 5. Module Cleanup

- [x] 5.1 Update `crates/lain-core/src/renderer/mod.rs` exports: add cell_grid, gpu, cpu; keep glyphon_backend and rect as pub(crate)
- [x] 5.2 Update `crates/lain-core/src/lib.rs` if needed for new public types
- [x] 5.3 Verify `cargo check` passes with no warnings
- [x] 5.4 Verify `cargo build` succeeds for both GPU and CPU paths

## 6. Validation

- [ ] 6.1 Test GPU backend: launch with `LAIN_RENDERER=gpu`, verify terminal renders correctly (manual)
- [ ] 6.2 Test CPU backend: launch with `LAIN_RENDERER=cpu`, verify terminal renders correctly (manual)
- [ ] 6.3 Test auto-detection: launch without LAIN_RENDERER, verify GPU is selected when adapter available (manual)
- [ ] 6.4 Verify performance improvement: confirm that typing in GPU mode no longer rebuilds all Buffers per frame (add log or timing)
- [ ] 6.5 Test resize in both backends (manual)
- [ ] 6.6 Test cursor movement without content change renders correctly in CPU mode (manual)
