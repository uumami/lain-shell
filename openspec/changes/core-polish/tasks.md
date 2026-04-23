## 1. Dependencies

- [x] 1.1 Add `arboard = "3"` to `crates/lain-core/Cargo.toml`

## 2. EventListener — Proxy Missing Events

- [x] 2.1 Extend `TerminalEvent` enum in `terminal.rs` with variants: `ClipboardStore(String)`, `ClipboardLoad(Arc<dyn Fn(&str) -> String + Send + Sync>)`, `PtyWrite(String)`, `Title(String)`, `TextAreaSizeRequest(Arc<dyn Fn(alacritty_terminal::event::WindowSize) -> String + Send + Sync>)`
- [x] 2.2 Update `JsonLessListener::send_event()` to handle `Event::ClipboardStore(_, text)` → `TerminalEvent::ClipboardStore(text)`, `Event::ClipboardLoad(_, cb)` → `TerminalEvent::ClipboardLoad(cb)`, `Event::PtyWrite(s)` → `TerminalEvent::PtyWrite(s)`, `Event::Title(s)` → `TerminalEvent::Title(s)`, `Event::TextAreaSizeRequest(cb)` → `TerminalEvent::TextAreaSizeRequest(cb)`

## 3. Handle Proxied Events in main.rs

- [x] 3.1 In `App::user_event()`, handle `TerminalEvent::PtyWrite(s)` → `terminal.send_input(s.as_bytes())`
- [x] 3.2 Handle `TerminalEvent::Title(s)` → `window.set_title(&s)`
- [x] 3.3 Handle `TerminalEvent::TextAreaSizeRequest(cb)` → get `(cell_w, cell_h)` from renderer, compute pixel size, call `cb(window_size)`, send result via `terminal.send_input()`
- [x] 3.4 Handle `TerminalEvent::ClipboardStore(text)` → `arboard::Clipboard::new()?.set_text(text)`
- [x] 3.5 Handle `TerminalEvent::ClipboardLoad(cb)` → `arboard::Clipboard::new()?.get_text()`, format via `cb(&text)`, send via `terminal.send_input()`

## 4. Scrollback

- [x] 4.1 Add `NamedKey::PageUp` and `NamedKey::PageDown` handling in `Key::Named` match in `main.rs` — call `term.lock().scroll_display(Scroll::PageUp/PageDown)` then `window.request_redraw()`; do NOT forward these keys to the PTY
- [x] 4.2 Handle `WindowEvent::MouseWheel` in `App::window_event()` — check `TermMode::MOUSE_MODE` on the term; if set, encode as mouse scroll sequences and send to PTY; if not set, call `term.lock().scroll_display(Scroll::Delta(lines))` and `window.request_redraw()`
- [x] 4.3 Add `display_offset: usize` field to `CellGrid`; set it in `CellGrid::extract()` from `term.grid().display_offset()`

## 5. Mouse State Tracking

- [x] 5.1 Add mouse state fields to `App`: `mouse_pos: (f64, f64)`, `mouse_button_held: bool`, `active_selection: bool`
- [x] 5.2 Handle `WindowEvent::CursorMoved` — store position in `self.mouse_pos`; if `mouse_button_held` and `MOUSE_MOTION` mode set, encode motion event and send to PTY; if `mouse_button_held` and selection mode, call `term.lock().selection_mut().update(point, side)` and `window.request_redraw()`

## 6. Mouse Reporting to PTY

- [x] 6.1 Add helper `fn encode_mouse_button(button: u8, col: usize, row: usize, pressed: bool, sgr: bool) -> Vec<u8>` — returns X10 bytes `\x1b[M<b+32><x+32><y+32>` or SGR `\x1b[<b;x;yM/m` depending on `sgr` flag
- [x] 6.2 Handle `WindowEvent::MouseInput` — check `TermMode::MOUSE_REPORT_CLICK`; if set, call `encode_mouse_button()` with `TermMode::SGR_MOUSE` flag, send to PTY, set/clear `mouse_button_held`; if not set, begin/end selection (see section 7)

## 7. Text Selection

- [x] 7.1 On mouse button press (no mouse mode): compute terminal `(col, row)` from pixel position using `cell_w`, `cell_h`, and `cell_grid.display_offset`; call `term.lock().start_selection(SelectionType::Simple, point, Side::Left)`; set `mouse_button_held = true`, `active_selection = true`
- [x] 7.2 On cursor moved with button held (no mouse mode): update selection via `term.lock().selection_mut().update(point, Side::Right)`; request redraw
- [x] 7.3 On mouse button release (no mouse mode): finalize selection; call `term.lock().selection_to_string()`; if Some(text), write to PRIMARY via `arboard::Clipboard::new()?.set_primary_text(text)`; set `mouse_button_held = false`

## 8. Copy / Paste Keyboard Shortcuts

- [x] 8.1 In `Key::Character` handler in `main.rs`, detect Ctrl+Shift+C (control key + shift key + `c`): get `term.lock().selection_to_string()`; if Some(text), write to CLIPBOARD via arboard; do NOT forward to PTY
- [x] 8.2 Detect Ctrl+Shift+V: read CLIPBOARD via arboard; if Some(text), call `terminal.send_input(text.as_bytes())`; do NOT forward the raw keystroke to PTY

## 9. Verification

- [x] 9.1 `cargo check` passes with no warnings
- [x] 9.2 `cargo build` succeeds (selection highlight rendering added to cell_grid.rs)
- [ ] 9.3 Test scrollback: run `ls -la /usr/bin`, scroll up with wheel and PageUp/PageDown, verify history visible
- [ ] 9.4 Test clipboard: select text with mouse, verify middle-click pastes it; press Ctrl+Shift+C, paste in another app
- [ ] 9.5 Test Ctrl+Shift+V: copy text in another app, press Ctrl+Shift+V in lain-shell, verify text appears
- [ ] 9.6 Test OSC52: open vim, yank a word with `"+yiw`, verify text available in system clipboard
- [ ] 9.7 Test mouse reporting: run `htop`, verify mouse clicks work; run `fzf`, verify mouse selection works
- [ ] 9.8 Test window title: run `echo -ne "\033]0;test title\007"`, verify window title bar updates
