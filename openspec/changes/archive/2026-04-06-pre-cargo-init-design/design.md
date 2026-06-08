## Context

lain-shell has 10 accepted ADRs and comprehensive architecture docs for all five quanta. The docs define trait interfaces, component boundaries, and data models. But five cross-cutting TODOs in `systems-design.md` remain unresolved, and two independent reviews identified correctness gaps: the event bus invariant is unenforceable with `tokio::broadcast`, PTY ownership on crash is ambiguous, and the Level 1 security diagram conflates seccomp with path-level file policy.

These gaps must be resolved *in the docs* before initializing the Cargo workspace, because they shape every crate's public API from day one. Retrofitting error types, event semantics, or ownership models after code exists is exponentially harder.

Current state of the five `systems-design.md` TODOs:
- Crate boundaries: "Likely: lain-core, lain-navi, lain-motoko, lain-maggi, lain-wired, lain-shell"
- Error handling: "Define error strategy"
- Logging/tracing: "Define log levels, structured fields"
- Testing strategy: "Define"
- Repository structure: "Define once crate boundaries are settled"

## Goals / Non-Goals

**Goals:**
- Resolve all five `systems-design.md` TODOs with concrete, implementable decisions
- Fix invariant #8 so it's enforceable by the chosen technology
- Clarify PTY/VTE state ownership so crash semantics are unambiguous
- Specify Level 1 mount namespace precisely enough to implement
- Establish the serialization discipline for all cross-crate API types
- Add authority/override ladder to resolve customization-vs-security tension
- Fix all hygiene issues (naming, numbering, stale references, legacy notices)

