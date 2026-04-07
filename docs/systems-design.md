# lain-shell — Systems Design

> *Overall technology choices, stack decisions, and implementation strategy.*
> *For boundaries and interfaces, see `systems-architecture.md`.*
> *For values and principles, see `philosophy.md`.*
> *For decision rationale, see `decisions/`.*

---

## Language

**Rust.** Sole language for the entire platform. Polyglot (Rust+Go, Rust+Zig) was considered and rejected — the single-ecosystem benefit outweighs Go's ergonomic edge for networking, and Zig is pre-1.0. See ADR-003.

The one exception: the bootstrap installer may be a shell script or, if a compiled binary is needed, Go is acceptable for that specific artifact. The bootstrap is not lain-shell — it's a ~200-line tool that runs once.

---

## Technology Stack — Decided

These are decisions, not candidates. For the investigation history, see `stack/investigation.md`. For individual decision rationale, see the linked ADRs.

### Use (crate dependencies)

| Crate | Purpose | Quantum | Notes |
|---|---|---|---|
| `alacritty_terminal` | VTE parser, cell grid, terminal state | Core | Years of vttest-compliant correctness work. Do not reimplement. |
| `portable-pty` | PTY creation, child process, resize | Core | WezTerm's extraction. Battle-tested, cross-platform. |
| `wgpu` | GPU rendering pipeline | Core | Own the pixels. WebGPU standard (W3C). See ADR-004. |
| `softbuffer` | CPU rendering fallback | Core | Required for headless/SSH. |
| `cosmic-text` | Font shaping, bidi, ligatures, emoji | Core | Pure Rust, most complete option. |
| `tokio` | Async runtime | All | Non-blocking PTY I/O, API calls, servers. Uncontroversial. |
| `tonic` | gRPC server/client | THE WIRED | Typed, schema-driven, over tokio. |
| MCP SDK (Rust) | MCP server | THE WIRED | Standard agent protocol. |
| `serde` + `toml` | Config parsing | Core | Human-readable, git-friendly. Fits philosophy. |
| `aho-corasick` | Multi-pattern matching | MOTOKO | Compiled, microsecond latency. Tier 1 PTY output scanning. |
| `lancedb` | Vector store (embedded) | MAGGI | Local-first RAG. No separate process. |
| `tantivy` | Full-text search | MAGGI | Keyword retrieval. Complements vector search. |
| `wasmtime` | WASM plugin sandbox | Core | Plugin isolation. Zellij validates the approach. |

### Build (custom)

| Component | Quantum | Why custom |
|---|---|---|
| Rendering pipeline | Core | Must own every pixel for overlays, themes, agent panels. See ADR-004. |
| Overlay compositor | Core | Terminal content + MOTOKO alerts + agent status + status bars, layered. |
| Glyph atlas and cache | Core | GPU text rendering, tightly integrated with wgpu pipeline. |
| Block model / semantic scrollback | Core | Structured command history for agents. See ADR-005. |
| Theme engine | Core | Image backgrounds, watermarks, transparency, hot-reload. |
| Navi (multiplexer) | Navi | Agent-aware sessions. tmux model doesn't fit. See ADR-001. |
| Session persistence | Navi | Typed session serialization specific to lain-shell's model. |
| Layout engine | Navi | Pane splits, resize, arrangement. |
| Attach/detach protocol | Navi | Unix socket server/client for session reconnection. |
| Isolation Manager | Core | Four-level isolation (ADR-009). Namespace, seccomp, containers, host proxy. |
| Host proxy + shims | Core | Transparent proxying of Docker/GPU/tool commands from inside isolation. |
| MOTOKO Tier 1 | MOTOKO | seccomp-BPF profiles, namespace setup, network policy. Custom per lain-shell's policy model. |
| MOTOKO Tier 2 | MOTOKO | Behavioral baselines specific to lain-shell's session semantics. |
| Rule engine | MOTOKO | Turing-incomplete DSL, Falco-shaped. Signed, compiled to efficient matchers. |
| Audit log | MOTOKO | Append-only JSONL + hash chain. Simple, inspectable, tamper-evident. |
| Agent tool system | MAGGI | `lain` CLI commands as JSON Schema tool definitions. |
| Permission/policy engine | Core | `.lain/` manifest parsing, enforcement, validation. Mirror pattern (ADR-010). |
| THE WIRED protocol translation | THE WIRED | MCP/gRPC/Unix socket → internal command model. |
| Bootstrap | — | Shell script (or Go binary if compiled needed). Not lain-shell itself. |

---

## Platform Abstraction

Linux first (ADR-002). All platform-specific code behind trait boundaries.

