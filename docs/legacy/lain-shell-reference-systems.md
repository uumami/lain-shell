# lain-shell — Reference Systems and Prior Art
### A guide for implementation agents and stack decisions
> Pass this document alongside the main philosophy document. Together they give a complete picture.

---

## How to Use This Document

lain-shell is not built in a vacuum. Several existing systems already solve parts of the problem extremely well. This document maps those systems across three questions:

1. **What is this system and why is it relevant?**
2. **What should be directly borrowed, used as a crate, or used as a protocol?**
3. **What should be avoided or not inherited?**

A fourth column is added where applicable: **Use directly vs. study only** — the most important distinction for implementation decisions.

---

## Terminal Emulators

---

### Warp

**What it is:** The closest existing product to lain-shell. Built in Rust with GPU rendering. Introduced the block model — treating each command and its output as a discrete unit rather than raw scrollback bytes. Has moved toward an agentic development environment with agent sessions, project-level config files (WARP.md), and AI-assisted workflows.

**What to borrow:**
- The block/command-isolation concept for semantic scrollback — this is a real architectural idea, not just a UI trick. Each command, its output, its exit code, and its timestamp form a structured unit. Agents can query these units rather than scraping text.
- Proof that Rust + GPU rendering is production-viable and user-acceptable.
- The project-level behavior file concept (analogous to lain-shell's `.lain/workspace.toml`).

**What to avoid:**
- Cloud account requirement — lain-shell requires no account.
- Closed source — nothing can be reused directly, only studied.
- Warp has drifted from being a terminal toward being an "AI coding platform that happens to have a terminal." lain-shell must stay a terminal that hosts agents, not the reverse.

**Use directly:** No — closed source. Study only.

---

### Alacritty — `alacritty_terminal` crate

**What it is:** A fast, minimal terminal emulator. Matters to lain-shell primarily because its terminal emulation core has been extracted as a reusable crate: `alacritty_terminal`. This crate contains the VTE/ANSI escape sequence parser, the cell grid state machine, the scrollback buffer, and the terminal state model. It is vttest-compliant and handles the Kitty graphics protocol.

**What to borrow:**
- **Use `alacritty_terminal` directly as a crate dependency.** Do not reimplement VTE. Implementing ANSI/VT220/xterm correctly takes months and vttest compliance is brutal. This crate represents years of correctness work. It is explicitly designed to be reused — Zed and multiple other projects use it.
- The grid cell model and scrollback approach as the reference for lain-shell's terminal state.

**What to avoid:**
- Alacritty the app's product philosophy of radical minimalism. lain-shell is a platform. Alacritty refuses to be one by design.
- Any assumption that lain-shell's rendering or feature scope should match Alacritty's intentional narrowness.

**Use directly:** Yes — `alacritty_terminal` crate is a direct dependency candidate.

---

### WezTerm — `portable-pty` crate

**What it is:** The most feature-complete serious terminal available. Built in Rust, GPU-rendered via wgpu, Lua-configurable, with tabs, splits, SSH multiplexing, image protocol support, and a built-in multiplexer. Relevant to lain-shell both as a feature completeness reference and as the source of the `portable-pty` crate.

**What to borrow:**
- **Use `portable-pty` directly as a crate dependency.** This is WezTerm's PTY abstraction extracted as a standalone library. It handles cross-platform pseudoterminal creation, child process management, and PTY resize. Robust, battle-tested, cross-platform.
- WezTerm's Lua configuration layer as inspiration for lain-shell's optional advanced config.
- Its image protocol and SSH session handling as feature references.
- Its approach to treating the multiplexer as an internal concern, not an external dependency.

**What to avoid:**
- WezTerm's Lua-first configuration model as the default experience. It is powerful but intimidating. lain-shell's default config should be TOML, with Lua as an opt-in advanced layer.
- Its multiplexer engine directly — lain-shell controls tmux externally rather than embedding a multiplexer, keeping the terminal core lighter.

**Use directly:** Yes — `portable-pty` crate is a direct dependency candidate.

---

### Ghostty

**What it is:** A new terminal emulator built in Zig by Mitchell Hashimoto (creator of Vagrant, Packer, Terraform). Obsessively focused on being the best pure terminal: fast, correct, low-latency, feature-complete. Uses Metal on macOS and OpenGL/Vulkan on Linux. No agents, no AI, no platform ambitions — just an excellent terminal.

**What to borrow:**
- The performance bar. Ghostty defines what "fast terminal" means in 2025–2026. lain-shell must meet or exceed Ghostty's render latency and throughput for its core terminal operation.
- The GPU rendering pipeline ownership philosophy — Ghostty owns every pixel and its render loop is tight and deterministic.
- Its vttest compliance culture — correctness is not negotiable.
- Its feature set (ligatures, image protocol, advanced keyboard protocol, true color) as a completeness checklist.

**What to avoid:**
- Zig as the language — too early for a foundational bet on the toolchain.
- Its intentional refusal to become a platform. lain-shell is explicitly a platform.

**Use directly:** No — closed source / Zig. Study only as performance benchmark.

---

### Zed Terminal (GPUI + `alacritty_terminal`)

**What it is:** Zed is an editor, but its embedded terminal is one of the most technically relevant architecture references available. It composes `alacritty_terminal` for emulation and GPUI (Zed's own GPU UI framework) for rendering. The terminal view is cleanly separated from the editor so the integration pattern is legible and instructive.

**What to borrow:**
- The exact integration pattern between `alacritty_terminal`, GPUI, and `portable-pty`. The source is open and the architecture is clean.
- The separation between `Terminal` (state), `TerminalView` (rendered element), and workspace context — a model lain-shell's own rendering layer should mirror.
- GPUI itself as a candidate rendering framework if full `wgpu` ownership proves too slow to develop. GPUI gives you `gpui-terminal` for free and handles Metal/wgpu underneath.

**What to avoid:**
- IDE-embedded assumptions. Zed's terminal is designed as a panel inside an editor, not a standalone application. The UX decisions do not transfer directly.

**Use directly:** Conditionally — `alacritty_terminal` (yes, already), GPUI (evaluate as an alternative to raw `wgpu` for faster early progress).

---

### Kitty

**What it is:** A GPU-rendered terminal focused on extending terminal protocols. Created the Kitty graphics protocol (inline images), the extended keyboard protocol (full modifier/key disambiguation), and terminal hints. These protocols have become defacto standards and are now supported by other terminals.

**What to borrow:**
- **Kitty graphics protocol** — inline image rendering in the terminal, directly relevant to MAGGI rendering structured output (diffs, diagrams, charts) inside the terminal.
- **Kitty keyboard protocol** — richer input handling, especially for complex keybindings in lain-shell's overlay and agent UI.
- The philosophy of pushing terminal capabilities forward through protocol design rather than application-layer hacks.

**What to avoid:**
- Nothing specific — Kitty is purely a protocol and inspiration reference.

**Use directly:** Protocol specification — implement support, do not use Kitty code directly.

---

## Agent Runtimes

---

### Claude Code, Codex CLI, OpenCode, Goose, Amp

**What they are:** The coding agents lain-shell is expected to host. They are not terminals — they are the tenants. But studying them reveals what agents actually need from the surrounding environment, what permission boundaries they expect, and what workflows are most common.

**What to borrow:**
- Tool schema design — what capabilities agents declare, what operations they request, what structured responses they expect.
- Approval/confirmation UX — how agents surface dangerous or irreversible actions for human review. This directly informs MOTOKO's Tier 2 CRITICAL pause-and-present model.
- Git worktree usage — Claude Code frequently uses multiple worktrees simultaneously. This is the primary motivation for lain-shell's flat peer-session model (multiple sibling agent sessions, no nesting).
- Session scoping — how agents scope context to a repository and how context is handed off or summarized at session boundaries.

**What to avoid:**
- The implicit assumption that the terminal is a dumb text pipe. lain-shell inverts this — the terminal is the platform, agents connect to it with structure.
- Agents reaching outside their declared scope without explicit permission. lain-shell's isolation model is designed exactly to prevent this.

**Use directly:** No — reference only. Use these tools as users to understand their workflows firsthand.

---

## Shell References

---

### Nushell

**What it is:** A shell that treats output as structured data — tables, records, lists — rather than raw text. Commands emit typed objects. Pipelines carry structure rather than byte streams.

**What to borrow:**
- The semantic history concept: if shell output can be typed, agents can query history structurally rather than scraping text. This directly informs lain-shell's semantic scrollback design.
- Structured pipeline thinking — even if lain-shell does not impose Nushell on users, its internal session model could treat command output as structured events rather than raw bytes.
- The queryable history model as inspiration for `lain history query` or similar introspection capabilities.

**What to avoid:**
- Replacing the user's shell. lain-shell is shell-agnostic by design. Nushell is an inspiration for internal semantics, not an imposed runtime.

**Use directly:** No — inspiration only for semantic model.

---

## Multiplexer References

---

### tmux

**What it is:** The dominant terminal multiplexer. lain-shell controls tmux via its control mode protocol rather than embedding or replacing its session engine.

**What to borrow:**
- Session persistence, detach/attach semantics — lain-shell inherits these by controlling tmux.
- Control mode protocol — the machine-readable interface lain-shell uses to manage tmux programmatically.
- tmux's stability and universal availability across Linux distributions.

**What to avoid:**
- tmux's flat permission model.
- tmux's lack of structured agent/session semantics.
- Building a new multiplexer engine from scratch when tmux's engine can be controlled.

**Use directly:** Yes — control tmux via control mode as the session multiplexer substrate. Design the abstraction layer so the substrate can be swapped later.

---

### Zellij

**What it is:** A newer terminal multiplexer written in Rust with a WASM plugin system, built-in layout engine, and more modern UX defaults.

**What to borrow:**
- Its WASM plugin architecture as inspiration for lain-shell's plugin isolation model — running plugins in a sandboxed Wasm VM is a strong security idea.
- Its layout DSL as inspiration for lain-shell's workspace layout config.
- Its Rust implementation for direct code study.

**What to avoid:**
- Replacing tmux with Zellij as the default substrate — Zellij is not as universally installed and adds a Rust/Wasm runtime dependency.

**Use directly:** No — inspiration and possible future alternative substrate.

---

## Security References

---

### Falco (CNCF)

**What it is:** A cloud-native runtime security tool. Uses kernel events (syscalls, network, filesystem) to detect anomalous behavior in containers and processes. Maintains a large community-contributed rule library.

**What to borrow:**
- MOTOKO's rule language should be informed by Falco's rule schema — condition-based, event-driven, human-readable, Turing-incomplete.
- The community rule ecosystem model — how Falco manages community contributions, rule versioning, and rule signing is directly relevant to lain-shell's community MOTOKO rules.
- The concept of structured runtime events as the basis for detection rather than sampling or polling.

**Use directly:** No — inspiration for MOTOKO rule design and community governance.

---

### Tetragon (Cilium)

**What it is:** A Kubernetes security and observability tool using eBPF for deep kernel-level visibility and enforcement.

**What to borrow:**
- The eBPF observability model as the reference for lain-shell's advanced opt-in eBPF tier.
- How Tetragon uses kernel-level events to detect lateral movement, privilege escalation, and exfiltration — directly relevant to MOTOKO's threat model.
- Their approach to policy-as-code for runtime behavior.

**Use directly:** No — inspiration for the advanced eBPF tier. Not a direct dependency.

---

### Sysdig

**What it is:** A deep system inspection and forensics tool. Captures system calls, network events, and process behavior for real-time analysis and postmortem review.

**What to borrow:**
- The postmortem workflow — how a session's activity can be reconstructed and explained after the fact from structured event logs.
- The forensic event review UX as inspiration for `lain postmortem`.
- Security event summarization — how raw kernel events become intelligible findings for a human reviewer.

**Use directly:** No — inspiration for `lain postmortem` design.

---

## What to Build vs. What to Use

This is the most important table in this document.

| Component | Decision | Reasoning |
|---|---|---|
| VTE parser / terminal state | **Use `alacritty_terminal` crate** | Years of correctness work, vttest-compliant, reusable by design |
| PTY management | **Use `portable-pty` crate** | WezTerm's own extraction, cross-platform, battle-tested |
| Session multiplexing engine | **Control tmux via control mode** | Stability, universal install, no need to rebuild |
| GPU rendering pipeline | **Build with `wgpu`** | Full control over the render loop and overlays |
| GPU rendering (fast path) | **Evaluate GPUI** | Zed's framework includes a working terminal view, faster early progress |
| CPU render fallback | **Build with `softbuffer`** | Required for headless and SSH |
| Font shaping | **Use `cosmic-text`** | Bidi, ligatures, emoji, pure Rust |
| Async runtime | **Use `tokio`** | Not controversial, dominant ecosystem |
| gRPC (THE WIRED) | **Use `tonic` (Rust) or `grpc-go` (Go)** | `tonic` if staying Rust, `grpc-go` if Go is chosen for THE WIRED |
| MCP server | **Use MCP SDK** | Standard agent protocol, do not reinvent |
| Config parsing | **Use `serde` + TOML** | Human-readable, git-friendly |
| Agent isolation | **Use rootless Podman** | No daemon, no root, OCI standard |
| Tier 1 enforcement | **Build with seccomp-BPF + `nix` crate** | Kernel-enforced, immutable — must be custom per lain-shell's policy model |
| Tier 2 analysis | **Build custom** | Behavioral baseline logic is specific to lain-shell's session semantics |
| Tier 3 reasoning | **Call model API** | Pluggable, no custom inference |
| MOTOKO rule engine | **Build custom, Falco-shaped** | Turing-incomplete, signed, specific to lain-shell's event model |
| Audit log | **Build custom (append-only JSONL + hash chain)** | Simple, inspectable, tamper-evident |
| Bootstrap binary | **Build custom, Go candidate** | Static, tiny, cross-compile trivially |
| Vector store (MAGGI memory) | **Use `lancedb`** | Embedded, no separate process, good Rust support |
| Full-text search | **Use `tantivy`** | Pure Rust, fast, complements vector search |
| TUI overlays | **Use `ratatui`** | Dominant Rust TUI, active community |
| Plugin sandbox | **Evaluate Wasm via `wasmtime`** | Zellij's model is the reference — sandboxed, safe, extensible |
| Memory allocator | **Profile first, then `jemalloc` or `mimalloc`** | Measure before committing |

---

## Summary for an Implementation Agent

Start by reading the main philosophy document. Then use this document to answer:

- **Which crates do I reach for first?** `alacritty_terminal`, `portable-pty`, `tokio`, `wgpu` or GPUI, `cosmic-text`, `serde`+TOML, `ratatui`, `lancedb`, `tantivy`.
- **Which existing product is closest to what we are building?** Warp for product comparison. Zed for architecture reference. Ghostty for performance bar.
- **What should never be reinvented?** VTE parsing, PTY management, gRPC, MCP protocol, font shaping, vector storage.
- **What must be built fresh?** The rendering overlay layer, MOTOKO's enforcement and analysis tiers, the semantic session model, the agent isolation and permission architecture, THE WIRED's lain-specific protocol, the bootstrap, the audit log.
- **What is the right way to approach security references?** Falco for rule language shape. Tetragon for eBPF model. Sysdig for postmortem design. None are direct dependencies.

The goal is not imitation. It is to avoid solving solved problems while building correctly the things that have not been built before.

---
*lain-shell — Built by NERV. Present day. Present time.*
