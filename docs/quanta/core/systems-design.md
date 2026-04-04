# Core — Systems Design

> *Implementation details, data structures, algorithms, technology choices.*

---

## Decided Technology

| Component | Choice | ADR |
|---|---|---|
| VTE parser / terminal state | `alacritty_terminal` | — |
| PTY management | `portable-pty` | — |
| GPU rendering | `wgpu` | ADR-004 |
| CPU fallback | `softbuffer` | — |
| Font shaping | `cosmic-text` | — |
| Async runtime | `tokio` | — |
| Config format | TOML via `serde` + `toml` | — |
| Config validation | JSON Schema (published via `lain schema`) | — |
| Advanced config | Lua via `mlua` (optional layer, not default) | — |
| Plugin sandbox | `wasmtime` (WASM) | — |
| Memory allocator | System allocator initially; benchmark `jemalloc` under real load | — |

---

## Block Model

Each command and its output form a structured unit — not raw scrollback bytes. See ADR-005.

Modern shells (bash 5.x, zsh, fish) emit shell integration escape sequences (OSC 133) marking:
- Command start (prompt end)
- Command text
- Command execution start
- Command finish + exit code

`alacritty_terminal` parses these sequences. Core maintains a structured command history alongside the raw cell grid:

```
CommandBlock {
    command: String,
    working_dir: PathBuf,
    start_line: usize,
    end_line: usize,
    exit_code: Option<i32>,
    timestamp: Timestamp,
    duration: Duration,
}
```

Agents query blocks through THE WIRED. MAGGI uses blocks for context. Navi can collapse/expand blocks visually.

> **Open question:** How to handle shells that don't emit OSC 133? Fallback to raw scrollback with no block structure? Attempt heuristic prompt detection?

---

## Rendering Pipeline

### Architecture

```
alacritty_terminal         cosmic-text           wgpu
(terminal state)           (font shaping)        (GPU pipeline)
┌──────────────┐          ┌──────────────┐      ┌──────────────┐
│ Cell grid    │──cells──▶│ Shape text   │─────▶│ Glyph atlas  │
│ Scrollback   │          │ Layout runs  │      │ Cell vertices │
│ Selection    │          │ Bidi/emoji   │      │ Draw calls   │
│ Cursor       │          └──────────────┘      │ Compositor   │
└──────────────┘                                └──────┬───────┘
                                                       │
                                                ┌──────▼───────┐
                                                │ Layers:      │
                                                │ 1. Background│
                                                │ 2. Cell grid │
                                                │ 3. Selection │
                                                │ 4. Cursor    │
                                                │ 5. Overlays  │
                                                │ 6. Status bar│
                                                └──────────────┘
```

The compositor renders layers in order. Overlays (MOTOKO indicators, agent status, cost readouts) are real GPU layers with transparency — not escape sequence hacks.

### CPU fallback

When no GPU is available (headless, SSH, constrained environments), `softbuffer` provides a CPU-rendered framebuffer. Same layer model, rasterized in software. Performance is acceptable for basic use — complex overlays may be simplified.

### Glyph atlas

GPU texture atlas for rendered glyphs. `cosmic-text` shapes the text, a rasterizer (part of cosmic-text or `swash` if needed) produces bitmaps, the atlas caches them. LRU eviction when the atlas fills.

> **Open question:** Atlas size and eviction strategy. Start with a fixed 2048x2048 texture and measure. Ghostty's approach to atlas management is worth studying.

---

## Configuration Engine

### Format layers

1. **TOML** (default) — all `.lain/` files and user-global config. Parsed by `serde` + `toml`.
2. **JSON Schema** — every config file has a published schema. `lain schema` dumps them. Enables machine validation and editor completion.
3. **Lua** (optional) — via `mlua`. For users who want programmatic config (conditional settings, computed values). WezTerm's model is the inspiration. Not the default path.

### Config resolution order

1. Built-in defaults (compiled into binary)
2. User-global (`~/.config/lain-shell/`)
3. Workspace (`.lain/` in project directory)
4. Session overrides (runtime, not persisted)

