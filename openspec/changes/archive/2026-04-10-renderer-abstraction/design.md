## Context

The vertical spike proved the PTY → VTE → wgpu rendering pipeline works end-to-end. However, the spike conflated three responsibilities inside `pipeline.rs::render_frame()`:

1. **Cell extraction** — lock Term, iterate `renderable_content()`, build flat `CellInfo[]` grid
2. **Frame orchestration** — build background rects, cursor rect, prepare renderers
3. **GPU rendering** — glyphon text atlas + wgpu rect pipeline + render pass

All three are wired directly to wgpu types (`Device`, `Queue`, `CommandEncoder`, `TextureView`). There's no seam where a CPU backend could plug in. Additionally, `GlyphonRenderer::prepare()` rebuilds all cosmic-text `Buffer` objects every frame — the dominant performance bottleneck.

### Current file layout

```
src/main.rs                           → App struct with GpuState, RenderPipeline, Terminal
crates/lain-core/src/renderer/
  mod.rs                              → exports submodules
  pipeline.rs                         → RenderPipeline (extraction + orchestration + GPU)
  glyphon_backend.rs                  → GlyphonRenderer + CellInfo + ansi_to_color
  rect.rs                             → RectRenderer (wgpu quad pipeline)
```

### Key constraints

- `alacritty_terminal::Term` is behind `Arc<FairMutex<Term>>` — lock must be held briefly
- cosmic-text `FontSystem` is required by both backends (GPU: glyphon atlas, CPU: swash rasterization)
- wgpu 29 API: `CurrentSurfaceTexture` enum, `InstanceDescriptor::new_without_display_handle_from_env()`
- glyphon is a git dependency (main branch targets wgpu 29 + cosmic-text 0.18)
- softbuffer 0.4: `Context::new(display)` + `Surface::new(&context, window)`, then `surface.next_buffer()` → `Buffer` with `.pixels() → &mut [Pixel]` (Pixel struct with r/g/b/a fields, constructed via `Pixel::new_rgb(r, g, b)`), call `buffer.present()` to display

## Goals / Non-Goals

**Goals:**
- Separate cell extraction from rendering so both backends consume the same data
- Row-level dirty tracking to eliminate redundant text shaping
- Cache cosmic-text Buffers — only re-shape rows where content changed
- Introduce `Renderer` enum with `Gpu` and `Cpu` variants
- Add softbuffer-based CPU rendering backend
- Runtime backend selection via `LAIN_RENDERER` env var with GPU-first auto-detection
- Keep all renderer code inside `crates/lain-core/src/renderer/`

**Non-Goals:**
- Font configuration UI or runtime font switching
- Scrollback rendering (Term already handles scrollback; we render visible grid only)
- Multi-window support
- Ligature support (cosmic-text handles this eventually but not a goal here)
- Mixed GPU+CPU rendering (one backend per session)
- Dynamic backend switching at runtime (restart required)

## Decisions

### Decision 1: Enum dispatch, not trait object

**Choice:** `enum Renderer { Gpu(GpuRenderer), Cpu(CpuRenderer) }` with match arms in main.rs.

**Alternatives considered:**
- `Box<dyn TerminalRenderer>` — forces a unified surface type. GPU needs `&Device, &Queue, &mut CommandEncoder, &TextureView`. CPU needs `&mut [u32], width, height`. A trait that accommodates both is either overly generic or requires unsafe casts.
- Two separate `App` structs — massive duplication of event handling, input routing, resize logic.

**Rationale:** Exactly two variants, chosen once at startup. Enum gives exhaustive match checks, no vtable overhead, and each arm can use its native types directly. The shared code (cell extraction, input handling) stays in App; only the render call branches.

### Decision 2: CellGrid as shared extraction layer

**Choice:** New `CellGrid` struct owns the extracted cell data, cursor state, and row-level dirty flags. Populated by locking Term once per frame.

```rust
pub struct CellGrid {
    cells: Vec<CellInfo>,
    prev_cells: Vec<CellInfo>,
    dirty_rows: Vec<bool>,
    cursor: CursorState,
    cols: u16,
    rows: u16,
}
```

**Rationale:** Both backends need the same input data. Extracting it once and comparing against the previous frame gives row-level dirty detection. The grid is small (80×24 = 1920 cells, ~30KB) so double-buffering is cheap. Simple `Vec<bool>` for dirty flags — bit vectors add complexity with negligible memory benefit at terminal scale.

### Decision 3: Buffer cache in GlyphonRenderer, not in CellGrid

**Choice:** Each backend owns its own cosmic-text Buffer cache. CellGrid provides dirty flags; backends decide what to cache.

**Alternatives considered:**
- Shared Buffer cache in CellGrid — both backends consume pre-shaped buffers. Attractive but problematic: GPU path feeds Buffers to glyphon's `TextArea` references with specific lifetime requirements. CPU path needs to rasterize individual glyphs from the Buffer via swash. Shared ownership creates borrow checker friction.

**Rationale:** The GPU and CPU paths consume shaped text differently. GPU: glyphon borrows `&Buffer` to build texture atlas entries. CPU: iterates layout runs to get glyph images. Letting each backend own its Buffers avoids lifetime tangles. The shaping cost (the expensive part) is the same either way — it's `set_rich_text` + `shape_until_scroll` — and the dirty flags from CellGrid tell both backends which rows to skip. FontSystem can be shared (it's just font data + caches).

