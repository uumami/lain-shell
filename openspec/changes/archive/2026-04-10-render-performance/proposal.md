## Why

The current renderer uses `Shaping::Advanced` (HarfBuzz) for every dirty row on every frame — including pure ASCII rows that don't need it. This costs 500–2000µs per row, making programs like `htop` (20 dirty rows × ~1ms = 20ms) miss the 16.6ms frame budget and making keystrokes feel sluggish. Additionally, `PresentMode::Fifo` adds up to 16ms of VSync latency between PTY output and the visible frame, and the GPU surface lifecycle handles `Suboptimal` differently from `Outdated`, causing unnecessary visual artifacts during live resize.

## What Changes

- **Extract `text_shaping.rs`**: New shared module with `build_row_buffer()` and `needs_advanced_shaping()`. Both `GpuRenderer` and `CpuRenderer` call into this module — eliminating duplicated shaping logic and ensuring identical behavior across backends.
- **Add `needs_advanced_shaping()` fast path**: Per-row check for characters that actually require HarfBuzz (Arabic, Hebrew, Indic scripts, Thai, combining marks, emoji ZWJ range). ASCII, Latin, CJK, box-drawing, and block elements use `Shaping::Basic` (~10–50x faster). Complex script rows fall back to `Shaping::Advanced`.
- **`PresentMode::AutoNoVsync` in GpuRenderer**: Replaces `Fifo`. wgpu selects the best low-latency mode available on the current platform (Immediate → Mailbox → FifoRelaxed → Fifo). Eliminates up to 16ms of VSync floor on inputs and PTY output delivery.
- **Treat `Suboptimal` identically to `Outdated`/`Lost`**: Configure the surface first, then retry — rather than presenting a suboptimal frame and reconfiguring afterward. Removes per-frame visual artifacts during live window resize.

## Capabilities

### New Capabilities
- `text-shaping`: Shared text shaping module — `build_row_buffer()` produces a `cosmic_text::Buffer` for a row of `CellInfo`, with `needs_advanced_shaping()` selecting `Shaping::Basic` or `Shaping::Advanced` based on the actual script complexity of the row's characters.

### Modified Capabilities
- `wgpu-cell-rendering`: Adds requirements for low-latency frame presentation (AutoNoVsync) and correct surface lifecycle on resize (Suboptimal treated as Outdated).

## Impact

- **New file**: `crates/lain-core/src/renderer/text_shaping.rs`
- **Modified**: `crates/lain-core/src/renderer/glyphon_backend.rs` — removes `build_row_buffer()`, delegates to `text_shaping`
- **Modified**: `crates/lain-core/src/renderer/cpu.rs` — removes `build_row_buffer()`, delegates to `text_shaping`
- **Modified**: `crates/lain-core/src/renderer/gpu.rs` — `PresentMode::AutoNoVsync`, unified Suboptimal/Outdated/Lost handling
- **No new dependencies** — all changes are within existing `cosmic-text`, `glyphon`, and `wgpu` APIs
