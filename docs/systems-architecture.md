# lain-shell — Systems Architecture

> *Overall system map: boundaries, interfaces, data flow, invariants.*
> *For technology choices and implementation details, see `systems-design.md`.*
> *For values and principles, see `philosophy.md`.*

---

## The Five Quanta — Topology

All quanta are designed as **separate processes** communicating over IPC. During development, they may run in-process behind the same trait interfaces. The calling code does not know or care whether the implementation is in-process or remote.

```
┌─────────────────────────────────────────────────────────┐
│  lain-shell (Core + Supervisor)         MAIN PROCESS    │
│  ┌────────────────────────────────────────────────────┐ │
│  │ Supervisor                                         │ │
│  │ Start order: Core → MOTOKO → Navi → MAGGI → WIRED │ │
│  │ Health: heartbeat monitoring                       │ │
│  │ Restart rules: Erlang-inspired (see below)         │ │
│  └────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────┐ │
│  │ Core                                               │ │
│  │ PTY Manager · Renderer · Config · CLI              │ │
│  │ Command Router · Pod Manager · Event Bus           │ │
│  └────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  MOTOKO                                 SEPARATE PROCESS│
│  Per-session scanners · cross-session correlator        │
│  Policy compiler · audit log · internal panic isolation │
│                                                         │
│  Navi                                   SEPARATE PROCESS│
│  Session → Tab → Pane hierarchy                         │
│  Layout engine · attach/detach · persistence            │
│                                                         │
│  MAGGI                                  SEPARATE PROCESS│
│  Per-session contexts · shared knowledge store          │
│  Model backend · tool dispatch                          │
│                                                         │
│  WIRED                                  SEPARATE PROCESS│
│  MCP server · gRPC server · Unix socket listener        │
│                                                         │
│  Agent Pods                             ONE EACH        │
│  Each agent pane runs in its own container/namespace    │
│                                                         │
│  MOTOKO Tier 3                          ON DEMAND       │
│  Isolated reasoning pod, spawned per analysis           │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Why Core is special

Core cannot be a separate process because it owns the window and renderer. The `lain-shell` binary IS Core plus supervisor logic. Other quanta are pluggable: in-process for development, separate processes for production.

---

## Communication Model — Hybrid

Three communication patterns, each used where it fits:

### 1. Direct calls (request/response)

Quanta call each other through **async trait interfaces** with owned, serializable types. In-process: direct function call. Separate process: gRPC over Unix socket. The caller doesn't know which.

```rust
#[async_trait]
trait NaviApi: Send + Sync {
    async fn create_session(&self, params: SessionParams) -> Result<SessionId>;
    async fn list_sessions(&self) -> Result<Vec<SessionInfo>>;
    async fn pause_session(&self, id: SessionId) -> Result<()>;
    // All types: #[derive(Serialize, Deserialize, Clone)]
}
```

Used for: commands that need a response. "Create a PTY." "List sessions." "Read config."

### 2. Event bus (observation, pub/sub)

A central typed event bus. Quanta publish events when significant actions occur. Subscribers receive events they're interested in. Events are filtered by configurable profiles.

Used for: notifications, audit, coordination. "Session was created." "Config changed." "Agent spawned."

### 3. Dedicated channels (high-frequency streams)

Structurally isolated pipes between specific quanta. Not routed through the event bus.

Used for: PTY output → MOTOKO (continuous byte stream for pattern matching). High-frequency, security-critical, must not be observable or interruptible by other quanta.

### Why three patterns

| Pattern | Strength | Weakness |
|---|---|---|
| Direct call | Natural for request/response. Type-safe. Fast. | No natural interception for observation. |
| Event bus | Natural for observation. Decoupled. Auditable. | Awkward for request/response (correlation IDs). |
| Dedicated channel | Isolated. Cannot be tampered with or congested by other traffic. | Special-cased. Only for specific high-frequency flows. |

The hybrid avoids forcing everything into one pattern. Requests are direct calls. Observations are bus events. Security-critical streams are dedicated channels.

### Event emission discipline

Every trait method that mutates state emits an event to the bus **from within its implementation**, not from the caller:

```rust
impl NaviApi for Navi {
    async fn create_session(&self, params: SessionParams) -> Result<SessionId> {
        let id = self.session_manager.create(params.clone()).await?;
        self.bus.emit(Event::SessionCreated { id, params }); // always emitted
        Ok(id)
    }
}
```

Callers cannot forget to emit events because they don't do it. The quantum itself guarantees event emission.

---

## Communication Map

```
FROM          TO            PATTERN              WHAT
─────         ──            ───────              ────
Navi      →   Core          direct call          create/resize/destroy PTY
Navi      →   Core          direct call          allocate render surface
Core      →   Navi          event bus            PTY exit events
Core      →   MOTOKO        dedicated channel    PTY output bytes (scanning)
Core      →   MOTOKO        event bus            command block events, config changes
Navi      →   MOTOKO        event bus            session/agent lifecycle events
MOTOKO    →   Navi          direct call          pause/kill session (CRITICAL, rare)
MAGGI     →   Core          direct call          config read/write, CLI commands
MAGGI     →   Navi          direct call          session/tab/pane management
MAGGI     →   Navi          direct call          query session state
MOTOKO    →   MAGGI         event bus            security events for explanation
WIRED     →   Router        direct call          all external requests
Router    →   *             direct call          dispatched to appropriate quantum
ALL       →   event bus     event bus            audit-critical events (mandatory)
```

---

## Dedicated MOTOKO Channel

The PTY output stream from Core to MOTOKO is **structurally isolated** from the event bus. This is a security decision:

- **No other quantum can subscribe** to raw PTY output (prevents data leaks)
- **Bus congestion cannot delay** MOTOKO's observation (prevents evasion through flooding)
- **The channel cannot be intercepted** by a compromised quantum
- When processes are separate: dedicated Unix domain socket at `/run/user/$UID/lain/motoko.pipe`

```
Core                              MOTOKO
┌──────────┐                     ┌──────────┐
│PTY output│══dedicated pipe════▶│Aho-Cora- │
│bytes     │  (isolated,         │sick scan  │
│          │   mandatory,        │behavioral │
│          │   untamperable)     │analysis   │
└──────────┘                     └──────────┘
```

---

## Event Bus — Profile-Filtered

Events are emitted at full granularity. A profile filter determines which events flow to which subscribers.

```
┌─────────────────────────────────────────────────┐
│                Event Bus                         │
│                                                  │
│  ┌────────────────────────────────────────────┐  │
│  │ MANDATORY (no profile can disable):        │  │
│  │ - Security blocks (Tier 1)                 │  │
│  │ - CRITICAL events (Tier 2)                 │  │
│  │ - Session lifecycle (create, destroy)       │  │
│  │ - Agent lifecycle (spawn, kill, pause)      │  │
│  │ - MOTOKO PTY scanning (dedicated channel)  │  │
│  │ - Audit-critical events                    │  │
│  └────────────────────────────────────────────┘  │
│                                                  │
│  ┌────────────────────────────────────────────┐  │
│  │ PROFILE-CONTROLLED:                        │  │
│  │ - Command block events                     │  │
│  │ - Config reads                             │  │
│  │ - Agent tool calls                         │  │
│  │ - Performance metrics                      │  │
│  │ - Debug tracing                            │  │
│  │ - Pane output summaries                    │  │
│  └────────────────────────────────────────────┘  │
│                                                  │
│  Profiles:                                       │
│  - minimal:   mandatory events only              │
│  - standard:  mandatory + lifecycle + commands   │
│  - verbose:   everything                         │
│  - debug:     everything + internal state        │
│  - custom:    user-defined                       │
└─────────────────────────────────────────────────┘
```

---

## Command Router

All external requests (CLI, MCP, gRPC, Unix socket) enter through a single **command router** in Core. The router handles auth, dispatches to the appropriate quantum, and emits audit events.

```
┌─────────────────────────────────────────────────┐
│                Command Router (Core)             │
│                                                  │
│  Inputs:                                         │
│  ├── lain CLI (direct)                          │
│  ├── MCP (via WIRED)                            │
│  ├── gRPC (via WIRED)                           │
│  └── Unix socket (via WIRED)                    │
│                                                  │
│  Pipeline:                                       │
│  1. Parse → LainCommand                         │
│  2. Auth check (does this caller have permission?)│
│  3. Dispatch to target quantum                  │
│  4. Emit audit event to bus                     │
│  5. Return response                              │
│                                                  │
│  LainCommand {                                   │
│    action: Action,        // SessionCreate, etc. │
│    target: Option<Target>,// specific session    │
│    params: Params,        // action-specific     │
│    auth: AuthContext,     // who, what permissions│
│  }                                               │
└─────────────────────────────────────────────────┘
```

WIRED translates external protocols to `LainCommand`. The router dispatches. No quantum other than the router needs to handle auth.

---

## Pod Manager

A component within Core that manages containers and namespaces for agent panes. Navi requests pods; Core's pod manager creates them; MOTOKO provides the security context.

```
Agent pane creation sequence:

