## Context

The renderer-abstraction change introduced two backends (`GpuRenderer`, `CpuRenderer`) sharing a `CellGrid` extraction layer with row-level dirty tracking. Both backends call an independent copy of `build_row_buffer()` that converts a row of `CellInfo` into a `cosmic_text::Buffer`. Both copies use `Shaping::Advanced` unconditionally, invoking HarfBuzz on every dirty row every frame.

**Measured costs:**
- `Shaping::Advanced` per row: 500–2000µs (harfbuzz + font fallback resolution)
- `Shaping::Basic` per row: ~50–100µs (direct codepoint→glyph, no HarfBuzz)
- `glyphon.prepare()` for 24 rows after atlas warmup: ~150–250µs (vertex assembly + queue.write_buffer)
- `PresentMode::Fifo` latency floor: 0–16ms added to every frame

Frame budget at 60fps is 16.6ms. With htop redrawing ~20 rows: 20 × 1ms = 20ms → misses budget. With Shaping::Basic for ASCII rows: 20 × 0.075ms = 1.5ms → well within budget.

**Why `glyphon.prepare()` cannot be partially updated:** glyphon's `TextRenderer::prepare()` clears its `glyph_vertices` vec and rebuilds from all provided TextAreas each call. The render pass uses `LoadOp::Clear` — there is no frame-to-frame surface persistence. Therefore "skip clean rows in prepare" would leave those rows blank. A partial-update model would require `LoadOp::Load` and dirty-rectangle rendering — a larger architectural change deferred to a future proposal.

**Current `build_row_buffer()` duplication:** The function is nearly identical in `glyphon_backend.rs` (GPU path) and `cpu.rs` (CPU path). These will diverge under independent maintenance.

## Goals / Non-Goals

**Goals:**
- `needs_advanced_shaping()`: per-row check selecting `Shaping::Basic` or `Shaping::Advanced` based on actual script complexity
- `text_shaping::build_row_buffer()`: single shared implementation used by both backends
- `PresentMode::AutoNoVsync` in GpuRenderer: lets wgpu select the best low-latency mode per platform
- Treat `Suboptimal` identically to `Outdated`/`Lost`: configure-first, retry, render — no suboptimal frames presented

**Non-Goals:**
- Partial GPU rendering via `LoadOp::Load` and dirty rectangles — requires architectural rethink
- Per-row `TextRenderer` instances in glyphon — doesn't work without frame persistence
- Font configuration or runtime font switching
- Column-level damage tracking (row-level is sufficient and matches production terminals)
- CPU backend subpixel rendering or improved font metrics (separate `render-quality` change)
- Scrollback rendering

## Decisions

### Decision 1: `needs_advanced_shaping()` uses explicit Unicode range checks

**Choice:** A free function in `text_shaping.rs` that returns `true` only when a row contains characters that genuinely require HarfBuzz:

```
Complex (needs Advanced):
  Combining marks       U+0300–U+036F
  Hebrew                U+0590–U+05FF
  Arabic                U+0600–U+06FF
  Syriac                U+0700–U+074F
  Thaana                U+0780–U+07BF
  Devanagari            U+0900–U+097F
  Other Indic scripts   U+0980–U+0DFF
  Thai                  U+0E00–U+0E7F
  Lao / Tibetan         U+0E80–U+0FFF
  Emoji / ZWJ range     U+1F000+

Simple (uses Basic):
  ASCII                 U+0000–U+007F
  Latin-1 supplement    U+0080–U+00FF
  Latin Extended A/B    U+0100–U+024F
  Greek, Cyrillic       U+0370–U+04FF
  CJK (each independent)U+4E00–U+9FFF
  Box drawing           U+2500–U+257F
  Block elements        U+2580–U+259F
  Braille               U+2800–U+28FF
  All other ranges      → Basic (conservative: no complex shaping needed)
```

**Alternatives considered:**
- `c.is_ascii()` only — misses box-drawing rows (U+2500+) used by every ncurses TUI; those would still use Advanced unnecessarily.
- `c as u32 < 0x100` — same box-drawing problem.
- Unicode category lookup via the `unicode-general-category` crate — correct but adds a dependency for a simple range check; not justified.

**Rationale:** The function checks for scripts where character shape depends on context (Arabic joining, Indic conjuncts), directionality (RTL), or combining mark positioning. CJK characters are independent glyphs — each codepoint maps to one glyph regardless of neighbors. Box-drawing characters are single glyphs with no contextual shaping. `Shaping::Basic` is correct for all of these. The range list is conservatively inclusive on the "complex" side: unknown ranges default to Basic, not Advanced, since unlisted scripts are almost certainly independent glyphs.

### Decision 2: `text_shaping.rs` as a dedicated module

