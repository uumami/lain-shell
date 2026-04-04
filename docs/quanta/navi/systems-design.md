# Navi — Systems Design

> *Implementation details for the multiplexer and session manager.*

---

## Decided Technology

Navi is custom-built (ADR-001). No tmux dependency. No Zellij dependency.

| Component | Choice | Notes |
|---|---|---|
| PTY creation | Via Core (`portable-pty`) | Navi requests PTYs from Core, does not manage them directly |
| Session persistence | Custom TOML serialization | Typed sessions → TOML, stored in state directory |
| Attach/detach | Unix domain socket | Server in Navi process, clients connect to reattach |
| Layout data structure | Binary tree | tmux-validated approach. Each node is a split or a leaf (pane). |
| Keybinding dispatch | Custom | Configurable, ships with tmux-compatible and Zellij-style keymaps |

---

## Key Design Questions (Remaining)

1. **Layout algorithm.** Binary tree of splits is the starting point (tmux-validated). Each internal node is a horizontal or vertical split with a ratio. Each leaf is a pane. Resize operations adjust ratios. Zoom temporarily makes one pane full-screen.

2. **Session serialization.** What survives a restart:
   - Layout tree structure and split ratios
   - Working directory per pane
   - Pane type (shell, agent, container)
   - Agent metadata (which agent, what model, permission scope)
   - Environment variables marked for persistence

   What is reconstructed:
   - PTY instances (respawned)
   - Scrollback content (lost unless session recording is enabled)
   - Agent conversation state (MAGGI manages its own persistence)

3. **Attach/detach mechanism.** Navi process owns sessions and PTYs. When the terminal window closes (detach), the Navi process keeps running. Reattaching means a new renderer connects to the existing Navi process via Unix domain socket at a well-known path (`/run/user/$UID/lain/`).

4. **Keybinding system.** Ships with multiple keymaps:
   - **lain-shell native** — designed fresh, optimized for the feature set
   - **tmux-compatible** — prefix-based (`Ctrl-b` default), familiar muscle memory
   - **Zellij-style** — discoverable, with a hint bar showing available actions

   Users can create custom keymaps. Keybindings are defined in TOML.

5. **IPC with Core.** Navi and Core are in the same process (same Rust binary). Communication via typed async channels (tokio mpsc). Navi sends: "create PTY with these parameters", "allocate render surface at these coordinates". Core responds with handles.

---

## Server-Client Model

```
┌──────────────────────────┐
│     Navi Server           │
│  (long-running process)   │
│                           │
│  Sessions ──┬── Window 1  │
│             │   ├─ Pane A │
│             │   └─ Pane B │
│             └── Window 2  │
│                 └─ Pane C │
│                           │
│  Unix socket listener     │
│  /run/user/$UID/lain/     │
└──────────┬───────────────┘
           │
    ┌──────┴──────┐
    │             │
┌───▼───┐   ┌───▼───┐
│Client │   │Client │
│(term  │   │(term  │
│window)│   │window)│
└───────┘   └───────┘
```

This is the tmux server-client pattern. The server owns state. Clients render. Detach = client disconnects. Reattach = new client connects. Multiple clients can attach to the same session (pair programming).

> **Open question:** When multiple clients attach, do they all see the same active window? Or can each client navigate independently? tmux supports both modes. Navi should too.

---

## References — Prior Art

### tmux Internals

tmux is now "study, don't use" (ADR-001). But its implementation encodes decades of multiplexer engineering.

**Study directly:**
- **Server-client model** — tmux runs a server process, clients connect. Navi uses the same pattern. Study how tmux manages the server lifecycle (when does it start? when does it exit? what happens on crash?).
- **Layout tree** — tmux uses a binary tree for pane layout. Each node stores a split direction and position. Study `layout.c` for how resize, zoom, and layout presets work.
- **Control mode protocol** — even though Navi doesn't use it, the protocol design shows what operations a multiplexer needs to expose programmatically.
- **Session persistence** — tmux doesn't natively persist sessions across restarts (that's what tmux-resurrect does). Study tmux-resurrect's approach to session serialization.

**Borrow:** Server-client architecture, layout tree data structure, attach/detach semantics, keyboard prefix model.
**Avoid:** String-based metadata, lack of typed sessions, no permission model, text-based control protocol.

### Zellij Architecture

Zellij is written in Rust. Its source code is directly readable and relevant.

**Study directly:**
- **`Screen` / `Tab` / `Pane` hierarchy** — Zellij's internal model for multiplexing. More modern than tmux's C implementation.
- **Layout DSL** — Zellij has a KDL-based layout definition. Study how layouts are defined declaratively and applied.
- **WASM plugin system** — Zellij runs plugins in WASM. This directly informs lain-shell's plugin sandboxing via `wasmtime`. Study what API surface Zellij exposes to plugins, what the performance overhead is, and what the pain points are (startup latency, limited API).
- **Discoverable keybindings** — Zellij's status bar shows available keybindings contextually. Inspiration for Navi's Zellij-style keymap option.

**Borrow:** Rust multiplexer architecture patterns, WASM plugin model, discoverable UI.
**Avoid:** Zellij's specific layout format (lain-shell uses TOML). Zellij as a dependency.

### WezTerm's Built-in Multiplexer

WezTerm embeds its own multiplexer rather than depending on tmux. This validates that a Rust terminal can build multiplexing internally.

**Study:** How WezTerm manages its pane model, SSH multiplexing, and session tabs. The multiplexer is tightly integrated with the renderer — relevant to how Navi integrates with Core.
