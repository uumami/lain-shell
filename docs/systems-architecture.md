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
│  │ Command Router · Isolation Manager · Event Bus      │ │
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
│  Agent Isolation                        PER PANE        │
│  Level 0-3: naked/sandboxed/contained/air-gapped       │
│  Host proxy alongside for Docker/GPU/tool access        │
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

### Event delivery model — two tiers

The event system has two tiers with different delivery guarantees:

**Tier 1: Mandatory events** — security, audit, lifecycle (session/pane/isolation created/destroyed, security escalations, config changes). Emitted via the `AuditSink` trait, which routes to MOTOKO — the single audit log writer. MOTOKO serializes writes, maintains the hash chain, and fsyncs. The audit write must complete before the mutating operation proceeds. If the write fails, the operation fails (fail-closed).

**Tier 2: Observable events** — debug, informational, profile-filtered. Delivered via `tokio::broadcast`. Best-effort. A lagging receiver drops events and a warning is logged. Consumers must tolerate gaps.

**Bridge:** Every mandatory event is also sent to the `tokio::broadcast` bus for convenience of non-security consumers (MAGGI, UI, plugins). The broadcast copy is not the source of truth — consumers that require completeness must read the audit log.

**AuditSink trait** (defined in `lain-types/src/traits/audit.rs`):

```rust
#[async_trait]
trait AuditSink: Send + Sync {
    async fn emit(&self, event: MandatoryEvent) -> Result<()>;
}
```

In dev mode: direct in-process call to MOTOKO's writer (serialized via mpsc channel). In production: RPC to MOTOKO's process over Unix socket. Either way, MOTOKO is the single writer — no concurrent file appends, no hash chain forks.

### Event emission discipline

Every trait method that mutates state emits events **from within its implementation**, not from the caller. **Write-before-mutate ordering:** audit the intent first, then mutate. If audit fails, nothing happens. If mutation fails after audit, the log shows an unmatched intent (useful diagnostic, not a bug).

```rust
impl NaviApi for Navi {
    async fn create_session(&self, params: SessionParams) -> Result<SessionId> {
        // 1. Audit the intent — if this fails, nothing happens (fail-closed)
        self.audit_sink.emit(MandatoryEvent::SessionCreate {
            params: params.clone(),
        }).await?;

        // 2. Mutate — audit already recorded the intent
        let id = self.session_manager.create(params.clone()).await?;

        // 3. Observable: best-effort, for UI/debug/MAGGI
        let _ = self.bus.send(Event::SessionCreated { id, params });

        Ok(id)
    }
}
```

Callers cannot forget to emit events because they don't do it. The quantum itself guarantees event emission. Mandatory events use `AuditSink` (durable, single-writer). Observable events use `tokio::broadcast` (lossy, best-effort).

---

## Communication Map

```
FROM          TO            PATTERN              WHAT
─────         ──            ───────              ────
Navi      →   Core          direct call          create/resize/destroy PTY
Navi      →   Core          direct call          create/destroy isolation (via Isolation Manager)
Navi      →   Core          direct call          allocate render surface
Core      →   Navi          event bus            PTY exit events
Core      →   MOTOKO        dedicated channel    PTY output bytes (scanning)
Core      →   MOTOKO        dedicated channel    host proxy audit stream
Core      →   MOTOKO        event bus            command block events, config changes, isolation events
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

## Isolation Manager

A component within Core that manages agent isolation at four configurable levels. Previously called "Pod Manager" — renamed because isolation is a spectrum, not just containers. See ADR-009.

### Isolation Levels

| Level | Name | Mechanism | Use case |
|---|---|---|---|
| 0 | Naked | No isolation. MOTOKO PTY scanning only. | Trusted agents, GPU workloads, explicit opt-out |
| 1 | Sandboxed | seccomp-BPF + PID/mount namespace. Host tools visible read-only. | **Default.** Daily coding, most users. |
| 2 | Contained | Rootless Podman container. Own filesystem. | Untrusted agents, sensitive repos, compliance |
| 3 | Air-gapped | Level 2 + all network dropped. | Maximum restriction, sensitive environments |

Level 1 is the default because it provides strong security (blocks credential theft, lateral movement, privilege escalation) with near-zero configuration effort. Agent-generated Dockerfiles make Level 2 a ~5 minute setup.

### Host Proxy

For Levels 1-3, a host proxy process runs alongside the sandbox/container:

```
Agent (inside sandbox)              Host Proxy (on host)
┌──────────────────────┐           ┌──────────────────────┐
│  $ docker compose up │           │  Receives command     │
│  → "docker" is a shim│           │  Checks allowlist     │
│  → forwards to proxy │──socket──▶│  Translates paths     │
│  → receives output   │◀──socket──│  Executes on host     │
│  → prints normally   │           │  Streams output back  │
└──────────────────────┘           └──────────────────────┘