**Choice:** New `crates/lain-core/src/renderer/text_shaping.rs` with `pub(crate) fn build_row_buffer(...)` and `pub(crate) fn needs_advanced_shaping(...)`. Both backends import from this module.

**Alternatives considered:**
- Keep duplicated in each backend — simple but guarantees divergence; the shaping threshold logic will be maintained in two places.
- Add to `cell_grid.rs` — `CellGrid` owns `FontSystem` and `CellInfo` but has no rendering concerns; adding `Buffer` creation there mixes the abstraction boundary.
- Add to `mod.rs` — would make it part of the public renderer surface; `pub(crate)` in a dedicated module is cleaner.

**Rationale:** The function is stateless (`font_system` is passed by `&mut`), takes only types already in scope (`CellInfo`, `FontSystem`), and has a single well-defined job. A dedicated module makes the split explicit: `cell_grid.rs` = what to render, `text_shaping.rs` = how to shape it, `gpu.rs`/`cpu.rs` = how to draw it.

### Decision 3: `PresentMode::AutoNoVsync` — delegate to wgpu

**Choice:** Replace `PresentMode::Fifo` with `PresentMode::AutoNoVsync` in `GpuRenderer::new()`.

**Alternatives considered:**
- Explicit mode selection (`[Mailbox, FifoRelaxed, Fifo].find(available)`) skipping `Immediate` to avoid X11 tearing — more lines, manually reimplements what `AutoNoVsync` does. wgpu already knows which modes are safe per platform/backend.
- `PresentMode::Mailbox` unconditionally — panics if unavailable (not in `surface_caps.present_modes`).
- Keep `Fifo` — adds up to 16ms latency floor; unacceptable for a terminal.

**Rationale:** `AutoNoVsync` is the explicit API for "I want low latency, pick the best available." On Wayland/Vulkan it picks Mailbox if available, FifoRelaxed otherwise, Fifo as last resort. On X11 it may pick Immediate — which can tear, but on Wayland (lain-shell's target) the compositor prevents tearing at the display level regardless of present mode. Delegating to wgpu is correct.

### Decision 4: Suboptimal → configure-first, same as Outdated

**Choice:** On `Suboptimal`, reconfigure the surface immediately and retry `get_current_texture`, rather than presenting the suboptimal frame and reconfiguring after.

**Alternatives considered:**
- Keep current behavior (present Suboptimal frame, configure after) — wgpu says Suboptimal is presentable; we're leaving latency on the table and may show a mismatched frame.
- Skip rendering entirely on Suboptimal — wastes a frame, increases perceived lag.

**Rationale:** `Suboptimal` means the surface dimensions don't match the window. In our flow, `Resized` already calls `surface.configure()` before `request_redraw()`. A subsequent `Suboptimal` in `render_frame()` means the window size changed again mid-flight — reconfiguring again is correct and cheap (~10µs). Unifying the Outdated/Suboptimal/Lost handling into one path simplifies the code and eliminates the class of visual artifacts that come from presenting wrong-size frames.

## Risks / Trade-offs

**[Risk] AutoNoVsync may not improve latency on all Wayland compositors** → On compositors that don't support Mailbox (e.g., some wlroots compositors), AutoNoVsync falls back to Fifo transparently. No regression — worst case is identical to current behavior.

**[Risk] `needs_advanced_shaping()` range list is incomplete** → The conservative fallback is `Shaping::Basic` for unlisted ranges, not `Advanced`. If a range is misclassified, the worst visible outcome is slightly incorrect rendering for a script that needed HarfBuzz. The function is easy to extend: add a range to the "complex" check. This is not a safety concern.

**[Risk] `Shaping::Basic` has no font fallback** → If the system monospace font lacks a glyph for a character classified as "simple" (e.g., a box-drawing char not in the font), it shows tofu instead of falling back. For lain-shell's current font setup (system monospace), box-drawing coverage is universal in any modern terminal font. For CJK: `Shaping::Basic` won't fallback to a CJK font if the monospace font lacks glyphs. CJK rendering in a terminal PTY is uncommon; when it occurs, tofu is acceptable until a font-configuration change adds proper fallback.

**[Trade-off] `build_row_buffer()` still shapes per-row, not per-span** → Contour Terminal does span-level shaping (shape ASCII spans with Basic, non-ASCII spans with Advanced within one row). We don't. A row with one Arabic character in an otherwise-ASCII line uses Advanced for the entire row. This is correct but slightly over-conservative. Span-level shaping would require building multiple Buffers and compositing, which is significantly more complex. Per-row granularity matches our existing `dirty_rows` model and is sufficient for all current lain-shell content.

## Open Questions

None — all design decisions resolved before implementation.
