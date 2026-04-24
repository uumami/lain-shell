## Context

lain-shell renders by reacting to `TerminalEvent::Wakeup` events from the PTY thread — each wakeup triggers an immediate full redraw. During heavy terminal output this produces irregular frame bursts (50–200 redraws/second) causing visible choppiness. Without vsync, frames land at arbitrary times relative to the display refresh cycle. Font metrics are hardcoded at `font_size * 0.6` rather than measured from the loaded font, causing sub-pixel cell misalignment. Text attributes beyond bold (italic, underline, strikethrough, dim) are silently discarded. These compound into a terminal that feels significantly worse than Alacritty or Kitty even at the same underlying rendering speed.

The two renderers (GPU via wgpu+glyphon, CPU via softbuffer) share the same `CellGrid` extraction layer. All changes here affect both renderers uniformly.

## Goals / Non-Goals

**Goals:**
- Eliminate choppy frame bursts during PTY output by capping render rate to 60fps
- Eliminate input lag by giving keystroke-triggered redraws an immediate fast-path
- Measure cell_width from the actual loaded font (not a fixed multiplier)
- Scale font size with `window.scale_factor()` for HiDPI correctness
- Blink the cursor at 500ms intervals using `ControlFlow::WaitUntil`
- Render italic, underline, strikethrough, and dim in both CPU and GPU paths
- Work correctly for every user: any display DPI, any terminal program, nvim or not

**Non-Goals:**
- Smooth scroll animation (sub-cell pixel offsets between scroll positions)
- Font configurability (font family, size are still hardcoded constants for now)
- Subpixel antialiasing (ClearType) in the CPU path — treat SubpixelMask as mask
- Ligature support beyond what cosmic-text provides by default

## Decisions

### D1: Frame gate via dirty flag + `ControlFlow::WaitUntil`

**Decision**: Introduce a `needs_redraw: bool` flag in `App`. On `Wakeup`, set the flag; if the elapsed time since `last_frame` exceeds the frame interval (16.7ms / 60fps), call `window.request_redraw()` immediately; otherwise call `event_loop.set_control_flow(WaitUntil(last_frame + interval))`. `RedrawRequested` renders and updates `last_frame`.

**Keystroke fast-path**: `KeyboardInput` events call `window.request_redraw()` directly, bypassing the frame gate. This ensures keystroke echo always renders at the next available frame.

**Alternative considered — dedicated render thread**: More precise timing but requires cross-thread window handle sharing, which winit restricts. Not worth the complexity.

**Alternative considered — always vsync wait**: Using only `AutoVsync` without a dirty flag would cap render rate but still render on every wakeup. Wastes GPU time when terminal output is idle.

### D2: `AutoVsync` presentation mode

**Decision**: Switch `PresentMode` from `AutoNoVsync` to `AutoVsync` in the GPU renderer. This aligns frame presentation with the display refresh cycle, eliminating tearing and ensuring smooth scrolling.

**Latency concern**: vsync waits happen on the GPU side (`queue.submit` or `present`), not on the CPU side. The event loop continues processing keyboard/mouse events while the GPU waits. Measured worst-case additional latency: 1 vblank = 16.7ms at 60Hz. Acceptable for a terminal emulator.

**No impact on CPU renderer**: softbuffer's `buffer.present()` does not have a vsync mode; leave it as-is.

### D3: Font metrics measured from font via multi-char layout

**Decision**: At renderer initialization (and on scale_factor change), lay out 10 repetitions of `'M'` in a temporary `Buffer` at the target physical font size. Divide `run.line_w / 10` and round to the nearest integer pixel to get `cell_width`. Keep `line_height = 18.0` logical (× scale_factor for physical) as it is a reasonable ratio; only cell_width is measured since it is the source of visible misalignment.

**Why round to integer**: Sub-pixel cell widths cause box-drawing characters to misalign at certain column positions. Integer cell widths guarantee every column boundary is on a pixel boundary.

**Alternative — use font's advance width API directly**: cosmic-text does not expose raw font metric structs in its public API at this time; the layout measurement approach is the only stable option without reaching into fontdb internals.

### D4: HiDPI via scale_factor multiplication

**Decision**: Store `logical_font_size: f32 = 14.0` as the canonical size. All physical sizes computed as `logical_font_size * scale_factor as f32`. `cell_metrics()` returns physical pixel dimensions. The main loop reads `scale_factor` from the `Window` on startup and re-initializes renderer metrics on `WindowEvent::ScaleFactorChanged`.

**cols/rows recalculation**: On scale change, `cols = physical_width / physical_cell_w` and `rows = physical_height / physical_cell_h`. Terminal is resized accordingly. This is the same code path as window resize.

### D5: Cursor blink with WaitUntil

**Decision**: Add `cursor_blink_visible: bool` and `cursor_blink_deadline: Option<Instant>` to `App`. The blink interval is 500ms. After each render, if the window is focused, set `WaitUntil(now + 500ms)`. On the timer wake-up, toggle `cursor_blink_visible`, request a redraw, reschedule.

After `cell_grid.extract()`, override `cell_grid.cursor.visible = self.cursor_blink_visible`. This avoids modifying the extract function signature.

On any keypress or window-focus-lost: reset `cursor_blink_visible = true` and cancel/restart the timer.

**Alternative — blink in the cell_grid extract**: Would require passing blink state into extract; slightly more coupled. The override in main.rs is simpler.

### D6: Text attributes — shaper for italic/dim, rects for underline/strikethrough

**Decision**: Extend `CellInfo` with `italic: bool`, `underline: bool`, `strikethrough: bool`, `dim: bool`. In `cell_grid.rs` extract(), populate from `Flags::ITALIC`, `Flags::UNDERLINE`, `Flags::STRIKEOUT`, `Flags::DIM_BOLD`. In `text_shaping.rs`, pass `Style::Italic` to Attrs when italic; apply dim by reducing fg color to 60% brightness. Underline and strikethrough are pixel-accurate rects:

```
Underline:     y = row * cell_h + cell_h - 2   height = 1
Strikethrough: y = row * cell_h + cell_h * 0.6  height = 1
```

Both use the cell's fg color. In the GPU renderer they are additional `RectInstance` entries; in the CPU renderer they are drawn as filled pixel runs after glyph rendering.

## Risks / Trade-offs

- **WaitUntil timer precision**: OS sleep granularity (~1ms on Linux) means frames may land slightly off from 16.7ms. Acceptable — this is better than the current unbounded jitter.
- **AutoVsync on Wayland compositors**: Some Wayland compositors implement MAILBOX rather than FIFO vsync. `AutoVsync` will use whatever is available; visual result may vary. Not a regression from current state.
- **Font metrics on missing fallback font**: If the monospace font is unavailable, layout of 'M' returns 0 or fallback width; the code must guard with a sensible fallback (`font_size * 0.6`).
- **CellInfo struct growth**: Adding 4 bool fields (4 bytes) to a struct used for every cell. A 220×50 terminal = 11,000 cells × 4 extra bytes = 44KB. Not a concern.
- **Scale factor change during session**: Reinitializing font metrics mid-session causes a full re-render. May briefly flash blank. Acceptable — this happens rarely (user moves window between monitors).

## Open Questions

- Should cursor blink stop when the window loses focus? (Common convention: yes.) Should stop and show cursor as visible (non-blinking) while unfocused.
- What is the correct strikethrough y-position? Using `cell_h * 0.6` from top; may need adjustment per font.