Allowlist defined in .lain/permissions.toml
MOTOKO audits every proxied operation
```

Shim binaries (docker, nvidia-smi, kubectl, etc.) are placed in the agent's PATH. The agent runs commands normally — shims transparently forward to the host proxy. The agent doesn't know the difference.

### Per-pane, highest wins

```
Resolution: max(agent_default, repo_policy, directory_policy)

agent_default    = Level 1  (claude-code)
repo_policy      = Level 2  (.lain/permissions.toml in this repo)
directory_policy = Level 3  (~/.config/lain-shell/directory-policies.toml)

effective_level  = Level 3  (highest restriction wins)
```

Users can lower per-pane with explicit confirmation and audit logging.

### Agent pane creation sequence

```
User: "open a new pane with Claude Code"
      │
      ▼
MAGGI translates intent to commands
      │
      ├──1──▶ navi.create_pane(session, tab, type=agent)
      │       Navi creates pane structure (layout, metadata)
      │
      ├──2──▶ core.isolation_manager.create_isolation(agent_config)
      │       Isolation Manager:
      │       ├── Resolves effective level (agent + repo + directory)
      │       ├── Queries MOTOKO for security profile
      │       ├── Level 0: nothing (just PTY scanning)
      │       ├── Level 1: creates namespace, applies seccomp, mounts
      │       ├── Level 2: creates container, builds/pulls image
      │       ├── Level 3: Level 2 + drops network
      │       ├── Spawns host proxy if Level 1-3
      │       └── Returns IsolationHandle
      │
      ├──3──▶ core.pty_manager.create_pty(in_isolation)
      │       PTY spawned inside the namespace/container/host
      │
      └──4──▶ Events emitted:
              PaneCreated, IsolationCreated, AgentSpawned
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
| Kill agent isolation | Namespace/container destroyed. Agent dies. Pane stays (shows exit). | Session, tab, pane structure. |
| Pause agent | Agent frozen (SIGSTOP via IsolationHandle). Pane shows "paused." Resumable. | Everything stays. |

Kill isolation ≠ kill pane. MOTOKO CRITICAL pauses/kills the **agent's isolation environment**, not the pane. The user sees what happened and can decide.

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

### Crash semantics — what survives

Core owns PTY file descriptors, VTE state (`alacritty_terminal::Term`), and scrollback buffers. Navi owns session topology (sessions, tabs, panes) and persists it to TOML. This separation determines what survives each crash scenario:

| Quantum dies | PTYs | VTE state | Session topology | Recovery |
|---|---|---|---|---|
| Core | Die (OS reclaims fds) | Lost | Navi has it (useless without PTYs) | Supervisor restarts Core. Navi re-requests PTYs for restorable panes. New processes, scrollback lost. |
| Navi | Survive in Core | Survives in Core | Lost (recover from persisted TOML) | Supervisor restarts Navi. Navi reads TOML, queries Core for live PTYs via `CorePtyApi`, reconciles: matching PTYs restored, dead PTYs show "[exited]". |
| MOTOKO | Survive | Survives | Survives | Tier 1 kernel enforcement (seccomp, namespaces) survives. Agent sessions paused per restart rules. Supervisor restarts MOTOKO. |
| MAGGI | Survive | Survives | Survives | Agent UX unavailable. Terminal and sessions work normally. |

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
Mandatory events: AuditSink → MOTOKO writer task (in-process), fsync by MOTOKO
Observable events: tokio broadcast channel (best-effort)
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
Mandatory events: AuditSink → MOTOKO writer (RPC over Unix socket), fsync by MOTOKO
Observable events: pub/sub over Unix socket (best-effort)
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

## .lain/ Config Mirror Pattern

The `.lain/` directory in the project repo is **declarative source**. The active configuration lives in a safe directory outside the agent's reach. See ADR-010.

```
IN THE REPO (version controlled, agent-writable):
  ~/project/.lain/
  ├── config.toml           ← agent CAN modify (just project files)
  ├── permissions.toml      ← agent CAN modify (just project files)
  ├── policies.toml         ← agent CAN modify (just project files)
  └── containers/Dockerfile ← agent CAN modify (helpful)

  Modifying these has NO EFFECT on active isolation.
  They are suggestions until a human syncs them.

THE SAFE COPY (what lain-shell actually reads):
  ~/.local/state/lain-shell/workspaces/<project-hash>/
  ├── config.toml           ← copied from .lain/ via `lain config sync`
  ├── permissions.toml      ← copied from .lain/ via `lain config sync`
  ├── policies.toml         ← copied from .lain/ via `lain config sync`
  └── containers/Dockerfile ← copied from .lain/ via `lain config sync`

  MOTOKO compiles rules from HERE.
  Isolation Manager reads from HERE.
  The agent cannot reach this path.
```

`lain config sync` shows a diff, requires user confirmation for security-relevant changes. MAGGI can prompt for sync when it detects repo `.lain/` differs from active config.

**The agent cannot configure its own cage.**

---

## System Invariants

These must hold at all times, regardless of deployment mode:

1. **No agent process exists without MOTOKO monitoring.** MOTOKO must be healthy before any agent isolation is created.
2. **Every PTY is owned by exactly one Navi pane.** No orphan PTYs.
3. **Config changes are atomic.** Partial writes are never visible.
4. **Tier 1 enforcement survives MOTOKO crashes.** seccomp and namespaces are kernel-enforced.
5. **MOTOKO's dedicated channel is structurally isolated.** No other quantum can observe or interfere with the PTY→MOTOKO stream.
6. **Policy authoring and enforcement are separated.** MOTOKO never writes policy files.
7. **Cross-session MAGGI access is explicit.** No automatic context sharing between sessions.
8. **Mandatory events are durably written to the audit log before the mutating operation completes.** The audit log is the source of truth for security and lifecycle events. The observable event bus (`tokio::broadcast`) is best-effort and may drop events for lagging receivers.
9. **Active security config is outside agent reach.** The `.lain/` mirror pattern ensures agents cannot modify their own isolation config. See ADR-010.
10. **Isolation level resolution is max().** When multiple policies apply (agent default, repo policy, directory policy), the highest restriction wins. Lowering requires explicit user confirmation and audit logging.

---

## Authority and Override Ladder

When customization meets security, this precedence applies:

```
runtime invariant > team policy > workspace policy > user preference
```

