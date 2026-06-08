## Context

lain-shell has a working PTY + VTE + rendering pipeline. `alacritty_terminal` already collects 10,000 lines of scrollback history and provides `term.scroll_display()`. Mouse events from winit are not handled at all. The `JsonLessListener` (PTY thread event listener) silently drops five event types that programs rely on. There is no clipboard integration.

The challenge is that clipboard operations need to cross three boundaries: the PTY thread (where `alacritty_terminal` emits clipboard events), the winit event loop thread (where we have access to the window and terminal sender), and the system clipboard (which may block).

## Goals / Non-Goals

**Goals:**
- Scrollback via mouse wheel and PageUp/PageDown
- OSC52 clipboard (vim/nvim `"+y` integration) working via EventListener fix
- User selection → PRIMARY clipboard (auto on mouse-up)
- Ctrl+Shift+C → CLIPBOARD, Ctrl+Shift+V → paste from CLIPBOARD
- Mouse events forwarded to PTY when program enables mouse mode (htop, fzf, vim)
- Mouse drives text selection when program has not enabled mouse mode
- Window title updates from escape sequences

**Non-Goals:**
- Vi-style copy mode (Navi's responsibility)
- Pane-aware clipboard (Navi's responsibility)
- Cursor blinking (separate concern, low priority)
- Kitty keyboard protocol
- Sixel / image rendering

## Decisions

### Decision 1: Proxy clipboard events through winit event loop (Option A)

`JsonLessListener` runs on the PTY thread. `Event::ClipboardLoad` needs to read the system clipboard and write the result back to the PTY. Doing this on the PTY thread risks blocking it; arboard can block on Wayland.

**Decision**: Extend `TerminalEvent` with clipboard variants. `JsonLessListener` re-wraps `ClipboardStore` and `ClipboardLoad` into `TerminalEvent` values and fires them at the winit proxy. `main.rs` handles them in `user_event()` where it already has access to the `Terminal` sender, the window, and can safely call arboard.

```
PTY thread:   Event::ClipboardStore(text)  →  TerminalEvent::ClipboardStore(text)
              Event::ClipboardLoad(cb)     →  TerminalEvent::ClipboardLoad(cb)
              Event::PtyWrite(s)           →  TerminalEvent::PtyWrite(s)
              Event::Title(s)              →  TerminalEvent::Title(s)
              Event::TextAreaSizeRequest   →  TerminalEvent::TextAreaSizeRequest(cb)

winit thread: TerminalEvent::ClipboardStore  →  arboard::set_text()
              TerminalEvent::ClipboardLoad   →  arboard::get_text() → send_input(cb(text))
              TerminalEvent::PtyWrite        →  send_input(bytes)
              TerminalEvent::Title           →  window.set_title()
              TerminalEvent::TextAreaSizeRequest → send_input(cb(size))
```

**Alternatives considered**: Threading the `EventLoopSender` into the listener (Alacritty's approach) — rejected because it adds complexity, can block the PTY thread on clipboard ops, and is harder to intercept at the Navi layer later.

### Decision 2: arboard for system clipboard (X11 + Wayland)

arboard abstracts X11 (`CLIPBOARD` + `PRIMARY` atoms) and Wayland (`wl-data-device` + `zwp_primary_selection_v1`). It spawns a background thread on X11 to maintain clipboard ownership without blocking our threads.

**Alternatives considered**: `wl-clipboard` CLI tool — rejected (external process, not portable to X11). winit clipboard extensions — not available in winit 0.30.

### Decision 3: Mouse mode governs click/drag behavior

`TermMode` flags in `alacritty_terminal` track whether the running program has enabled mouse reporting. When `MOUSE_REPORT_CLICK` or `MOUSE_MOTION` is set, click and drag events are encoded as ANSI mouse sequences and forwarded to the PTY. When those flags are clear, click and drag drive text selection.

Scroll wheel is handled independently: when `MOUSE_MODE` (scroll forwarding) is set, scroll events are forwarded as ANSI sequences; otherwise they call `term.scroll_display()`.

```
MouseWheel   →  if MOUSE_MODE set: encode → PTY
                else: term.scroll_display(Delta)

MouseInput   →  if MOUSE_REPORT_CLICK set: encode → PTY
                else: begin/update selection on term

CursorMoved  →  if MOUSE_MOTION set AND button held: encode → PTY
                else if button held: update selection on term
```

**ANSI mouse encoding**: X10 (`\x1b[Mbxy`) for basic reporting; SGR (`\x1b[<Mb;x;yM/m`) when `SGR_MOUSE` mode is set. Check `TermMode::SGR_MOUSE` to select encoding.

### Decision 4: Selection → PRIMARY on mouse-up; Ctrl+Shift+C → CLIPBOARD

Unix convention: selecting text automatically copies to PRIMARY (available via middle-click). Explicit copy (Ctrl+Shift+C) copies to CLIPBOARD. Paste (Ctrl+Shift+V) reads from CLIPBOARD. Middle-click pastes from PRIMARY.

This maps directly to arboard's `Set::primary()` and `Set::clipboard()` APIs.

### Decision 5: display_offset tracked in CellGrid for mouse hit-testing

When the user is scrolled up, a pixel coordinate must map to a history row, not a viewport row. `term.grid().display_offset()` gives the number of lines scrolled above the viewport. `CellGrid.extract()` already reads the term under a lock; it will also read and store `display_offset` as a field so renderers and mouse handlers can use it without taking the term lock again.

## Risks / Trade-offs

- **Wayland primary selection**: `zwp_primary_selection_v1` is optional in the Wayland spec. Some minimal compositors don't implement it. Middle-click paste silently fails on those compositors. Mitigation: no crash, just no-op.
- **arboard background thread on X11**: arboard spawns a thread to respond to `XConvertSelection` requests. This is well-established but adds a background thread. Mitigation: arboard handles the lifecycle; no action needed.
- **Selection and scrollback scroll position**: if the user scrolls up and selects text, the selection range spans history rows. `term.selection_to_string()` handles this correctly since `Selection` operates on absolute grid coordinates. We just need correct `display_offset` tracking.
- **Navi layering**: when Navi arrives, it will intercept mouse and keyboard events before Core sees them (for pane navigation). The current direct wiring in `main.rs` will need to be routed through Navi's dispatcher. This is expected — no special mitigation needed now.
