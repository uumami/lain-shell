## Why

The vertical spike proved the PTY → VTE → wgpu pipeline works but is unusably slow — every frame rebuilds all cosmic-text Buffers from scratch (O(rows × cols) text shaping per frame). Before fixing performance, the code must be restructured: cell extraction is entangled with GPU rendering in `pipeline.rs`, making it impossible to add a second backend (CPU via softbuffer) or cache shared work. Separating "what to render" from "how to render" is prerequisite to both performance and dual-backend support.

## What Changes

- **Extract CellGrid**: Move cell extraction from `pipeline.rs::render_frame()` into a standalone `CellGrid` struct that both backends consume. Includes cursor state and row-level dirty tracking.
- **Add row-level dirty detection**: Diff current frame against previous frame to identify which rows actually changed. Skip re-shaping unchanged rows.
- **Cache cosmic-text Buffers**: Maintain one `Buffer` per row, only call `set_rich_text` + `shape_until_scroll` on dirty rows. This is the primary performance fix (~10-50x improvement for typical usage).
- **Introduce Renderer enum**: `enum Renderer { Gpu(GpuRenderer), Cpu(CpuRenderer) }` with match-based dispatch in `main.rs`. No trait — exactly two variants, chosen at startup.
- **Wrap existing GPU code as GpuRenderer**: Current `GlyphonRenderer` + `RectRenderer` become internals of the `Gpu` variant. No functional change to GPU path.
- **Add CpuRenderer via softbuffer**: New backend that rasterizes glyphs directly to a pixel buffer using cosmic-text + swash. Backgrounds are simple pixel fills.
- **Runtime backend selection**: `LAIN_RENDERER=gpu|cpu` env var, with auto-detection fallback (GPU if adapter available, else CPU).

## Capabilities

### New Capabilities
- `cell-grid`: Shared cell extraction, cursor state, and row-level dirty tracking — consumed by both renderer backends
- `cpu-cell-rendering`: CPU-based terminal rendering via softbuffer + cosmic-text/swash direct rasterization
- `renderer-selection`: Runtime selection between GPU and CPU backends via env var and auto-detection

### Modified Capabilities
- `wgpu-cell-rendering`: TextRenderer trait requirement replaced with enum dispatch; GlyphonRenderer and RectRenderer become internals of GpuRenderer variant; shared Buffer cache moves to cell-grid layer

## Impact

- **Code**: `crates/lain-core/src/renderer/` restructured — `pipeline.rs` split into `cell_grid.rs` + backend-specific modules. `src/main.rs` updated for enum dispatch and backend initialization.
- **Dependencies**: Add `softbuffer` to `lain-core/Cargo.toml`. cosmic-text `FontSystem` is shared (owned by CellGrid); Buffer caches are per-backend (each backend owns its own cached Buffers).
- **APIs**: `RenderPipeline` replaced by `Renderer` enum. `GlyphonRenderer` internalized (no longer public).
- **Performance**: Dirty-row caching eliminates per-frame full reshaping. Typical frames drop from ~1920 cells shaped to ~1-3 rows shaped.