| Surface | Linux (implement now) | macOS (implement later) |
|---|---|---|
| Security enforcement | seccomp-BPF via `libseccomp` bindings | macOS Sandbox (investigate alternatives to deprecated `sandbox-exec`) |
| Process isolation | Namespaces + rootless Podman | macOS containers / App Sandbox |
| System observation | auditd | Endpoint Security Framework |
| GPU rendering | Vulkan via wgpu | Metal via wgpu (same crate, different backend) |
| Display server | Wayland (primary), X11 (fallback) | AppKit / Cocoa |

The trait boundaries must be designed with both platforms in mind, even though only Linux is implemented initially. Review trait designs against macOS equivalents periodically.

---

## Performance Targets

| Metric | Target | How to measure |
|---|---|---|
| Core RSS (GPU) | < 80 MB | `smaps_rollup` under typical session |
| Core RSS (headless) | < 15 MB | `smaps_rollup` with softbuffer, no GPU |
| Startup to first prompt | < 100ms | Wall clock, cold start |
| Render latency | Parity with Ghostty | `typometer` or equivalent input-to-pixel measurement |
| MOTOKO Tier 1 | < 10μs per check | Benchmark seccomp + pattern match |
| MOTOKO Tier 2 | < 1ms per analysis cycle | Benchmark statistical analysis pass |

Ghostty is the performance benchmark for rendering. Measure against it continuously.

---

## Cross-Cutting Concerns

### Error handling

**`thiserror` everywhere. `LainError` in `lain-types`. No `anyhow` in libraries.**

All public trait methods that cross crate boundaries return `Result<T, LainError>`. `LainError` is defined in `lain-types/src/error.rs`:

```rust
#[derive(Debug, thiserror::Error, serde::Serialize, serde::Deserialize, Clone)]
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

**Rules:**

1. `LainError` derives `serde::Serialize + serde::Deserialize` — it must round-trip through gRPC without information loss.
2. Internal crate errors (`std::io::Error`, `wgpu::Error`, `toml::de::Error`) are converted to `LainError` at the crate boundary via `impl From<InternalError> for LainError`. The conversion includes enough context to be actionable (resource name, ID, reason).
3. `anyhow` is NOT used in library crates. It is permitted only in `src/main.rs` (the binary) for top-level error reporting.
4. Every `LainError` variant carries enough context to be actionable without a stack trace.

**Why:**
- `anyhow::Error` is not serializable — breaks gRPC transport.
- `anyhow::Error` erases the type — callers can't match on variants.
- MAGGI needs to match error types to provide useful explanations.
- MOTOKO needs to distinguish security errors from operational errors.
- THE WIRED needs to map errors to appropriate gRPC status codes (`NotFound` → `NOT_FOUND`, `PermissionDenied` → `PERMISSION_DENIED`).

### Logging and tracing

**`tracing` crate for operational observability. Separate from MOTOKO's audit log.**

These are two distinct systems:
- **`tracing`** = debugging and operational observability. Goes to stderr or log file. Developers read it. Configurable, filterable, can be noisy.
- **MOTOKO audit log** = security record. Append-only JSONL. Tamper-evident (hash chain). MOTOKO reads it. Cannot be reconfigured by agents.

They log overlapping events (e.g., session creation appears in both), but they serve different audiences and have different guarantees.

**Log levels:**
- `ERROR` — broken invariant, unrecoverable failure
- `WARN` — recoverable issue, degraded operation
- `INFO` — lifecycle events (session created, pane split, agent spawned)
- `DEBUG` — internal state changes, trait call details
- `TRACE` — byte-level (PTY I/O, event bus traffic)

**Structured spans:** every async operation creates a `tracing::Span` with relevant IDs (`session_id`, `pane_id`, `pty_id`). This enables filtering logs by session or pane.

### Concurrency model

Tokio async for I/O-bound work (PTY reads, network, API calls). OS threads for CPU-bound work (rendering, pattern matching). The render loop should not compete with async tasks for CPU time.

### Transport discipline

Dev mode (in-process) and production mode (gRPC over Unix sockets) use identical trait APIs. This means:

1. **All cross-boundary types derive `serde::Serialize + serde::Deserialize`.** Every type in a public trait method signature (parameters and return types) must round-trip through JSON/protobuf without loss.
2. **IDs are newtypes over serializable primitives.** `SessionId`, `PaneId`, `TabId`, `PtyId`, `AttachId`, `IsolationHandle` — all newtypes over `u64` or `uuid::Uuid`. They derive `Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash`.
3. **No OS handles in public APIs.** Raw file descriptors, channel senders, and process handles are crate-internal. Public APIs use serializable IDs that the owning crate maps to internal handles.
4. **Ordering assumptions are documented.** Every trait method that has ordering dependencies states them in doc comments: "Precondition: session must exist" or "Safe to call concurrently with X." `tokio::mpsc` guarantees FIFO; gRPC does not. Code must not silently depend on in-process ordering.

The dev-mode implementation is "gRPC without the wire" — same types, same error variants, same ordering constraints. The only difference is transport.

### Build system

Cargo workspaces. One workspace, seven member crates:

| Crate | Owns | Notes |
|---|---|---|
| `lain-types` | Shared IDs, events, errors, command model, inter-quantum API traits | Every quantum crate depends on this. Internal module structure for future extractability. |
| `lain-core` | PTY, VTE, renderer, config, isolation manager, command router, event bus | Heaviest crate. Owns `alacritty_terminal`, `wgpu`, `portable-pty`. Implements traits from `lain-types`. |
| `lain-navi` | Sessions, tabs, panes, layout, attach/detach, persistence | Uses Core API traits from `lain-types`. Does NOT depend on `lain-core`. |
| `lain-motoko` | Security policy, audit log writer, scanners, rule engine | Single writer to audit log. Implements `AuditSink`. Monitors PTY streams. |
| `lain-maggi` | Operator agent, tools, RAG, model backend | Uses `lancedb`, `tantivy`. Interacts with Navi/Core via traits from `lain-types`. |
| `lain-wired` | MCP/gRPC/Unix socket protocol translation | Uses `tonic`, MCP SDK. Does NOT depend on `lain-core`. |
| `lain-shell` | Binary entry point, supervisor, composition | Composes all crates via `Arc<dyn Trait>`. The only crate that depends on everything. |

**`lain-types` internal module structure:**

```
lain-types/src/
├── lib.rs        pub mod declarations
├── ids.rs        SessionId, PaneId, TabId, PtyId, AttachId, IsolationHandle, etc.
├── events.rs     Event, MandatoryEvent, EventMetadata
├── command.rs    LainCommand, AuthContext, Params, LainResponse
├── error.rs      LainError hierarchy
└── traits/
    ├── mod.rs
    ├── core.rs   CorePtyApi, CoreRenderApi, CoreIsolationApi, CoreConfigApi, CoreBlockApi, CoreRouterApi
    ├── navi.rs   NaviApi
    ├── motoko.rs MotokoApi
    ├── maggi.rs  MaggiApi
    ├── wired.rs  WiredApi
    └── audit.rs  AuditSink