| Setting | Who wins | Why |
|---|---|---|
| Disable MOTOKO Tier 1 entirely | Cannot. Runtime invariant. | Kernel enforcement (seccomp, namespaces) survives any config change. |
| Set isolation to Level 0 for all agents | Team policy can forbid. User preference alone can lower to L0 with confirmation + audit. | Team policy wins over user preference. |
| Lower isolation from L2 to L1 | User can, with confirmation + audit log. Team policy can block. | Lowering is allowed but audited. |
| Disable audit logging | Cannot. Runtime invariant. | Tamper-evidence is structural. |
| Change keybinding for pane split | User preference. | No security implication — fully customizable. |
| Set custom MAGGI system prompt | User preference. | No security implication. |
| Override workspace isolation minimum | Cannot without user confirmation. Team policy is ceiling. | `max()` resolution from invariant #10. |

**What is NOT an invariant** (freely customizable): visual layer (themes, fonts, colors, transparency), keybindings, session defaults, MAGGI model/prompt/cost limits, observable event subscriptions, layout presets.

This resolves the tension between philosophy principle #7 ("every ceiling should be removable") and the system invariants: ceilings imposed by user preference are removable. Ceilings imposed by runtime invariants or team policy are not — they are structural, not configurable.

---

## Lifecycle Diagrams

### Agent session creation

```
User: "open Claude Code in a new pane"
│
├─1─▶ MAGGI: parse intent
├─2─▶ MAGGI → Navi: create_pane(session, tab, type=agent, agent=claude-code)
├─3─▶ Navi → Core.isolation_manager: create_isolation(agent_config)
│     Isolation Manager resolves effective level:
│       max(claude-code default: L1, repo policy: L1) = Level 1
├─4─▶ Core.isolation_manager: creates namespace, applies seccomp,
│     constructs mount view, spawns host proxy, returns IsolationHandle
├─5─▶ Core.pty_manager: create_pty(inside isolation)
├─6─▶ Navi: registers pane with IsolationHandle, updates layout
├─7─▶ Events: mandatory (PaneCreated, IsolationCreated, AgentSpawned) → AuditSink; observable → bus
├─8─▶ MOTOKO: observes events, begins monitoring (adapts to isolation level)
├─9─▶ Core.renderer: allocates render surface for new pane
└─10─▶ Agent process starts, PTY output flows to renderer + MOTOKO
```

### Security event escalation

```
Agent attempts to access ~/.ssh/id_rsa
│
├─ Level 1 (mount namespace): ~/.ssh is not mounted → ENOENT
│  Agent sees "No such file or directory". Silent. Zero overhead.
│  This is the primary protection for path-level file access.
│
├─ If sensitive data appears in PTY output via other paths:
├─ Tier 1 (pattern matching): Aho-Corasick detects SSH key pattern in output
│  Event written to audit log (mandatory).
│
├─ Tier 2 (statistical): flags as ANOMALY or CRITICAL
│  ├─ ANOMALY: non-blocking indicator, queued for postmortem
│  └─ CRITICAL:
│     ├── MOTOKO → Navi: pause_session(session_id)
│     ├── Navi pauses the agent isolation (SIGSTOP via IsolationHandle)
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
├── Client's AttachHandle removed
├── Navi server keeps running (PTYs alive, agents running)
├── MOTOKO keeps monitoring
├── MAGGI stays available (but no user input from this client)
├── Session state unchanged
├── Other attached clients: unaffected
└── If last client detaches: session persists, no viewers

Reattach (user opens new terminal, runs `lain attach work`):
├── New renderer connects to Navi server via Unix socket
├── Navi creates new AttachHandle with view state (active tab, focused pane)
├── Navi sends current layout + pane state
├── Renderer reconstructs the view
├── PTY output flows to this client
├── Session continues exactly where it was
└── Other attached clients: unaffected

Multi-client (two windows on same session):
├── Client A: local terminal, 200×50, viewing Tab 1
├── Client B: remote via THE WIRED, 80×24, viewing Tab 2
├── Each has its own AttachHandle with independent view state
├── Input from A → A's focused pane. Input from B → B's focused pane.
├── PTY output broadcast to all clients viewing that tab
└── PTY size: smallest client viewing each tab (recalculated on attach/detach/switch)
```