**Non-Goals:**
- Writing Rust code (this is design docs only)
- Full security threat model (standalone doc, separate change — too large for this scope)
- Host proxy argument-level policy (needs implementation experience first)
- Plugin API surface (correctly deferred to future)
- Full authority matrix mapping every actor to every permission (premature — add ladder now, full matrix after THE WIRED auth is implemented)
- Cost accounting architecture (real gap, but not a cargo-init blocker)
- First-run UX design (important, but doesn't block workspace setup)

## Decisions

### Decision 1: Cargo workspace with `lain-types` shared crate

**Choice:** Single `lain-types` crate with internal module structure.

```
lain-shell/
├── Cargo.toml              (workspace root)
├── crates/
│   ├── lain-types/         shared IDs, events, errors, command model
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ids.rs      SessionId, PaneId, TabId, PtyId, AttachId, etc.
│   │       ├── events.rs   Event, MandatoryEvent, EventMetadata
│   │       ├── command.rs  LainCommand, AuthContext, Params, LainResponse
│   │       └── error.rs    LainError hierarchy
│   ├── lain-core/          PTY, VTE, renderer, config, isolation, router, event bus
│   ├── lain-navi/          sessions, tabs, panes, layout, attach/detach, persistence
│   ├── lain-motoko/        security policy, audit log, scanners, rule engine
│   ├── lain-maggi/         operator agent, tools, RAG, model backend
│   └── lain-wired/         MCP/gRPC/Unix socket protocol translation
├── src/
│   └── main.rs             binary entry point, supervisor, composition
├── docs/
├── tests/                  cross-crate integration tests
└── .lain/                  example/default config
```

**Why `lain-types` over micro-crates (`lain-ids`, `lain-events`, etc.):**
- At this stage, the types are small and interdependent (events reference IDs, errors reference commands)
- Splitting now creates 4+ tiny crates with cross-dependencies between them
- Internal module structure (`ids.rs`, `events.rs`, etc.) makes future extraction cheap if `lain-types` grows too large
- Single shared crate = one version to coordinate, one compile unit

**Why not put shared types in `lain-core`:**
- Creates a dependency from every crate on `lain-core`, which is the heaviest crate (PTY, VTE, renderer, wgpu)
- `lain-wired` should not need to compile `wgpu` to reference a `SessionId`

**Dependency graph:**
```
lain-types ←── lain-core ←── lain-shell (binary)
     ↑              ↑              ↑
     ├── lain-navi ─┘              │
     ├── lain-motoko ──────────────┤
     ├── lain-maggi ───────────────┤
     └── lain-wired ───────────────┘

All quantum crates depend on lain-types.
lain-navi depends on lain-core (PTY, render, isolation APIs).
lain-shell composes all crates into the binary.
No circular dependencies.
```

**Alternatives considered:**
- *No shared crate, duplicate types:* Fast initial compile, but divergence and conversion boilerplate accumulate immediately. Rejected.
- *Multiple micro-crates (`lain-ids`, `lain-events`, `lain-error`):* Correct in theory, premature at this scale. Module structure inside `lain-types` gives the same extraction path with less overhead. Revisit if `lain-types` exceeds ~2000 lines.

### Decision 2: Error strategy — `thiserror` everywhere, `LainError` in `lain-types`

**Choice:** Every crate defines its own error enum using `thiserror`. Cross-crate boundaries use `LainError` from `lain-types` as the common currency.

```rust
// lain-types/src/error.rs
#[derive(Debug, thiserror::Error, serde::Serialize, serde::Deserialize)]
pub enum LainError {
    #[error("not found: {resource} {id}")]
    NotFound { resource: String, id: String },

    #[error("permission denied: {reason}")]
    PermissionDenied { reason: String },

    #[error("invalid config: {detail}")]
    InvalidConfig { detail: String },

    #[error("isolation error: {detail}")]
    IsolationError { detail: String },

    #[error("pty error: {detail}")]
    PtyError { detail: String },

    #[error("bus error: {detail}")]
    BusError { detail: String },

    #[error("internal: {message}")]
    Internal { message: String },
}
```

**Key rules:**
- All trait methods in public APIs return `Result<T, LainError>`
- `LainError` derives `serde::Serialize + serde::Deserialize` — it must round-trip through gRPC
- Internal crate errors (e.g., `wgpu::Error`, `std::io::Error`) are converted to `LainError` at the crate boundary via `impl From<InternalError> for LainError`
- No `anyhow` in library crates. `anyhow` is allowed only in `src/main.rs` (the binary) for top-level error reporting
- Errors carry enough context to be actionable without the stack trace (resource name, ID, reason)

**Why not `anyhow` everywhere:**
- `anyhow::Error` is not serializable — breaks gRPC transport
- `anyhow::Error` erases the type — callers can't match on error variants
- `anyhow` is great for applications but wrong for library boundaries

**Why structured `LainError` over string-based errors:**
- MAGGI needs to match on error types to provide useful explanations
- MOTOKO needs to distinguish security errors from operational errors
- THE WIRED needs to map errors to appropriate gRPC status codes

### Decision 3: Two-tier event delivery model

**Choice:** Mandatory events are synchronous audit log writes. Observable events use `tokio::broadcast`.

```
┌─────────────────────────────────────────────────────────────┐
│                      Event System                           │
│                                                             │
│  Tier 1: Mandatory Events                                   │
│  ─────────────────────                                      │
│  What: security, audit, lifecycle (session/pane/isolation)  │
│  Delivery: synchronous append to MOTOKO audit log           │
│  Mechanism: audit_log.write(event).await? — fsync'd         │
│  Failure mode: if write fails, mutating operation fails     │
│             or enters fail-closed state                     │
│  Consumer: MOTOKO tails the audit log                       │
│  Guarantee: durable, ordered, never dropped                 │
│                                                             │
│  Tier 2: Observable Events                                  │
│  ─────────────────────                                      │
│  What: debug, informational, profile-filtered               │
│  Delivery: best-effort via tokio::broadcast                 │
│  Mechanism: bus.send(event) — fire and forget               │
│  Failure mode: lagging receiver drops events, warning logged│
│  Consumers: MAGGI, plugins, debug tools, UI indicators      │
│  Guarantee: none — receivers must tolerate gaps              │
│                                                             │
│  Bridge: every mandatory event is ALSO sent to broadcast    │
│  (for convenience of non-security consumers). But the       │
│  broadcast copy is not the source of truth.                 │
└─────────────────────────────────────────────────────────────┘
```

**The pattern in trait implementations:**

```rust
impl NaviApi for Navi {
    async fn create_session(&self, params: SessionParams) -> Result<SessionId> {
        let id = self.session_manager.create(params.clone()).await?;

        // Mandatory: synchronous, durable, fail-closed
        self.audit.write(MandatoryEvent::SessionCreated {
            id, params: params.clone()
        }).await?;  // if this fails, we must roll back or fail

        // Observable: best-effort, for UI/debug/MAGGI
        let _ = self.bus.send(Event::SessionCreated { id, params });

        Ok(id)
    }
}
```

**Why this over a single bus:**
- `tokio::broadcast` drops messages for lagging receivers — documented behavior
- Making `tokio::broadcast` lossless requires unbounded buffers (OOM risk) or backpressure (blocks all producers)
- The audit log is the natural durable store — MOTOKO already writes one
- Separating the tiers makes the guarantee explicit: mandatory = durable + ordered, observable = best-effort

**Invariant #8 rewrite:**
> "Mandatory events are durably written to the audit log before the mutating operation completes. The audit log is the source of truth for security and lifecycle events. The observable event bus (`tokio::broadcast`) is best-effort and may drop events for lagging receivers."

**Alternatives considered:**
- *Acked delivery with backpressure:* Correct but complex. Requires every subscriber to ack, which means a slow subscriber blocks all producers. Rejected — too much coupling.
- *Persistent message queue (e.g., embedded NATS):* Overkill for single-machine IPC. The audit log file *is* the persistent queue. Rejected.
- *Single `tokio::mpsc` per subscriber:* Guarantees delivery but requires knowing all subscribers at compile time. Doesn't support dynamic plugin subscriptions. Rejected for the general bus, but the MOTOKO dedicated channel already uses this pattern correctly.

### Decision 4: Core owns PTY + VTE state + scrollback

**Choice:** Core owns the full PTY pipeline. Navi stores `PtyId`, queries cell grids via `CorePtyApi`.

```
Core owns:                           Navi owns:
─────────                            ──────────
PTY file descriptor                  Session topology (sessions, tabs, panes)
alacritty_terminal::Term             Pane metadata (type, agent info, working dir)
Scrollback ring buffer               PtyId per pane (not PtyHandle)
Raw byte stream → VTE parsing        Layout tree (splits, ratios)
Cell grid (current screen state)     Attach/detach (AttachHandle, view state)
MOTOKO dedicated channel feed        Persistence (serialize/deserialize topology)
```

**Updated `CorePtyApi`:**

```rust
trait CorePtyApi {
    // Lifecycle
    async fn create_pty(&self, params: PtyParams) -> Result<PtyId>;
    async fn destroy_pty(&self, id: PtyId) -> Result<()>;
    async fn resize_pty(&self, id: PtyId, size: TerminalSize) -> Result<()>;
    async fn write_pty(&self, id: PtyId, data: &[u8]) -> Result<()>;

    // State queries (renderer and Navi use these)
    async fn get_cells(&self, id: PtyId, region: CellRegion) -> Result<CellGrid>;
    async fn get_scrollback(&self, id: PtyId, lines: usize) -> Result<Vec<Row>>;
    async fn get_cursor(&self, id: PtyId) -> Result<CursorState>;

    // Subscriptions
    fn subscribe_pty_output(&self, id: PtyId) -> PtyOutputStream;  // raw bytes (for MOTOKO)
    fn subscribe_pty_exit(&self, id: PtyId) -> PtyExitReceiver;
    fn subscribe_cell_changes(&self, id: PtyId) -> CellChangeStream; // parsed diffs (for renderer)
}
```

**Crash semantics:**

| Quantum dies | PTYs | VTE state | Session topology | What user sees |
|---|---|---|---|---|
| Core | Die (fd closed) | Lost | Navi has it, useless without PTYs | Everything dies. Supervisor restarts Core → Navi reconciles. |
| Navi | Survive in Core | Survives in Core | Lost (must recover from persistence) | PTYs keep running. Navi restarts → reads persisted topology → re-queries Core for live PTYs. |
| MOTOKO | Survive | Survives | Survives | Monitoring stops. Tier 1 (kernel) survives. Supervisor restarts MOTOKO. |
| MAGGI | Survive | Survives | Survives | Agent UX unavailable. Terminal works. |

**Why Core owns VTE state (not Navi):**
- Core already owns the renderer, which needs the cell grid
- MOTOKO's dedicated channel receives raw PTY bytes from Core — Core is already in the byte path
- If Navi crashes, Core can keep rendering "last known state" until Navi recovers
- Avoids splitting the render pipeline (Core renders but Navi owns cell data = coordination nightmare)

**Why Navi stores `PtyId` not `PtyHandle`:**
- `PtyHandle` implies ownership — if Navi serializes it to TOML for persistence, what does restoring a `PtyHandle` mean?
- `PtyId` is just a reference. On Navi restart, it queries Core: "what PTYs are alive?" and reconciles against persisted topology
- `PtyId` is serializable (it's just a u64 or UUID). `PtyHandle` is not.

### Decision 5: Level 1 mount namespace specification

**Choice:** Path security comes from mount namespace construction. Seccomp handles syscall class restrictions only.

**Level 1 mount table:**

```
Path inside sandbox          Source                    Mode
──────────────────           ──────                    ────
/                            tmpfs                     (empty root)
/usr                         host /usr                 read-only
/bin                         host /bin                 read-only
/lib, /lib64                 host /lib, /lib64         read-only
/etc/resolv.conf             host resolv.conf          read-only
/etc/hosts                   host hosts                read-only
/etc/ssl                     host /etc/ssl             read-only
/tmp                         private tmpfs             read-write
/dev/{null,zero,urandom}     devtmpfs                  standard
/proc                        new procfs                PID-namespaced
/workspace                   project directory         read-write
/home/agent/.cargo           host ~/.cargo             read-only
/home/agent/.rustup          host ~/.rustup            read-only
/home/agent/.npm             host ~/.npm               read-only
/home/agent/.config/git      host git config           read-only
/run/lain/proxy.sock         host proxy socket         read-write

NOT MOUNTED (invisible):
~/.ssh, ~/.gnupg, ~/.aws, ~/.kube, ~/other-projects, /mnt/*, host /home/*
```

**Level 1 seccomp profile (syscall classes, not paths):**

```
BLOCKED:
  ptrace, process_vm_readv/writev     can't inspect other processes
  mount, umount2                       can't escape mount namespace
  setns, unshare                       can't create new namespaces
  bpf                                  can't load eBPF
  kexec_load, kexec_file_load         can't replace kernel
  init_module, finit_module            can't load kernel modules
  pivot_root, chroot                   can't change root
  personality                          can't change execution domain
  reboot                               can't reboot
  swapon, swapoff                      can't manipulate swap

ALLOWED (needed for coding):
  open, read, write, close, stat...    file I/O (mount namespace limits visibility)
  fork, execve, clone, wait4...        process creation (PID namespace limits scope)
  socket, connect, bind, sendto...     networking (iptables limits destinations)
  ioctl                                terminal ops
  mmap, mprotect, brk                  memory management
```

**Security escalation diagram fix:**

The current diagram says "seccomp blocks the read syscall" for `~/.ssh/id_rsa`. This is wrong. The corrected flow:

```
Agent attempts to access ~/.ssh/id_rsa
│
├─ Level 1 (mount namespace): ~/.ssh is not mounted → ENOENT
│  Agent sees "No such file or directory". No event needed.
│  This is the primary protection. Silent. Zero overhead.
│
├─ If somehow a sensitive pattern appears in PTY output anyway:
├─ Tier 1 (Aho-Corasick): detects SSH key pattern in output
│  Event emitted to bus.
│
├─ Tier 2 (statistical): flags as ANOMALY or CRITICAL
│  [... rest of escalation unchanged ...]
```

**What's configurable per-workspace:**
- Additional read-only mounts (e.g., mount `~/.pyenv` for Python projects)
- Additional blocked/allowed syscalls
- Network filtering rules (iptables)
- Host proxy allowlist

Configured in `.lain/permissions.toml`, read from the safe mirror (ADR-010).

### Decision 6: Transport discipline — serialize from day one

**Choice:** All types that cross crate boundaries must derive `serde::Serialize + serde::Deserialize`. No exceptions.

**Rules:**
1. Every type in `lain-types` derives `Serialize + Deserialize + Debug + Clone`
2. IDs are newtypes over `u64` or `uuid::Uuid` — always serializable
3. No raw file descriptors, channel senders, or OS handles in public APIs
4. `PtyId` replaces `PtyHandle` in all cross-boundary APIs. `PtyHandle` (which wraps an fd) is Core-internal only.
5. `IsolationHandle` is an opaque ID (serializable), not a process handle
6. Trait methods document ordering assumptions: "This method is safe to call concurrently with X" or "Must be called after Y completes"

**Why:**
- Dev mode (in-process, `tokio::mpsc`) and production mode (gRPC over Unix socket) use the same traits
- If a type is not serializable, it silently works in dev mode and breaks in production
- `tokio::mpsc` guarantees FIFO. gRPC does not. If code relies on call ordering, it must be documented.

### Decision 7: Authority and override ladder

**Choice:** Add to `systems-architecture.md` a precedence section:

```
runtime invariant > team policy > workspace policy > user preference
```

**Concrete examples:**

| Setting | Who wins | Why |
|---|---|---|
| Disable MOTOKO Tier 1 entirely | Cannot. Runtime invariant. | Kernel enforcement survives any config. |
| Set isolation to Level 0 for all agents | Team policy can forbid this. User preference alone can lower. | Team policy wins over user preference. |
| Change keybinding for pane split | User preference. | No security implication. |
| Lower isolation from L2 to L1 | User can, with confirmation + audit log. Team policy can block. | Lowering is allowed but audited. |
| Disable audit logging | Cannot. Runtime invariant. | Tamper-evidence is structural. |

**What is NOT an invariant (can be customized freely):**
- Visual layer (themes, fonts, colors, transparency)
- Keybindings
- Session defaults (tab naming, layout presets)
- MAGGI model choice, system prompt, cost limits
- Which observable events to subscribe to

### Decision 8: Logging and tracing sketch

**Choice:** `tracing` crate with structured spans. Separate from MOTOKO audit log.

- `tracing` = debugging and operational observability. Goes to stderr/file. Developers read it.
- MOTOKO audit log = security record. Append-only JSONL. Tamper-evident. MOTOKO reads it.

These are **separate systems** that happen to log overlapping events. `tracing` can be noisy, filtered, reconfigured. The audit log cannot.

Log levels: `ERROR` (broken invariant), `WARN` (recoverable issue), `INFO` (lifecycle events), `DEBUG` (internal state), `TRACE` (byte-level).

Detailed logging architecture is a SHOULD-during-coding item. The key decision is: **tracing and audit are separate, not unified.**

### Decision 9: Testing strategy sketch

**Choice:**
- Unit tests per crate (`cargo test -p lain-core`)
- Integration tests in `tests/` for cross-crate interactions
- Property-based tests for VTE parsing (via `proptest`)
- Security tests: attempt forbidden operations under each isolation level, verify they fail
- Terminal acceptance matrix: vttest baseline, OSC 133, bracketed paste, alternate screen, TUI apps — defined as the integration test suite grows
- Performance benchmarks via `criterion` for MOTOKO Tier 1 and render latency

Detailed test plan evolves with implementation. The key decision: **security tests are first-class, not afterthoughts.**

## Risks / Trade-offs

**[`lain-types` gravity well] → Mitigation: internal module structure with clear ownership per module. If any module exceeds ~500 lines, evaluate extraction. Review quarterly.**

**[Two-tier events add complexity] → Mitigation: the audit log write path is simple (append JSONL + fsync). The complexity is in the bridge (mandatory events also go to broadcast). If the bridge becomes a source of bugs, remove it and let consumers read the audit log directly.**

**[Core owns VTE state = Core is heavy] → Mitigation: Core was already the heaviest crate (renderer, VTE, isolation). Adding explicit ownership of `Term` doesn't change the dependency graph — `alacritty_terminal` was always in Core. The risk is Core becoming a monolith. Watch for this; if Core exceeds ~10k lines, consider splitting renderer into its own crate.**

**[Serialization overhead in dev mode] → Mitigation: in dev mode, `serde` derives exist but serialization never happens (direct function calls). The overhead is compile-time only (derive macros). Runtime cost is zero in dev mode.**

**[Level 1 mount table is Linux-specific] → Accepted. ADR-002 says Linux-first. The mount table is behind a trait boundary. macOS implementation is a separate future concern. The trait API (`CoreIsolationApi`) is platform-agnostic; only the implementation is Linux-specific.**

**[Navi recovery after crash depends on Core's PTY registry] → Mitigation: Core maintains a `HashMap<PtyId, PtyState>` that Navi queries on restart. If Core also crashed (full process restart), both rebuild from scratch — Navi from persisted TOML, Core creates new PTYs. Scrollback and VTE state are lost on full restart (documented limitation).**

## Open Questions

1. **Scrollback memory budget.** Per-pane ring buffer — but how many lines? 10k? 100k? Configurable? This affects memory per pane significantly. Defer to implementation spike.

2. **Cost accounting flow.** `CostAccumulator` exists on `Session` but the reporting/enforcement pipeline is undesigned. Not a cargo-init blocker but needs attention before MAGGI implementation.

3. **First-run UX.** Open question #1 in `open-questions.md`. Not addressed here. Needs its own design pass.

4. **Plugin event subscription.** The two-tier event model works for built-in quanta. When plugins arrive (WASM via wasmtime), they'll need a subscription API. The `tokio::broadcast` tier is compatible (plugins subscribe like any other consumer), but plugins must not receive mandatory events directly. Defer to plugin API design.
