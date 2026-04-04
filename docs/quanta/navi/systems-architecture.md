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

Each pane has metadata: pane ID, type, PTY handle, optional pod handle, working directory, agent info.

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

---

## Session Data Model

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
    active_pane: PaneId,
}

struct Pane {
    id: PaneId,
    pane_type: PaneType,         // Shell, Agent, ReadOnly
    pty_handle: PtyHandle,       // from Core
    pod_handle: Option<PodHandle>, // from Core's Pod Manager
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

    // Tabs
    async fn create_tab(&self, session: SessionId, params: TabParams) -> Result<TabId>;
    async fn switch_tab(&self, session: SessionId, tab: TabId) -> Result<()>;
    async fn close_tab(&self, session: SessionId, tab: TabId) -> Result<()>;

    // Panes
    async fn create_pane(&self, session: SessionId, tab: TabId, params: PaneParams) -> Result<PaneId>;
    async fn split_pane(&self, pane: PaneId, direction: SplitDirection) -> Result<PaneId>;
    async fn close_pane(&self, pane: PaneId) -> Result<()>;
    async fn resize_pane(&self, pane: PaneId, size: PaneSize) -> Result<()>;
    async fn get_pane_output(&self, pane: PaneId, lines: usize) -> Result<Vec<String>>;

    // Agent lifecycle (MOTOKO calls these)
    async fn pause_session(&self, id: SessionId) -> Result<()>;
    async fn kill_session(&self, id: SessionId) -> Result<()>;
    async fn pause_pane_agent(&self, pane: PaneId) -> Result<()>;

    // Attach/detach
    async fn attach(&self, session: SessionId) -> Result<AttachHandle>;
    async fn detach(&self, handle: AttachHandle) -> Result<()>;
}
```

---

## Kill Granularity

| Target | API call | What happens | What survives |
|---|---|---|---|
| Kill pane | `close_pane(pane_id)` | PTY dies, container destroyed if agent | Session, tab, other panes |
| Kill tab | `close_tab(session, tab)` | All panes die | Session, other tabs |
| Kill session | `destroy_session(session)` | All tabs and panes die | Other sessions |
| Kill agent pod | `core.pod_manager.destroy_pod(handle)` | Container destroyed, pane stays | Session, tab, pane (shows exit) |
| Pause agent | `pause_pane_agent(pane_id)` → `core.pod_manager.pause_pod(handle)` | Agent frozen, pane shows "paused" | Everything stays, resumable |

---

## Integration with Other Quanta

- **Core**: Navi calls Core's PTY, Render, and Pod APIs. Core streams PTY output and exit events back.
- **MOTOKO**: Navi emits session/agent lifecycle events to the bus. MOTOKO calls `pause_session`/`kill_session` on CRITICAL events.
- **MAGGI**: MAGGI calls Navi's API for session/tab/pane management. MAGGI queries session state for context.
- **THE WIRED**: External requests for session management route through the command router to Navi's API.
