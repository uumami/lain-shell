## 1. CellInfo Extended Attributes

- [x] 1.1 Add `italic: bool`, `underline: bool`, `strikethrough: bool`, `dim: bool` fields to `CellInfo` struct in `cell_grid.rs`
- [x] 1.2 Update `CellGrid::resize()` default `CellInfo` to include the new fields as `false`
- [x] 1.3 Update `CellGrid::extract()` to populate all four flags from `alacritty_terminal::term::cell::Flags` (ITALIC, UNDERLINE, STRIKEOUT, DIM_BOLD)
- [x] 1.4 Update the comparison / clone usage of `CellInfo` in dirty-row detection to include new fields

## 2. Text Shaping — Italic and Dim

- [x] 2.1 In `text_shaping.rs`, import `cosmic_text::Style` and add `style` to `Attrs` when `cell.italic` is true
- [x] 2.2 In `text_shaping.rs`, apply dim: when `cell.dim` is true multiply each channel of `cell.fg` by 0.6 before passing as `Attrs::color`

## 3. Underline and Strikethrough Rendering — CPU

- [x] 3.1 In `cpu.rs` `render_frame()`, after the glyph render pass for each dirty row, iterate cells and draw a 1px underline rect at `y = row * cell_h + cell_h - 2` in fg color when `cell.underline` is true
- [x] 3.2 In `cpu.rs`, draw a 1px strikethrough rect at `y = row * cell_h + (cell_h * 0.6) as u32` in fg color when `cell.strikethrough` is true

## 4. Underline and Strikethrough Rendering — GPU

- [x] 4.1 In `gpu.rs` `render_frame()`, when building `rect_instances`, add underline `RectInstance` for each cell where `cell.underline` is true
- [x] 4.2 In `gpu.rs`, add strikethrough `RectInstance` for each cell where `cell.strikethrough` is true
- [x] 4.3 Confirm rect instances are rendered after (on top of) text in the render pass order

## 5. Font Metrics — Measure from Font

- [x] 5.1 Add a `measure_cell_width(font_system: &mut FontSystem, font_size: f32) -> f32` helper function in `text_shaping.rs` that lays out `"MMMMMMMMMM"` and returns `(run.line_w / 10.0).round()`, with fallback to `font_size * 0.6`
- [x] 5.2 In `GlyphonRenderer::new()`, call `measure_cell_width()` after creating the `FontSystem` and use the result as `self.cell_width`
- [x] 5.3 In `CpuRenderer::new()`, call `measure_cell_width()` and use the result as `self.cell_width`

## 6. HiDPI Scale Factor

- [x] 6.1 In `GlyphonRenderer`, replace the hardcoded `font_size = 14.0` with `pub fn new_with_scale(device, queue, format, scale_factor: f32) -> Self` that computes `physical_font_size = 14.0 * scale_factor`; pass it to `measure_cell_width()` and to `Metrics::new()`
- [x] 6.2 In `CpuRenderer`, similarly accept `scale_factor: f32` in `new()` and use `physical_font_size = 14.0 * scale_factor`
- [x] 6.3 In `main.rs` `resumed()`, read `window.scale_factor() as f32` and pass it when constructing the renderer
- [x] 6.4 Handle `WindowEvent::ScaleFactorChanged { scale_factor, .. }` in `main.rs`: reconstruct renderer metrics at the new scale, resize cell_grid and terminal, request redraw

## 7. vsync Presentation Mode

- [x] 7.1 In `gpu.rs`, change `present_mode: wgpu::PresentMode::AutoNoVsync` to `wgpu::PresentMode::AutoVsync` in `SurfaceConfiguration`
- [x] 7.2 Verify that surface reconfigure in `resize()` and error-recovery path also use `AutoVsync` (they inherit from `self.config` so should be automatic)

## 8. Frame Gate — Dirty Flag + WaitUntil

- [x] 8.1 Add `needs_redraw: bool`, `last_frame: Instant`, and define `const FRAME_INTERVAL: Duration = Duration::from_micros(16_667)` in `main.rs`
- [x] 8.2 In `user_event()`, handle `TerminalEvent::Wakeup`: set `needs_redraw = true`; if `now - last_frame >= FRAME_INTERVAL`, call `window.request_redraw()`; else call `event_loop.set_control_flow(ControlFlow::WaitUntil(last_frame + FRAME_INTERVAL))`
- [x] 8.3 In `window_event()`, on `WindowEvent::RedrawRequested`: after rendering, set `last_frame = Instant::now()`, set `needs_redraw = false`
- [x] 8.4 In `window_event()`, on `WindowEvent::KeyboardInput` with state `Pressed`: call `window.request_redraw()` unconditionally (before input processing), bypassing the frame gate

## 9. Cursor Blink

- [x] 9.1 Add `cursor_blink_visible: bool = true`, `cursor_focused: bool = true` fields to `App`
- [x] 9.2 After `cell_grid.extract()` in `RedrawRequested`, set `cell_grid.cursor.visible = self.cursor_blink_visible`
- [x] 9.3 After rendering in `RedrawRequested`, if `cursor_focused` is true, schedule next blink: `event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(500)))`
- [x] 9.4 In `user_event()`, handle the WaitUntil wake-up (timer fires with no event): if focused, toggle `cursor_blink_visible`, request_redraw, reschedule WaitUntil
- [x] 9.5 In `window_event()`, on `WindowEvent::KeyboardInput` Pressed: set `cursor_blink_visible = true` (always show on keypress)
- [x] 9.6 Handle `WindowEvent::Focused(true/false)`: set `cursor_focused`; on false, set `cursor_blink_visible = true` (solid, non-blinking while unfocused)

## 10. Verification

- [x] 10.1 `cargo check` passes with no warnings
- [x] 10.2 `cargo build` succeeds
- [x] 10.3 Run with GPU: cursor blinks at ~500ms interval
- [x] 10.4 Run with CPU: cursor blinks at ~500ms interval
- [x] 10.5 Run `cat /etc/passwd` rapidly: observe smooth output with no frame bursts
- [x] 10.6 Type in the shell: confirm keystrokes feel immediate (no perceivable lag)
- [x] 10.7 Run `htop`: confirm box-drawing characters are aligned with no gaps
- [x] 10.8 Run nvim with a colorscheme using italic comments: confirm italics render
- [x] 10.9 Run a program that sets underline (e.g., `man ls`): confirm underline appears
- [x] 10.10 Move window to a different monitor (if DPI differs): confirm text stays sharp
