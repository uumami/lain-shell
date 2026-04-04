# Stack Investigation

> *Technology candidates evaluated during design. Decisions are marked.*
> *For decision rationale, see `docs/decisions/` (ADRs).*
> *For the finalized stack, see `docs/systems-design.md`.*

---

## Core Language

~~**Rust** — strongest candidate. See ADR-003 (proposed).~~

**DECIDED: Rust.** Sole language. See ADR-003. Polyglot (Rust+Go, Rust+Zig) considered and rejected — single-ecosystem benefit outweighs Go's ergonomic edge, Zig is pre-1.0.

~~**C++ (C++20/23)** — maximum performance, memory safety complexity.~~
**Rejected.** Memory safety complexity increases security risk surface — contradicts MOTOKO's philosophy.

~~**Zig** — interesting, too early for a foundational bet.~~
**Rejected.** Pre-1.0. Ecosystem not ready. Watch for future subsystem use.

---

## PTY and Process Management

**DECIDED: `portable-pty`** — WezTerm's extraction. Cross-platform, battle-tested.

~~**`nix` crate** — direct POSIX bindings. More control, more work.~~
**Rejected as primary.** May be used alongside `portable-pty` for specific POSIX operations (e.g., namespace setup for MOTOKO).

~~**Direct `libc` bindings** — maximum control. Likely overkill.~~
**Rejected.** No need given portable-pty and nix crate.

---

## VTE Parser

**DECIDED: `alacritty_terminal`** — VTE parser, cell grid, scrollback. Vttest-compliant. Used by Alacritty, Zed.

~~**`vte` crate** — lower-level parser.~~
**Rejected.** Less batteries-included. No reason to go lower level.

~~**Custom** — only if above can't support specific rendering requirements.~~
**Rejected.** Too high cost for a solved problem.

---

## GPU Rendering

**DECIDED: `wgpu`** — own the pipeline. WebGPU standard (W3C). See ADR-004.

~~**GPUI** (Zed's framework) — wraps wgpu, includes `gpui-terminal`.~~
**Rejected.** Framework dependency contradicts "Core owns the pixels." See ADR-004.

**DECIDED: `softbuffer`** — CPU fallback. Required for headless/SSH.

---

## Font Rendering and Shaping

**DECIDED: `cosmic-text`** — bidi, ligatures, font fallback, emoji. Pure Rust. Most complete.

~~**`rustybuzz` + `swash`** — HarfBuzz port + rasterizer.~~
**Rejected as primary.** `swash` may be used for glyph rasterization if cosmic-text doesn't cover it. cosmic-text uses rustybuzz internally.

~~**`fontdue`** — fast, simpler.~~
**Rejected as primary.** May be useful for CPU fallback path.

---

## Async Runtime

**DECIDED: `tokio`** — dominant, mature, everything integrates. Not a place to be clever.

~~**`async-std`** — alternative. Less ecosystem momentum.~~
**Rejected.**

~~**`smol`** — lightweight.~~
**Rejected.**

---

## IPC and External Protocol (THE WIRED)

**DECIDED: `tonic`** — gRPC over tokio. Typed, schema-driven.

~~**`grpc-go`** — if Go were chosen for THE WIRED.~~
**Rejected.** Rust decision made.

**DECIDED: Rust MCP SDK** — standard agent protocol.

**DECIDED: tokio Unix socket** — for local plugins. JSON-RPC 2.0.

---

## Configuration Parsing

**DECIDED: `serde` + TOML** — default config format. Human-readable, git-friendly.

**DECIDED: JSON Schema** — validation layer. Published via `lain schema`.

**DECIDED: `mlua`** — optional advanced Lua config layer. Not the default.

---

## Process Isolation

**DECIDED: Rootless Podman** — full OCI container isolation for agent pods.

**DECIDED: Direct Linux namespaces** (`nix` crate) — lightweight isolation for less-sensitive tiers.

~~**macOS Sandbox** — required for macOS.~~
**Deferred.** Linux first (ADR-002). macOS parity designed for but not implemented initially.

---

## Security Enforcement

**DECIDED: seccomp-BPF** via `libseccomp` bindings — primary blocking. Immutable, kernel-enforced.

**DECIDED: `aho-corasick`** — compiled multi-pattern matching for PTY output scanning.

**DECIDED: auditd** (Linux) — system observation. No eBPF by default.

~~**`aya`** (Rust eBPF) — future opt-in.~~
**Deferred.** Advanced opt-in feature. Not default due to attack surface.

---

## MAGGI Model Backend (Local)

**DECIDED: Ollama** — dominant, zero friction, good Rust client support.

~~**llama.cpp server** — lower level.~~
**Rejected as primary.** Ollama wraps llama.cpp. No reason to go lower level for the default path.

~~**candle** (HF Rust ML) — in-process.~~
**Rejected.** Higher complexity, no clear benefit over Ollama for the default use case.

---

## MAGGI Agent Framework

**DECIDED: Direct tool-calling** — `lain` CLI commands as JSON Schema function definitions. No framework dependency.

~~**`rig`** — Rust-native agent framework.~~
**Rejected as initial choice.** Reconsider if tool-calling boilerplate becomes a real burden.

~~**Custom thin layer** — if above are insufficient.~~
**Deferred.** Start with direct tool-calling. Build thin orchestration only if needed.

---

## Vector Store (RAG)

**DECIDED: `lancedb`** — embedded, no separate process. Right size for local-first.

~~**`qdrant`** — Rust-based, embedded or separate.~~
**Rejected as primary.** Overkill for local single-user knowledge base.

**DECIDED: `tantivy`** — full-text keyword search. Complements vector search.

---

## Memory Allocator

**DECIDED: System allocator as baseline.** Benchmark `jemalloc` and `mimalloc` under real load. Switch only if measurable improvement.

---

## Audit Log

**DECIDED: Append-only JSONL + hash chaining** — simple, inspectable, tamper-evident.

~~**`rusqlite`** (SQLite)~~
**Deferred.** Add as an index layer if query complexity outgrows simple JSONL filtering.

~~**`sled`** — embedded KV store.~~
**Rejected.** Overkill.

---

## Plugin Sandbox

**DECIDED: `wasmtime`** — WASM runtime. Zellij validates the sandboxed plugin model.

---

## Dashboard and Overlay UI

**DECIDED: Custom GPU rendering** via wgpu — overlays are native GPU layers in the compositor. Not TUI escape sequences.

~~**`ratatui`** — TUI framework.~~
**Rejected as primary.** lain-shell IS the terminal — it doesn't render overlays by printing ANSI sequences to itself. May be useful for the CPU fallback path where overlays degrade to text-based rendering.

~~**Web dashboard** — separate local web UI.~~
**Deferred.** Future addition for complex visualizations that don't fit in terminal overlays.

---

## Multiplexer

**DECIDED: Custom (Navi).** See ADR-001.

~~**tmux control mode** — control tmux as substrate.~~
**Rejected.** Wrapper would be thicker than substrate. See ADR-001.

~~**Zellij** — Rust multiplexer, WASM plugins.~~
**Rejected as dependency.** Study for architecture patterns and WASM plugin model. Not a substrate.