User: "open a new pane with Claude Code"
      │
      ▼
MAGGI translates intent to commands
      │
      ├──1──▶ navi.create_pane(session, tab, type=agent)
      │       Navi creates pane structure (layout, metadata)
      │
      ├──2──▶ core.pod_manager.create_pod(agent_config)
      │       Pod manager:
      │       ├── Creates container/namespace
      │       ├── Queries MOTOKO for seccomp profile
      │       ├── Applies seccomp, network policy, mounts
      │       └── Returns pod handle
      │
      ├──3──▶ core.pty_manager.create_pty(in_pod)
      │       PTY spawned inside the pod
      │
      └──4──▶ Events emitted:
              PaneCreated, PodCreated, AgentSpawned
              MOTOKO observes, begins monitoring
```

---

## Navi Session Hierarchy

```
Navi Server (one process)
│
├── Session "work" (persistent, attach/detach)
│   ├── Tab 1 "editor" (full-screen view)
│   │   ├── Pane A [shell: nvim]
│   │   └── Pane B [shell: cargo watch]
│   └── Tab 2 "agents" (switch tabs = switch full view)
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

Hierarchy:
  Session = persistent workspace, survives detach
  Tab     = full-screen view within a session
  Pane    = split within a tab, contains PTY + optional agent
```

### Kill granularity

| Target | What happens | What survives |
|---|---|---|
| Kill pane | PTY dies, pane removed. If agent: container destroyed. | Session, tab, other panes. |
| Kill tab | All panes in tab die. | Session, other tabs. |
| Kill session | All tabs and panes die. | Other sessions. |
| Kill agent pod | Container destroyed. Agent dies. Pane stays (shows exit). | Session, tab, pane structure. |
| Pause agent | Container paused (SIGSTOP). Agent frozen. Pane shows "paused." Resumable. | Everything stays. |

Kill pod ≠ kill pane. MOTOKO CRITICAL pauses/kills the **agent pod**, not the pane. The user sees what happened and can decide.

---

## MOTOKO Topology — Single Process, Internal Session Isolation

One MOTOKO process with per-session scanners internally. Cross-session correlation is a first-class capability.

```
MOTOKO (single process)
├── Per-session scanners (panic-isolated via catch_unwind)
│   ├── Session "work": scanner + baseline
│   ├── Session "ops": scanner + baseline
│   └── Session "background": scanner + baseline
│
├── Cross-session correlator
│   (detects patterns spanning multiple agents)
│
├── Policy compiler (once, shared across sessions)
├── Audit log writer (single, ordered)
└── Internal panic isolation:
    Session A analysis panics → caught, logged, scanner restarted
    Other sessions: unaffected