### Decision 4: FontSystem shared via single ownership in CellGrid

**Choice:** `CellGrid` owns the `FontSystem`. Passed as `&mut` to whichever backend is preparing.

**Rationale:** FontSystem is expensive to create (scans system fonts). Only one backend runs per session, so there's no contention. CellGrid is the natural owner since it's created before the backend and outlives the render call.

### Decision 5: CPU backend uses swash for glyph rasterization

**Choice:** `CpuRenderer` uses cosmic-text for shaping (same as GPU path) + swash (via `cosmic_text::SwashCache`) to rasterize glyphs into pixel data. The rasterization pipeline is: layout runs from Buffer → `PhysicalGlyph` → `CacheKey` → `SwashCache::get_image()` → `SwashImage` (alpha bitmap in `.data: Vec<u8>` with `.placement` for offset/size). Alpha values are composited with foreground color and blitted to softbuffer's `&mut [Pixel]` buffer.

**Alternatives considered:**
- fontdue — simpler API but doesn't integrate with cosmic-text's shaping pipeline. Would require a separate text layout system.
- ab_glyph — same issue, doesn't consume cosmic-text layout runs.

**Rationale:** cosmic-text already does the shaping work. SwashCache is part of cosmic-text's public API and produces glyph images that can be blitted directly to a pixel buffer. This reuses all the shaping infrastructure and avoids a parallel text stack.

### Decision 6: Backend selection via env var with auto-detection

**Choice:** Check `LAIN_RENDERER` env var first (`gpu`, `cpu`). If unset, try GPU (request_adapter); if None, fall back to CPU.

**Rationale:** Simple, no config file needed at this stage. Explicit override for testing/debugging. Auto-detection handles the common case (GPU when available, CPU for headless/SSH). The detection happens at startup in `main.rs::resumed()`.

### Decision 7: Each Renderer variant owns its surface state

**Choice:** `GpuRenderer` owns `wgpu::Surface`, `Device`, `Queue`, `SurfaceConfiguration`. `CpuRenderer` owns `softbuffer::Context` and `softbuffer::Surface`. The current `GpuState` struct in main.rs is absorbed into the `Gpu` variant. App only holds `Window`, `Terminal`, `CellGrid`, and `Renderer`.

**Alternatives considered:**
- Keep surface state in App, pass references to renderer — creates an awkward split where main.rs handles surface acquisition (`get_current_texture` / `next_buffer`) differently per backend, leaking backend concerns into shared code.

**Rationale:** Each backend's surface lifecycle is different: GPU does `get_current_texture` → create encoder → render pass → submit → present. CPU does `resize` → `next_buffer` → write pixels → present. Encapsulating this inside each variant keeps main.rs dispatch clean — it just calls `renderer.render_frame(&grid)` and each variant handles its own surface internally. Resize also stays encapsulated: GPU reconfigures its surface, CPU resizes its softbuffer.

### Decision 8: Cursor rendering is independent of row dirty tracking

**Choice:** Cursor is always rendered on top of the text layer, regardless of dirty_rows. Both backends re-render the cursor rect every frame. The previous cursor position is tracked to know which cell needs glyph restoration (CPU path only).

**Alternatives considered:**
- Mark cursor rows as dirty — forces full row re-shaping when only the cursor moved, negating the dirty-row optimization for the most common case (typing = cursor moves every frame).

**Rationale:** The cursor moves frequently without content changes (arrow keys, blinking). If cursor rows were marked dirty, nearly every frame would re-shape at least one row unnecessarily. Instead: GPU path draws cursor rect in a separate pass on top (already works this way). CPU path tracks `prev_cursor_pos` and restores the glyph underneath by re-blitting just that cell from the cached row, then draws the new cursor. This is O(1) per frame for cursor updates.

## Risks / Trade-offs

**[Risk] softbuffer performance on high-resolution displays** → Mitigation: For a terminal, the content changes infrequently (dirty rows). CPU rendering of 1-3 rows of monospace text per frame is fast even at 4K. If it's still slow, the dirty tracking reduces the blast radius to only changed rows. Acceptable for a fallback mode.

**[Risk] cosmic-text SwashCache API stability** → Mitigation: We already depend on cosmic-text 0.18 and use SwashCache in the GPU path (via glyphon). The CPU path uses the same API surface. If it changes, both backends break equally and the fix is mechanical.

**[Risk] glyphon Buffer lifetime requirements constrain GpuRenderer** → Mitigation: GpuRenderer owns its Buffers directly (Decision 3). CellGrid just provides dirty flags. No cross-ownership lifetime issues.

**[Risk] Enum dispatch adds match arms to main.rs** → Mitigation: There are exactly 4 dispatch points: init, render, resize, cell_metrics. The match arms are straightforward and the compiler enforces exhaustiveness. This is preferable to trait gymnastics.

**[Trade-off] Two Buffer caches means shaping isn't shared between backends** → Acceptable: only one backend runs per session. There's no scenario where both GPU and CPU shape the same text.
