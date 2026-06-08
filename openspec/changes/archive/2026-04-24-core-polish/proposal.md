## Why

lain-shell is usable as a terminal but not yet liveable — no scrollback, no clipboard, and mouse events are silently dropped. These are the table stakes every terminal user expects; without them the shell cannot serve as a daily driver or a foundation for MAGGI.

## What Changes

- **Scrollback**: mouse wheel and PageUp/PageDown scroll the terminal viewport through the already-collected 10,000-line history buffer
- **EventListener repair**: `JsonLessListener` currently drops `PtyWrite`, `TextAreaSizeRequest`, `Title`, `ClipboardStore`, and `ClipboardLoad` events silently; these will be proxied through the winit event loop and handled in `main.rs`
- **Clipboard**: mouse selection copies to PRIMARY (X11) / primary selection (Wayland); Ctrl+Shift+C copies selection to CLIPBOARD; Ctrl+Shift+V pastes CLIPBOARD into PTY; OSC52 sequences (vim/nvim clipboard integration) work via the EventListener fix
- **Mouse reporting**: when a running program enables terminal mouse mode (htop, vim, fzf), mouse events are translated to ANSI sequences and forwarded to the PTY; when mouse mode is off, mouse drives selection instead
- **Window title**: terminal title-change escape sequences update the winit window title

## Capabilities

### New Capabilities

- `terminal-scrollback`: viewport scroll through history via wheel and keyboard
- `terminal-clipboard`: system clipboard integration — selection, copy, paste, OSC52
- `terminal-mouse`: mouse event forwarding to PTY and selection when mouse mode is off

### Modified Capabilities

- `input-routing`: extended to cover mouse input routing (report to PTY vs. drive selection based on TermMode) and new keyboard shortcuts (Ctrl+Shift+C/V, PageUp/Down for scroll)

## Impact

- `crates/lain-core/src/terminal.rs`: `JsonLessListener` extended; `TerminalEvent` enum gains new variants for clipboard proxying
- `src/main.rs`: new `WindowEvent` handlers for `MouseWheel`, `CursorMoved`, `MouseInput`; new `TerminalEvent` handlers for clipboard; `Ctrl+Shift+C/V` shortcuts in keyboard handler
- `crates/lain-core/Cargo.toml`: adds `arboard` dependency (X11 + Wayland clipboard)
- No breaking changes to public API; `Terminal` and `CellGrid` structs unchanged
