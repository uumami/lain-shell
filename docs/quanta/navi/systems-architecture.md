# Navi — Systems Architecture

> *Components, boundaries, interfaces, and data flow within the Navi quantum.*

---

## Session Hierarchy

```
Navi Server (one process)
│
├── Session "work" (persistent, named, attach/detach)
│   ├── Tab 1 "editor" (full-screen view, switchable)
│   │   ├── Pane A [shell: nvim]
│   │   └── Pane B [shell: cargo watch]
│   └── Tab 2 "agents"
│       ├── Pane C [agent: claude-code, pod: isolated]
│       └── Pane D [shell: git log]
│
├── Session "ops"
│   └── Tab 1 "monitoring"
│       ├── Pane A [shell: htop]
│       └── Pane B [read-only: log tail]
│
└── Session "background"
    └── Tab 1
        └── Pane A [agent: analysis, pod: isolated]
```

See ADR-007 for the Session → Tab → Pane hierarchy decision.

---

## Components

```
┌──────────────────────────────────────────────────┐
│                     NAVI                          │
│                                                   │
│  ┌──────────────┐  ┌──────────────┐              │
│  │   Session    │  │    Layout    │              │
│  │   Manager    │  │    Engine    │              │
│  └──────┬───────┘  └──────┬───────┘              │
│         │                 │                       │
│  ┌──────┴───────┐  ┌──────┴───────┐              │
│  │    Pane      │  │  Keybinding  │              │
│  │   Registry   │  │  Dispatcher  │              │
│  └──────────────┘  └──────────────┘              │
│                                                   │
│  ┌──────────────┐  ┌──────────────┐              │
│  │ Persistence  │  │   Attach/    │              │
│  │    Layer     │  │   Detach     │              │
│  └──────────────┘  └──────────────┘              │
└──────────────────────────────────────────────────┘
```

### Session Manager

CRUD operations on sessions. Tracks all active sessions, their tabs, and their panes. Manages session metadata (name, workspace, creation time, agent context).

### Layout Engine

Binary tree of splits within each tab. Each internal node: horizontal or vertical split with a ratio. Each leaf: a pane. Handles resize, zoom (temporarily full-screen one pane), and layout presets.

### Pane Registry

Tracks all panes and their types:
- **Shell pane**: plain PTY running user's shell
- **Agent pane**: PTY inside a container/namespace, running an agent
- **Read-only pane**: output-only (log viewer, status monitor)

Each pane has metadata: pane ID, type, PTY handle, optional isolation handle, working directory, agent info.

### Keybinding Dispatcher

Receives keyboard input from Core. Matches against active keymap. Dispatches to appropriate action (split pane, switch tab, navigate, resize, etc.).

Ships with multiple keymaps:
- **lain-shell native**: designed fresh for the feature set
- **tmux-compatible**: prefix-based (Ctrl-b), familiar muscle memory
- **Zellij-style**: discoverable, with hint bar

Custom keymaps defined in TOML.

### Persistence Layer

Serializes session state to TOML for restart recovery. Stored in `~/.local/state/lain-shell/sessions/`.

### Attach/Detach

Unix domain socket server at `/run/user/$UID/lain/navi.sock`. Clients (renderer instances) connect to view and interact with sessions. Detach = client disconnects. Reattach = new client connects and receives current state.

Multiple clients can attach to the same session simultaneously. Each client gets its own `AttachHandle` with independent view state (active tab, focused pane, scroll position, terminal size). Local and remote clients (via THE WIRED) are architecturally identical — both get an `AttachHandle`, the difference is transport fidelity.

---

## Data Model — Session State vs View State

Session state and view state are **separate concerns**. Session state is shared across all clients. View state is per-client. This separation is what makes multi-client attach work — two windows (local or remote) can view the same session independently.

### Session state (shared, lives on server)

```rust
struct Session {
    id: SessionId,
    name: String,
    tabs: Vec<Tab>,
    created_at: Timestamp,
    workspace: Option<WorkspacePath>,   // .lain/ directory
    security_context: SecurityContext,   // from MOTOKO
    cost_accumulator: CostAccumulator,
}

struct Tab {
    id: TabId,
    name: String,
    layout: LayoutTree,  // binary tree of splits
    // NOTE: no active_pane here — that is view state, per-client
}

struct Pane {
    id: PaneId,
    pane_type: PaneType,         // Shell, Agent, ReadOnly
    pty_handle: PtyHandle,       // from Core
    isolation_handle: Option<IsolationHandle>, // from Core's Isolation Manager
    working_dir: PathBuf,
    agent_info: Option<AgentInfo>,
    created_at: Timestamp,
}

enum PaneType {
    Shell,
    Agent { agent_type: String, model: Option<String> },
    ReadOnly,
}
```

### View state (per-client, lives on AttachHandle)