```

### Why single process (not per-session)

1. Cross-session correlation requires seeing all sessions
2. Tier 1 (seccomp, namespaces) is kernel-enforced — survives MOTOKO crash
3. Internal panic isolation limits blast radius within the process
4. One policy compilation instead of N
5. One ordered audit log instead of N that need merging

---

## MAGGI Context Architecture — Pull, Not Push

MAGGI maintains per-session conversation contexts with a shared knowledge store. Context is pulled on demand, not pushed automatically.

```
MAGGI (single process)
│
├── Shared Knowledge Store
│   ├── lancedb: platform documentation (semantic search)
│   ├── tantivy: platform documentation (keyword search)
│   ├── User preferences and cross-session facts
│   └── Config schemas, available commands
│
├── Session "work" context
│   ├── Conversation history (MAGGI ↔ user)
│   ├── Session state (tabs, panes, agents)
│   ├── Session-scoped scratchpad
│   └── Pane contexts (per-pane working dir, recent blocks)
│
├── Session "ops" context
│   ├── (same structure, fully isolated)
│   └── ...
│
└── Cross-session access:
    Explicit tool calls only.
    maggi.query_session("ops") → pulls summary
    maggi.search_memory(scope=global) → searches shared store
    Never automatic. Always audited.
```

### Memory scoping (four-dimensional)

Every memory record is scoped:

| Dimension | Values | Purpose |
|---|---|---|
| `user_id` | "uumami" | Who owns this memory |
| `session_id` | "work", "ops", etc. | Which session context |
| `agent_id` | "maggi", "claude-code", etc. | Which agent created it |
| `scope` | pane, session, global | Visibility level |

- **Pane scope**: only MAGGI in that specific pane sees it
- **Session scope**: only MAGGI within that session sees it
- **Global scope**: any MAGGI context can pull from it (explicit query)

### How MAGGI attaches to panes

The user can talk to MAGGI from any pane. MAGGI sees:
- The current pane's context (working dir, recent command blocks)
- The current session's conversation history
- The ability to **pull** from the shared store or other sessions via tools

MAGGI does NOT automatically see other panes, other sessions, or other agents' conversations. It reaches for what it needs through explicit, audited tool calls.

---

## Policy Flow — Separation of Powers

```
POLICY AUTHORING              POLICY ENFORCEMENT
(who writes rules)            (who applies rules)

