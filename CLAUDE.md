# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Run

```bash
# Build (opt-level 1 in dev for usable rendering performance)
cargo build

# Run
cargo run

# Build/test a single crate
cargo build -p lain-core
cargo test -p lain-core

# Run all tests
cargo test

# Check without building artifacts
cargo check
```

`RUST_LOG=debug cargo run` enables structured tracing output. Use `RUST_LOG=lain_core=trace` to filter to a specific crate.

## Workspace Structure

Cargo workspace with seven crates. Dependency graph is strict:

```
lain-types  <-- lain-core
    ^
    |-- lain-navi
    |-- lain-motoko
    |-- lain-maggi
    |-- lain-wired
         |
    lain-shell (binary, depends on all)
```

**Hard rule: no quantum crate depends on any other quantum crate.** All inter-quantum API traits live in `lain-types/src/traits/`. The binary (`src/main.rs`) is the only place that wires implementations together via `Arc<dyn Trait>`.

## The Five Quanta

lain-shell is organized around five "quanta" — independent systems with defined boundaries:

| Crate | Quantum | Owns |
|---|---|---|
| `lain-core` | Core | PTY, VTE state (`alacritty_terminal`), GPU/CPU renderer, config, isolation manager, command router, event bus |
| `lain-navi` | Navi | Session → Tab → Pane hierarchy, layouts, attach/detach, persistence |
| `lain-motoko` | MOTOKO | Security policy, audit log (single writer), PTY output scanners, rule engine |
| `lain-maggi` | MAGGI | Operator agent, tools, RAG (lancedb + tantivy), model backend |
| `lain-wired` | THE WIRED | MCP server, gRPC server, Unix socket listener — translates external protocols to `LainCommand` |
| `lain-types` | (shared) | `LainError`, IDs, events, command model, all inter-quantum trait definitions |
| `lain-shell` | (binary) | Entry point, supervisor, composition of all quanta |

Core is special: it cannot be a separate process because it owns the window and renderer. The `lain-shell` binary IS Core plus supervisor logic.

## Dev Mode vs. Production Mode

The same async trait API is used in both modes. The calling code never knows which it is:

- **Dev (current):** trait implementations are in-process — direct function calls
- **Production:** implementations are gRPC clients over Unix sockets

This means all types in public trait method signatures (params and returns) must derive `Serialize, Deserialize`. OS handles (file descriptors, channel senders) are never exposed in public APIs — only serializable ID newtypes.

## Communication Patterns

Three patterns, each used where it fits:

1. **Direct calls** — request/response between quanta (e.g., Navi → Core: create PTY)
2. **Event bus** (`tokio::broadcast`) — observation, best-effort, profile-filtered. Lagging receivers drop events.
3. **Dedicated channel** (PTY bytes → MOTOKO) — structurally isolated, cannot be observed by other quanta or congested by bus traffic

**Mandatory events** (security/audit/lifecycle) go through `AuditSink` → MOTOKO (single writer, fsynced, hash-chained audit log). The mandatory audit write must complete before the mutating operation proceeds (fail-closed). Every mandatory event is also bridged to the broadcast bus for non-security consumers, but the broadcast copy is not the source of truth.

**Event emission discipline:** every trait method that mutates state emits events from *within its implementation*, not from the caller.

## Error Handling

`thiserror` everywhere. `LainError` is defined in `lain-types/src/error.rs` and derives `Serialize + Deserialize` so it round-trips through gRPC.

**`anyhow` is not permitted in library crates.** It is only acceptable in `src/main.rs` for top-level error reporting. All library crates convert internal errors to `LainError` variants at the crate boundary.

Internal crate errors are converted via `impl From<InternalError> for LainError` and must include enough context to be actionable (resource name, ID, reason) — never a bare `Internal { message: "io error".into() }`.

## Renderer Architecture

`lain-core` has a GPU/CPU dual-path renderer:

- `GpuRenderer` — wgpu pipeline with glyphon for text, custom rect renderer for backgrounds/cursors/decorations. Prefers sRGB surface formats.
- `CpuRenderer` — softbuffer fallback for headless/SSH
- `CellGrid` — shared extraction layer that reads `alacritty_terminal::Term` and produces a flat `Vec<CellInfo>`. Both renderers consume `CellGrid`. Tracks dirty rows for incremental updates.
- `Renderer` — enum that dispatches to either path

Frame pacing targets 60 fps (16.667ms). PTY wakeups are coalesced behind a `FRAME_INTERVAL` gate. Cursor blink uses a separate `BLINK_INTERVAL` timer.

## Isolation Levels

The Isolation Manager (in Core) resolves isolation level as `max(agent_default, repo_policy, directory_policy)` — highest restriction always wins. Four levels:

- Level 0: no isolation, MOTOKO PTY scanning only
- Level 1: seccomp-BPF + PID/mount namespace (default)
- Level 2: rootless Podman container
- Level 3: Level 2 + network dropped

Levels 1-3 include a host proxy alongside the sandbox for transparent forwarding of allowed host tools (docker, nvidia-smi, etc.) via shim binaries.

## .lain/ Config Mirror Pattern

`.lain/` in the project repo is declarative source (agent-writable). The active config that MOTOKO and the Isolation Manager actually read lives outside agent reach at `~/.local/state/lain-shell/workspaces/<project-hash>/`. `lain config sync` diffs and requires user confirmation before writing the safe copy. **The agent cannot configure its own cage.**

## Key Design Constraints

- All cross-boundary types: `#[derive(Serialize, Deserialize, Clone)]`
- IDs are newtypes over `u64` (`PtyId`, `SessionId`, etc.) — never raw primitives in APIs
- No borrowed references across quantum boundaries — owned types only
- No circular crate dependencies
- `tracing` crate for operational logs; MOTOKO audit log is a separate, non-configurable, tamper-evident JSONL with hash chain
- Tokio async for I/O-bound work; OS threads for CPU-bound work (rendering, pattern matching)

## Performance Targets

- Core RSS (GPU): < 80 MB
- Core RSS (headless): < 15 MB
- Startup to first prompt: < 100ms
- Render latency: parity with Ghostty
- MOTOKO Tier 1 check: < 10μs

## Docs

Architecture decisions are in `docs/decisions/` (ADR-001 through ADR-010). `docs/systems-architecture.md` covers the process topology, communication map, and system invariants. `docs/systems-design.md` covers technology choices and implementation strategy. `docs/philosophy.md` covers principles — read before making structural changes.
