## 1. Cargo Workspace Setup

- [x] 1.1 Create root `Cargo.toml` with workspace members and binary definition
- [x] 1.2 Create `crates/lain-types/` with `Cargo.toml` (serde, thiserror) and `src/lib.rs`
- [x] 1.3 Implement `lain-types/src/ids.rs` — PtyId newtype with Serialize/Deserialize/Debug/Clone/Copy/PartialEq/Eq/Hash
- [x] 1.4 Implement `lain-types/src/error.rs` — LainError enum with thiserror, serde derives
- [x] 1.5 Create `crates/lain-core/` with `Cargo.toml` (alacritty_terminal, portable-pty, wgpu, winit, cosmic-text, glyphon git, tokio, lain-types) and `src/lib.rs`
- [x] 1.6 Create skeleton crates: `lain-navi`, `lain-motoko`, `lain-maggi`, `lain-wired` — each with `Cargo.toml` (depends on lain-types only) and minimal `src/lib.rs`
- [x] 1.7 Create `src/main.rs` stub that depends on all crates, verify `cargo check` passes

## 2. PTY and Terminal State

- [x] 2.1 Implement `lain-core/src/pty.rs` — create PTY via alacritty_terminal::tty::new(), detect shell from $SHELL, spawn child process
- [x] 2.2 Implement `lain-core/src/terminal.rs` — create `alacritty_terminal::Term` with config, wrap in `Arc<FairMutex<Term>>`, start alacritty EventLoop on dedicated thread, expose EventLoopSender for input
- [x] 2.3 Wire PTY + terminal in main.rs, verify shell spawns (print to stdout for now, no rendering)

## 3. wgpu Surface and Window

- [x] 3.1 Set up winit window creation with event loop in main.rs
- [x] 3.2 Initialize wgpu instance, adapter, device, queue, and surface from winit window
- [x] 3.3 Handle window resize: reconfigure wgpu surface, recalculate cell grid dimensions (cols x rows from pixel size and cell metrics)

## 4. Text Rendering Pipeline

- [x] 4.1 Define internal `TextRenderer` trait in `lain-core/src/renderer/mod.rs` with prepare/render/resize methods
- [x] 4.2 Implement `GlyphonRenderer` in `lain-core/src/renderer/glyphon_backend.rs` — initialize cosmic-text FontSystem, glyphon TextAtlas and TextRenderer
- [x] 4.3 Implement cell-to-text mapping: lock Term, iterate `renderable_content()` cells, build cosmic-text text areas with correct grid positions, foreground colors, and font attributes (bold/italic)
- [x] 4.4 Implement `lain-core/src/renderer/rect.rs` — wgpu quad pipeline for cell background colors (vertex buffer of colored rectangles, simple vertex/fragment shader)
- [x] 4.5 Implement cursor rendering: read cursor position and shape from Term, render as colored rectangle at correct grid position

## 5. Frame Orchestration

- [x] 5.1 Implement `lain-core/src/renderer/pipeline.rs` — frame loop: lock Term → prepare backgrounds → prepare text (glyphon) → begin wgpu render pass → draw backgrounds → draw text → present
- [x] 5.2 Wire the full pipeline in main.rs: winit event → match on RedrawRequested → call pipeline → request_redraw on Term changes
- [x] 5.3 Trigger re-render when alacritty EventLoop signals new content (EventLoop event callback → request_redraw)

## 6. Input Routing

- [x] 6.1 Translate winit keyboard events to terminal byte sequences (printable chars, Enter, Backspace, Tab, Escape, arrow keys, Ctrl+key combinations)
- [x] 6.2 Send translated bytes to PTY via EventLoopSender
- [x] 6.3 Handle window resize event: update PTY dimensions via alacritty EventLoop Msg::Resize, notify Term of new size

## 7. Integration and Validation

- [x] 7.1 End-to-end test: launch app, verify shell prompt appears, type `echo hello`, see output (manual — requires graphical session)
- [x] 7.2 Verify ANSI colors: run `ls --color=auto` or similar, confirm colored output renders (manual)
- [x] 7.3 Verify resize: resize window, confirm `stty size` reports new dimensions, content reflows (manual)
- [x] 7.4 Verify Ctrl+C sends SIGINT and Ctrl+D sends EOF (shell exits) (manual)
- [x] 7.5 Verify clean shutdown: close window, confirm no orphaned shell process (manual)