User ──────────┐
               │
MAGGI ─────────┼──▶ .lain/policies.toml      MOTOKO
(helps author, │    .lain/permissions.toml    (reads, compiles,
 explains,     │                               enforces)
 suggests)     │                    │
               │              reads │
               │                    ▼
POLICY QUERYING│              MOTOKO enforces
               │              structurally
User ──────────┼──▶ lain motoko status    (read-only)
User ──────────┼──▶ lain audit verify     (read-only)
User ──────────┼──▶ lain postmortem       (triggers Tier 3)
               │
MAGGI ─────────┘──▶ reads MOTOKO events, translates to
                    plain language for the user
```

- **MOTOKO never writes policy.** It only reads and enforces. Separation of powers.
- **The user is the authority.** Either directly (text editor) or through MAGGI (conversational authoring with confirmation).
- **MAGGI is the pen.** It authors policy on the user's behalf, shows the rule, waits for confirmation.
- **MOTOKO is the enforcer.** It compiles rules and applies them structurally.
- **MOTOKO is queryable.** `lain motoko status`, `lain audit verify` are read-only.

---

## Supervisor — Process Lifecycle

The supervisor is part of the main `lain-shell` binary (alongside Core). It manages the lifecycle of other quanta.

### Start order

```
1. Core        (immediate — it's the main process)
2. MOTOKO      (before any agent can run)
3. Navi        (once MOTOKO is healthy)
4. MAGGI       (when model configured)
5. WIRED       (when external access configured)
```

### Restart rules

| Quantum dies | Action | Rationale |
|---|---|---|
| MOTOKO | Pause all agent sessions via Navi. Attempt restart. If 3 failures: kill agents. Prominent alert. | "If MOTOKO dies, agent sessions die." Tier 1 kernel enforcement still active. |
| Navi | Attempt restart. Sessions are persisted, restore after restart. Single-pane fallback while restarting. | Sessions survive restarts by design. |
| MAGGI | Show "MAGGI unavailable." No restart pressure. Retry on next user request. | MAGGI is optional. |
| WIRED | Show indicator. Attempt restart. | WIRED is optional. |
| Core | Everything dies. User re-launches or systemd restarts. | Core is the process. |

### Health monitoring

Each quantum exposes a health endpoint (gRPC health check or heartbeat over socket). The supervisor polls at a configurable interval (default: 1 second). Three missed heartbeats = quantum considered dead.

---

## Deployment Modes

### Development (single process)

```
lain-shell binary
├── Core (in-process)
├── MOTOKO: Arc<dyn MotokoApi> = InProcessImpl
├── Navi: Arc<dyn NaviApi> = InProcessImpl
├── MAGGI: Arc<dyn MaggiApi> = InProcessImpl
└── WIRED: Arc<dyn WiredApi> = InProcessImpl

All trait calls: direct function calls
Event bus: tokio broadcast channel
MOTOKO channel: tokio mpsc
Single binary. Single process.
```

### Production (separate processes)

```
lain-shell binary (Core + supervisor)
├── Core (in-process)
├── MOTOKO: Arc<dyn MotokoApi> = GrpcClient
├── Navi: Arc<dyn NaviApi> = GrpcClient
├── MAGGI: Arc<dyn MaggiApi> = GrpcClient
└── WIRED: Arc<dyn WiredApi> = GrpcClient

lain-motoko binary (separate process)
lain-navi binary (separate process)
lain-maggi binary (separate process)
lain-wired binary (separate process)

Trait calls: gRPC over Unix sockets
Event bus: pub/sub over Unix socket
MOTOKO channel: dedicated Unix socket
```

### Configurable per-quantum

```toml
# ~/.config/lain-shell/system.toml
[deployment]
mode = "development"  # or "production"

[deployment.overrides]
motoko = "separate"   # run MOTOKO separately even in dev
```

---

## Interface Design Rules

1. All trait methods are `async`
2. All parameter and return types: `#[derive(Serialize, Deserialize, Clone)]`
3. No borrowed references across quantum boundaries (owned types only)
4. Error types are serializable and meaningful across processes
5. Streams use a streaming trait that maps to both tokio channels and gRPC streams
6. Every trait method that mutates state emits an event from within its implementation

---

## System Invariants

These must hold at all times, regardless of deployment mode:

1. **No agent process exists without MOTOKO monitoring.** MOTOKO must be healthy before any agent pod is created.
2. **Every PTY is owned by exactly one Navi pane.** No orphan PTYs.
3. **Config changes are atomic.** Partial writes are never visible.
4. **Tier 1 enforcement survives MOTOKO crashes.** seccomp and namespaces are kernel-enforced.
5. **MOTOKO's dedicated channel is structurally isolated.** No other quantum can observe or interfere with the PTY→MOTOKO stream.
6. **Policy authoring and enforcement are separated.** MOTOKO never writes policy files.
7. **Cross-session MAGGI access is explicit.** No automatic context sharing between sessions.
8. **The event bus cannot drop mandatory events.** Security and lifecycle events are always delivered.

---

## Lifecycle Diagrams

### Agent session creation

```
User: "open Claude Code in a new pane"
│
├─1─▶ MAGGI: parse intent
├─2─▶ MAGGI → Navi: create_pane(session, tab, type=agent, agent=claude-code)
├─3─▶ Navi → Core.pod_manager: create_pod(seccomp=motoko.profile, net=restricted)
├─4─▶ Core.pod_manager: creates container, applies seccomp, network policy
├─5─▶ Core.pty_manager: create_pty(inside pod)
├─6─▶ Navi: registers pane, updates layout
├─7─▶ Events: PaneCreated, PodCreated, AgentSpawned → bus
├─8─▶ MOTOKO: observes events, begins session monitoring
├─9─▶ Core.renderer: allocates render surface for new pane
└─10─▶ Agent process starts, PTY output flows to renderer + MOTOKO
```

### Security event escalation

```
Agent attempts to read ~/.ssh/id_rsa
│
├─ Tier 1 (kernel): seccomp blocks the read syscall → SILENT
│  Event logged to audit. Agent sees EACCES. Session continues.
│
├─ OR if not caught by seccomp:
├─ Tier 1 (pattern matching): Aho-Corasick detects SSH key pattern in output
│  Event emitted to bus.
│
├─ Tier 2 (statistical): flags as ANOMALY or CRITICAL
│  ├─ ANOMALY: non-blocking indicator, queued for postmortem
│  └─ CRITICAL:
│     ├── MOTOKO → Navi: pause_session(session_id)
│     ├── Navi pauses the agent pod (SIGSTOP)
│     ├── Pane shows MOTOKO alert with context
│     ├── MOTOKO → MAGGI (via bus): security event for explanation
│     ├── MAGGI translates to plain language
│     └── User sees: what happened, choices (resume/kill/details)
│
├─ Tier 3 (triggered if CRITICAL is ambiguous):
│  ├── Spawn isolated reasoning pod
│  ├── Pass summarized event window (no raw logs)
│  ├── Pod analyzes, returns structured verdict
│  └── Verdict presented to user via MAGGI
```

### Session attach/detach

```
Detach (user closes terminal window):
├── Renderer disconnects
├── Navi server keeps running (PTYs alive, agents running)
├── MOTOKO keeps monitoring
├── MAGGI stays available (but no user input)
└── Session state unchanged

Reattach (user opens new terminal, runs `lain attach work`):
├── New renderer connects to Navi server via Unix socket
├── Navi sends current layout + pane state
├── Renderer reconstructs the view
├── PTY output resumes flowing to renderer
├── Session continues exactly where it was
```