```

The `traits/` module contains all inter-quantum API trait definitions. This means no quantum crate needs to depend on another — they all import traits from `lain-types`. The binary composes implementations via `Arc<dyn Trait>`.

If any module exceeds ~500 lines, evaluate extraction into its own crate. The module boundaries make this cheap.

**Dependency graph:**

```
lain-types ←── lain-core
     ↑
     ├── lain-navi
     ├── lain-motoko
     ├── lain-maggi
     └── lain-wired
              │
       lain-shell (binary, depends on all)
```

Rules:
- All quantum crates depend on `lain-types`. No exceptions.
- **No quantum crate depends on any other quantum crate.** Traits are in `lain-types`; the binary wires implementations.
- No circular dependencies.

### Testing strategy

- **Unit tests**: per crate (`cargo test -p lain-core`). Each crate owns its unit tests.
- **Integration tests**: top-level `tests/` directory for cross-crate interactions.
- **Property-based tests**: `proptest` for VTE parsing (random byte sequences → no panics, no memory corruption).
- **Security tests**: first-class, not afterthoughts. Attempt forbidden operations under each isolation level, verify they fail. Test seccomp profiles, mount namespace visibility, host proxy allowlist enforcement.
- **Terminal acceptance matrix**: vttest baseline, OSC 133 prompt detection, bracketed paste, alternate screen, TUI apps under multi-client attach. Defined incrementally as integration test suite grows.
- **Performance benchmarks**: `criterion` for MOTOKO Tier 1 latency (<10μs), render latency (parity with Ghostty), startup time (<100ms).

---

## Repository Structure

```
lain-shell/
├── Cargo.toml              workspace root
├── crates/
│   ├── lain-types/         shared IDs, events, errors, command model
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ids.rs
│   │       ├── events.rs
│   │       ├── command.rs
│   │       └── error.rs
│   ├── lain-core/          PTY, VTE, renderer, config, isolation, router, bus
│   ├── lain-navi/          sessions, tabs, panes, layout, attach/detach
│   ├── lain-motoko/        security policy, audit log, scanners, rules
│   ├── lain-maggi/         operator agent, tools, RAG, model backend
│   └── lain-wired/         MCP/gRPC/Unix socket translation
├── src/
│   └── main.rs             binary entry point, supervisor, composition
├── docs/                   architecture, design, decisions, philosophy
├── tests/                  cross-crate integration tests
└── .lain/                  example/default config
```