Later sources override earlier ones. Conflicts are resolved per-key, not per-file.

### Hot-reload

> **Open question:** Which settings hot-reload vs require restart? Theme and visual settings should hot-reload. PTY and security settings likely require session restart.

---

## Key Design Questions (Remaining)

1. **Overlay compositor details.** How are overlay layers defined, positioned, and composed? Z-ordering? Transparency blending?
2. **Input pipeline.** Keyboard events → which quantum handles? Core dispatches to Navi for multiplexer keys, to MAGGI for agent input, to PTY for shell input?
3. **Event bus implementation.** Typed async channels (tokio mpsc/broadcast)? How do other quanta subscribe to Core events?
4. **Image rendering.** Kitty graphics protocol and/or sixel support via `alacritty_terminal`? How do images compose with the overlay system?
5. **Scrollback limits.** How much scrollback is kept in memory vs. paged to disk? Configurable?

---

## References — Prior Art

### Alacritty

`alacritty_terminal` is a direct dependency. Study Alacritty's integration between its VTE crate and its renderer for the cell grid → GPU pipeline pattern. Alacritty's renderer uses OpenGL (not wgpu), so the specific rendering code doesn't transfer, but the data flow pattern does.

**Borrow:** VTE correctness, cell grid model, scrollback buffer, vttest compliance culture.
**Avoid:** Alacritty's radical minimalism as a product philosophy. lain-shell is a platform.

### Ghostty

Open source (Zig). The fastest terminal renderer in existence. **The primary performance benchmark.**

**Study directly:**
- Rendering pipeline architecture — how Ghostty manages its glyph atlas, batches draw calls, and achieves minimal input-to-pixel latency.
- Font rendering approach — Ghostty handles font fallback, ligatures, and emoji with extreme care.
- vttest compliance — Ghostty's test suite and compliance approach.

**Borrow:** Performance bar, rendering architecture patterns, correctness culture.
**Avoid:** Zig as language choice (pre-1.0). The patterns transfer even though the code doesn't.

### Zed (GPUI + `alacritty_terminal`)

Zed's terminal view composes `alacritty_terminal` with GPUI for rendering. The integration pattern between VTE state and GPU rendering is directly studiable.

**Study directly:**
- `crates/terminal/src/terminal.rs` — the `Terminal` struct that wraps `alacritty_terminal`.
- `crates/terminal_view/` — how terminal state maps to rendered elements.
- The separation between `Terminal` (state), `TerminalView` (rendered element), and workspace context.

**Borrow:** The `alacritty_terminal` integration pattern. The state/view separation.
**Avoid:** GPUI as a dependency (see ADR-004). IDE-embedded assumptions.

### Warp

Closest existing product to lain-shell. Closed source — study only.

**Borrow:**
- The block model concept — each command as a discrete, queryable unit. This directly informs ADR-005.
- Proof that Rust + GPU rendering is production-viable.
- The project-level config file concept (analogous to `.lain/`).

**Avoid:**
- Cloud account requirement.
- Drift from "terminal" toward "AI platform that has a terminal." lain-shell is a terminal that hosts agents, not the reverse.

### Kitty

Created the Kitty graphics protocol (inline images) and the extended keyboard protocol (full key disambiguation). Both are de facto standards.

**Implement:**
- Kitty graphics protocol — inline image rendering. Relevant to MAGGI rendering structured output (diffs, diagrams, charts). `alacritty_terminal` has support.
- Kitty keyboard protocol — richer input handling for complex keybindings in overlays and agent UI.

**Borrow:** The philosophy of extending terminal capabilities through protocol design rather than application-layer hacks.

### Nushell

Shell that treats output as structured data — tables, records, lists.

**Inspiration for:**
- The block model's structured output concept. Even though lain-shell doesn't impose a shell, the internal session model can treat command output as structured events.
- Queryable history: `lain history query` or similar introspection.

**Avoid:** Replacing the user's shell. lain-shell is shell-agnostic.