```rust
struct AttachHandle {
    id: AttachId,
    session: SessionId,
    active_tab: TabId,                              // which tab THIS client sees
    focused_pane: PaneId,                            // which pane gets THIS client's input
    terminal_size: TerminalSize,                     // THIS client's dimensions
    scroll_positions: HashMap<PaneId, ScrollOffset>, // per-pane scroll for THIS client
    client_type: ClientType,
    connected_at: Timestamp,
}

enum ClientType {
    Local,                          // Unix socket, full fidelity
    Remote { protocol: Protocol },  // gRPC via THE WIRED, needs differential updates
}
```

### Why the split matters

```
Client A (local, 200×50):
  AttachHandle { active_tab: Tab 2, focused_pane: Pane D, ... }

Client B (remote, 80×24):
  AttachHandle { active_tab: Tab 1, focused_pane: Pane A, ... }

Same session. Independent navigation.

Input from Client A → routed to Pane D's PTY
Input from Client B → routed to Pane A's PTY
PTY output → broadcast to all clients attached to that session
```

### Multi-client resize strategy

When multiple clients view the same pane, the PTY has one size. Strategy: **smallest client wins** (tmux-validated default).

```
Client A: 200×50, viewing Tab 1
Client B: 80×24, viewing Tab 1

Tab 1's PTY size: 80×24 (smallest of clients viewing it)
Client A sees the 80×24 content centered/aligned in its larger window.

If Client B switches to Tab 2:
Tab 1's PTY resizes to 200×50 (only Client A is viewing it now)
```

PTY size is recalculated when:
- A client attaches or detaches
- A client switches tabs
- A client resizes its terminal

### Multi-client output broadcast

Navi maintains a per-client output queue. When a PTY produces output:
1. VTE parser updates the shared cell grid
2. Navi sends the update to each client currently viewing that tab
3. Local clients get the full cell diff
4. Remote clients (via THE WIRED) get compressed differential updates

---

## Interface Exposed to Other Quanta

```rust
#[async_trait]
trait NaviApi: Send + Sync {
    // Sessions
    async fn create_session(&self, params: SessionParams) -> Result<SessionId>;
    async fn destroy_session(&self, id: SessionId) -> Result<()>;
    async fn list_sessions(&self) -> Result<Vec<SessionInfo>>;
    async fn get_session(&self, id: SessionId) -> Result<SessionDetail>;

    // Tabs (session state — affects all clients)
    async fn create_tab(&self, session: SessionId, params: TabParams) -> Result<TabId>;
    async fn close_tab(&self, session: SessionId, tab: TabId) -> Result<()>;

    // Panes (session state — affects all clients)
    async fn create_pane(&self, session: SessionId, tab: TabId, params: PaneParams) -> Result<PaneId>;
    async fn split_pane(&self, pane: PaneId, direction: SplitDirection) -> Result<PaneId>;
    async fn close_pane(&self, pane: PaneId) -> Result<()>;
    async fn resize_pane(&self, pane: PaneId, size: PaneSize) -> Result<()>;
    async fn get_pane_output(&self, pane: PaneId, lines: usize) -> Result<Vec<String>>;

    // View state (per-client — only affects the calling client)
    async fn switch_tab(&self, handle: AttachId, tab: TabId) -> Result<()>;
    async fn focus_pane(&self, handle: AttachId, pane: PaneId) -> Result<()>;
    async fn scroll_pane(&self, handle: AttachId, pane: PaneId, offset: ScrollOffset) -> Result<()>;

    // Agent lifecycle (MOTOKO calls these)
    async fn pause_session(&self, id: SessionId) -> Result<()>;
    async fn kill_session(&self, id: SessionId) -> Result<()>;
    async fn pause_pane_agent(&self, pane: PaneId) -> Result<()>;

    // Attach/detach
    async fn attach(&self, session: SessionId, client_type: ClientType) -> Result<AttachHandle>;
    async fn detach(&self, handle: AttachId) -> Result<()>;
    async fn list_attached(&self, session: SessionId) -> Result<Vec<AttachInfo>>;
}
```

---

## Kill Granularity

| Target | API call | What happens | What survives |
|---|---|---|---|
| Kill pane | `close_pane(pane_id)` | PTY dies, container destroyed if agent | Session, tab, other panes |
| Kill tab | `close_tab(session, tab)` | All panes die | Session, other tabs |
| Kill session | `destroy_session(session)` | All tabs and panes die | Other sessions |
| Kill agent isolation | `core.isolation_manager.destroy_isolation(handle)` | Namespace/container destroyed, pane stays | Session, tab, pane (shows exit) |
| Pause agent | `pause_pane_agent(pane_id)` → `core.isolation_manager.pause_isolation(handle)` | Agent frozen, pane shows "paused" | Everything stays, resumable |

---

## Integration with Other Quanta

- **Core**: Navi calls Core's PTY, Render, and Isolation APIs. Core streams PTY output and exit events back.
- **MOTOKO**: Navi emits session/agent lifecycle events to the bus. MOTOKO calls `pause_session`/`kill_session` on CRITICAL events.
- **MAGGI**: MAGGI calls Navi's API for session/tab/pane management. MAGGI queries session state for context.
- **THE WIRED**: External requests for session management route through the command router to Navi's API.
