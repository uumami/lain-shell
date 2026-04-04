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

> **TODO:** Define error strategy. `thiserror` for library errors, `anyhow` for application? Or a custom error hierarchy?

### Logging and tracing

> **TODO:** `tracing` crate is the standard. Define log levels, structured fields, and how tracing integrates with MOTOKO's audit log (they are separate — tracing is for debugging, audit is for security).

### Concurrency model

Tokio async for I/O-bound work (PTY reads, network, API calls). OS threads for CPU-bound work (rendering, pattern matching). The render loop should not compete with async tasks for CPU time.

### Build system

Cargo workspaces. One workspace, multiple crates:

> **TODO:** Define crate boundaries once architecture solidifies. Likely:
> - `lain-core` (PTY, VTE, renderer, config, CLI)
> - `lain-navi` (multiplexer, sessions)
> - `lain-motoko` (security, audit)
> - `lain-maggi` (agent)
> - `lain-wired` (gRPC, MCP, socket)
> - `lain-shell` (binary, composes all)

### Testing strategy

> **TODO:** Define. Unit tests per crate. Integration tests for cross-quantum interactions. Property-based tests for VTE parsing. Benchmark suite for performance targets. Security tests for MOTOKO (attempt forbidden operations, verify they're blocked).

---

## Repository Structure

> **TODO:** Define once crate boundaries are settled. Likely:
> ```
> lain-shell/
> ├── crates/
> │   ├── core/
> │   ├── navi/
> │   ├── motoko/
> │   ├── maggi/
> │   └── wired/
> ├── src/           (binary entry point)
> ├── docs/
> ├── .lain/         (example/default config)
> └── tests/         (integration)
> ```
