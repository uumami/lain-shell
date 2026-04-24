## Why

The renderer feels choppy and laggy: every PTY wakeup triggers an immediate full redraw with no frame pacing, producing irregular frame timing and input latency up to 33ms. Font metrics are guessed (`font_size * 0.6`) rather than measured from the actual font, causing sub-pixel misalignment visible in box-drawing characters. Text attributes beyond bold (italic, underline, dim) are silently dropped. These issues affect every user regardless of terminal program, display hardware, or DPI setting.

## What Changes

- **Frame-gated render loop**: coalesce PTY wakeups behind a 60fps frame gate; keystrokes bypass the gate for immediate response
- **vsync presentation**: switch from `AutoNoVsync` to `AutoVsync` for consistent frame timing
- **Font metrics measured from font**: query cosmic-text for the real advance width and line metrics at startup and on scale_factor change
- **HiDPI scale factor support**: multiply logical font size by `window.scale_factor()` for crisp text on 2x/1.5x displays; handle `ScaleFactorChanged` events
- **Cursor blink**: 500ms blink cycle using `ControlFlow::WaitUntil`; resets to visible on any keypress
- **Text attributes**: italic (cosmic-text `Style`), underline, strikethrough, and dim rendered correctly in both CPU and GPU paths

## Capabilities

### New Capabilities
- `frame-pacing`: Frame-gated render loop that coalesces PTY wakeups, limits to display refresh rate, and gives keystrokes immediate render priority
- `font-metrics`: Derivation of cell width, cell height, and baseline from the actual loaded font at runtime, with HiDPI scale factor applied
- `cursor-blink`: Blinking cursor with configurable period, automatic reset on input, and stop-on-focus-loss
- `text-attributes`: Full rendering of italic, underline, strikethrough, and dim cell attributes in both renderers

### Modified Capabilities
- `wgpu-cell-rendering`: vsync presentation mode and font metrics sourced from `font-metrics` capability instead of hardcoded constants

## Impact

- `src/main.rs`: event loop control flow, frame dirty flag, keystroke fast-path, cursor blink timer
- `crates/lain-core/src/renderer/glyphon_backend.rs`: font metrics measurement, vsync, scale factor
- `crates/lain-core/src/renderer/cpu.rs`: font metrics from font, scale factor, text attributes
- `crates/lain-core/src/renderer/text_shaping.rs`: italic style, underline/strikethrough as rects
- `crates/lain-core/src/renderer/cell_grid.rs`: `CellInfo` extended with italic/underline/strikethrough/dim flags; `CursorState` extended with `blink_visible`
- No new external dependencies required (all uses are within existing cosmic-text / wgpu / winit APIs)
