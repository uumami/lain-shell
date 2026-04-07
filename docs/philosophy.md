# lain-shell — Philosophy

### Built by NERV. For the agentic age.

> *"Present day. Present time."*

---

## How to Read This Document

This is the philosophy of lain-shell. It contains principles, values, and beliefs that guide every decision. It also contains ideas, possibilities, and open questions — these are marked explicitly.

- **Principle** — a constraint that shapes all decisions. Breaking it means rethinking the project.
- **Idea** — a possibility worth exploring. It may or may not survive contact with implementation.
- **Open question** — something deliberately unresolved. It needs design work, benchmarking, or real usage data.

The philosophy is the most stable layer. Architecture changes when components are reshaped. Design changes when implementations are revised. Philosophy changes only when beliefs change.

---

## Purpose

lain-shell is a **terminal operating platform**: a lean, programmable, security-first environment where humans, shells, tools, and coding agents coexist without the platform losing control of itself.

It is not a terminal emulator with AI bolted on. It is not an AI shell wrapper. It is a terminal rebuilt from first principles for a world where agents are real participants in software work.

The goal is to build the **best terminal for the agentic age**. Not the most popular. Not the most marketed. The best. If one user runs lain-shell and it is the most capable, most secure, most thoughtful terminal they have ever used, that is success. Quality is the only metric that matters.

### The central ideas

1. The terminal works perfectly with **no agent at all**.
2. Agents are first-class citizens when present.
3. Security is **opt-out, not opt-in** — the default is the highest restriction compatible with the use case (see ADR-009).
4. The platform stays **lightweight, inspectable, and durable** as agentic behavior grows.
5. Configuration is **explicit, human-readable, and version-controlled**.
6. The platform is worth replacing tmux/Zellij for reasons that have nothing to do with AI.

This document is the seed. It is not the implementation. Many questions are deliberately left open — they are marked as such. The purpose is to establish the philosophy, name the systems, define the boundaries, and give enough direction to begin building correctly.

---

## Core Principles

These are constraints. Every decision in lain-shell must be compatible with all of them simultaneously.

### 1. The platform stands alone

**Principle.** If every agent disappeared, lain-shell must still be worth using. Fast startup, low memory, reliable PTY, stable multiplexing, strong defaults, clean CLI.

This is not a hedge against agents failing. It is a design discipline. A platform that depends on AI for basic functionality is fragile. A platform that is excellent without AI and transformative with it is durable.

### 2. Agents are first-class, not mandatory

**Principle.** Agents deserve structured support — typed sessions, permission scopes, lifecycle events, cost tracking, structured IPC. They are not afterthoughts bolted onto a human terminal. But the system must never depend on them for core functionality.

### 3. Security is structural

**Principle.** The system cannot rely on promises, policies, or good behavior. If something is dangerous, it must be prevented by architecture — kernel restrictions, process isolation, permission manifests, network boundaries, auditable logs.

The reference point: the LiteLLM supply chain attack (March 2026). A trusted dependency was compromised through a poisoned security scanner. The payload harvested credentials, moved laterally through Kubernetes, and installed persistent backdoors. Severity 9.4/10. Real structural enforcement at the kernel and container level would have caught the credential file access attempt before any data left the system.

That is the standard. Not hypothetical protection. Structural prevention.

### 4. Lightweight is a product principle

**Principle.** Most useful behavior must happen without model invocation. Token costs and latency are not implementation details — they shape the user experience.

The lain-shell core targets under 80 MB with GPU rendering and under 15 MB headless. Hard constraint, not aspiration.

Agent isolation environments (namespaces or containers) are separate from lain-shell's core process. Their memory is not lain-shell's memory. A 200 MB agent process does not affect the terminal's RSS. The architectural separation is what makes the lightweight guarantee achievable alongside powerful agent capabilities. Isolation is a spectrum (Level 0-3), not a binary — even the lightest isolation (Level 1, namespace + seccomp) adds negligible overhead.

### 5. Human readability matters

**Principle.** Config, permissions, policies, audit logs, rules — all understandable to a human with a text editor. Encryption and signing are good. Black boxes are not.

### 6. The system must age well

**Principle.** Config must survive years. Team workflows must stay legible in git. The architecture tolerates changing models, providers, and workflows without forcing rewrites. No decision should create a dependency on a specific AI provider, model family, or protocol version.

### 7. Extreme customization is a core value

**Principle.** Every significant behavior should be configurable. The defaults are strong — a user who touches nothing should have an excellent experience. But every ceiling should be removable.

This means:

- **Visual layer** — themes, colors, fonts, transparency, image backgrounds, watermarks, status bar layout, overlay positions, animation preferences, everything
- **Behavior layer** — keybindings, split behavior, session persistence, scroll behavior, URL handling, bracket paste mode, all of it
- **Agent layer** — which model, what tools, what system prompt, cost limits, when it speaks, when it is silent
- **Security layer** — which tiers are active, what rules run, what opt-outs are in effect, postmortem schedule
- **Plugin layer** — the entire plugin system exists to extend customization beyond what the core ships
- **Distribution layer** — fork config, maintain dotfiles, share profiles with teams, apply named configurations in one command

The philosophy: **lain-shell bends to the user.** Not the other way around. For how customization interacts with security invariants and team policy, see the Authority and Override Ladder in `systems-architecture.md`.

### 8. Quality over adoption

**Principle.** The project optimizes for being the best tool, not the most used tool. One user who finds lain-shell indispensable is worth more than a thousand who find it adequate. Marketing, growth hacking, and engagement metrics are not goals. Craftsmanship is.

---

## The Quanta

lain-shell is composed of five fundamental systems — **quanta**. Each quantum is an indivisible unit with its own philosophy, architecture, and design. Each can be understood independently. Together they form the platform.

```
┌──────────────────────────────────────────────────┐
│                  User / Agents                   │
├────────┬────────┬────────┬────────┬──────────────┤
│ MAGGI  │ MOTOKO │  NAVI  │ WIRED  │              │
│ agent  │ watch  │  mux   │  API   │              │
├────────┴────────┴────┬───┴────────┴──────────────┤
│                    CORE                           │
│     PTY · Renderer · Config · CLI                 │
├───────────────────────────────────────────────────┤
│                OS / Kernel                        │
└───────────────────────────────────────────────────┘
```

### Core

The terminal itself. PTY management, rendering pipeline (GPU and CPU), configuration model, storage, CLI interface. Core is the foundation — every other quantum depends on it, but it depends on none of them.

Core owns the pixels. It opens the window, renders the grid, handles input, manages the `.lain/` directory. Without Core, nothing else exists.

### Navi

The multiplexer and session manager. Named after the personal computers in Serial Experiments Lain — the Navi is the interface through which Lain connects to the Wired, manages her windows, and gains power.

Navi owns sessions, panes, windows, layouts, attach/detach, and session persistence. It is a **purpose-built multiplexer** for the agentic age — not a wrapper around tmux, but a replacement that understands agent identity, permission scopes, cost tracking, and isolated processes (configurable per-pane isolation levels) natively.

Navi is built specifically for lain-shell. Where tmux tracks panes and windows as generic PTY containers, Navi tracks **typed sessions**: which agent is active, what permissions it holds, what its cost footprint is, what workspace it belongs to, what MOTOKO events have been recorded against it.

> **Idea:** Navi could expose a compatibility layer that accepts a subset of tmux keybindings and commands, easing the transition for tmux users. The internal model is entirely different, but the muscle memory can be preserved.

> **Idea:** Navi's session persistence could support "session templates" — named, shareable configurations that define a workspace layout with specific panes, agents, and permissions. Think of it as infrastructure-as-code for terminal workspaces.

### MOTOKO

The security and observation layer. Named after Major Motoko Kusanagi from Ghost in the Shell — the one who sees everything, questions everything, and operates at the boundary between machine, identity, control, and trust.

MOTOKO watches actions, enforces boundaries, records events, and escalates only when necessary. Its job is to be **quiet, cheap, and correct**. Not dramatic. Not theatrical. Structural.

### MAGGI

The terminal-native operator agent. Warm, approachable, always present when wanted. Named as a quiet Evangelion reference — visible to those who know, invisible to those who don't.

MAGGI is the **generalist operator** — not a coding specialist. It manages the environment, knows the platform deeply, coordinates specialist agents, explains security events, and helps the user do whatever they need.

MAGGI is a **role**, not a model. The intelligence is pluggable: a local model, a remote API, an external coding agent acting as MAGGI's engine, or nothing at all.

### THE WIRED

The connectivity layer. Named after the network in Serial Experiments Lain — the space where everything connects.

THE WIRED is how lain-shell faces outward. MCP server for agents. gRPC bridge for programmatic access. Unix socket for local tooling. Every surface is a translation layer over the same `lain` CLI internals. One implementation, many clients.

### Independence of the quanta

**Principle.** These systems are independent. The platform does not collapse if any one of them is absent.

- Without MAGGI: no operator agent. CLI, MOTOKO, sessions, plugins all work.
- Without MOTOKO: no security enforcement. The user explicitly chose this. It is logged.
- Without NAVI: single-pane mode. Core still renders, still runs a PTY.
- Without THE WIRED: no external agent connectivity. Everything local still works.
- Without Core: nothing works. Core is the foundation.

### Quanta boundaries

Each quantum owns its domain completely. Cross-quantum communication happens through defined interfaces, never through shared state or implicit coupling.

> **Open question:** What are the exact interfaces between quanta? This is an architecture question, not a philosophy question. The philosophy constrains: interfaces must be typed, auditable, and documented. The architecture defines what they actually are.

---

## Identity and Naming

### lain-shell

The platform. Three references converge in the name:

- **Serial Experiments Lain** — the Wired, connection as identity, the system that becomes indistinguishable from the people inside it. lain is not an assistant. lain *is* the infrastructure.
- **Ghost in the Shell** — consciousness inside systems, agency inside networks, the shell as identity container.
- **The Unix shell** — the literal command interface through which humans have operated computers for fifty years.

lain is the substrate. It does not have a personality. It is the environment.

### NERV

The builder identity. Disciplined, high-stakes, systems-oriented, engineering-first. NERV builds systems powerful enough to change the world, with enough discipline to control them — and enough self-awareness to know when control is at risk.

### Navi

The multiplexer. In Serial Experiments Lain, the Navi is the personal computer — the interface through which Lain accesses the Wired, manages her connections, and gains power over her environment. In lain-shell, Navi is the session manager that organizes the user's workspace: panes, windows, layouts, attach/detach, session persistence.

The Navi is personal. It adapts to you. It remembers your workspace. It is the tool through which you navigate your systems.

### MOTOKO

The security and observation layer. Named after Major Motoko Kusanagi from Ghost in the Shell — the one who sees everything, questions everything, and operates at the exact boundary between machine, identity, control, and trust.

### MAGGI

The terminal-native operator agent. Named as a quiet Evangelion reference — visible to those who know, invisible to those who don't.

### THE WIRED

The connectivity layer. In Serial Experiments Lain, the Wired is the network that permeates everything — the space where identity, consciousness, and communication converge. In lain-shell, THE WIRED is the set of bridges and interfaces through which external agents, automation, remote clients, and integrations connect.

---

## Why Replace tmux and Zellij

tmux and Zellij are excellent tools. The argument for lain-shell is not that they are bad — it is that they were designed for a world where the terminal is purely a human interface.

### What tmux/Zellij cannot be

**Session semantics.** tmux tracks panes and windows as generic PTY containers. lain-shell (through Navi) tracks sessions as typed entities: which agent is active, what permissions it holds, what its cost footprint is, what workspace it belongs to. This cannot be bolted onto tmux cleanly — it requires a different data model at the foundation.

**Security boundary.** tmux has no permission model. Everything in a tmux session has the same access as the user. lain-shell enforces per-pane, per-agent, per-workspace permission boundaries at the kernel and container level.

**Agent protocol.** tmux communicates via control mode — text-based, fragile for programmatic use. lain-shell exposes structured protocols through THE WIRED. External agents get a real API, not a scraper.

**Configuration portability.** tmux config is a flat script. Zellij config is YAML. Neither is designed to travel with a project, be team-shareable in git, or be extended by a security layer. lain-shell's `.lain/` directory is all of these.

**Rendering pipeline ownership.** Both tmux and Zellij own their rendering in ways that make custom overlays difficult. lain-shell owns the rendering pipeline, so agent status overlays, MOTOKO indicators, cost readouts, and permission summaries are rendered as real UI elements — not escape sequence hacks.

### Why not wrap tmux

The original seed document proposed using tmux control mode as the session substrate. After analysis, this was rejected (see ADR-001).

The core problem: lain-shell needs a multiplexer that understands agent identity, permission scopes, isolated processes (configurable per-pane isolation levels), cost tracking, structured lifecycle events, and typed session metadata. tmux provides none of these. Wrapping tmux would mean building a full session manager on top and translating between lain-shell's typed model and tmux's string-based one at every interaction. The wrapper would be thicker than the substrate.

Navi is built from scratch. It takes the best ideas from tmux (session semantics, attach/detach, keyboard-driven workflow) and Zellij (modern UX, discoverable keybindings) but builds on a foundation designed for agents from the start.

> **Idea:** Consider studying tmux's edge case handling for PTY management — decades of terminal compatibility bugs are encoded in that codebase. The lessons are valuable even if the code isn't reused directly.

> **Idea:** Zellij's WASM plugin architecture is interesting for Navi's extensibility model. Worth investigating even though lain-shell's plugin system is separate.

### What the user gets

The user who switches from tmux gets everything tmux gives — sessions, panes, windows, attach/detach, keyboard-driven workflow — plus:

- Typed, agent-aware session management
- Structured, auditable permission boundaries
- A config model that travels with projects in git
- A renderer that shows agent context without polluting shell output
- Cost tracking and resource awareness per session
- Session templates and workspace profiles
- Configurable isolation per pane (namespace, container, or air-gapped)

---

## The Standalone Principle

**Principle.** lain-shell is a complete, self-contained terminal. It does not require an existing terminal to host it. It is not a shell plugin, a tmux skin, or a wrapper around something else.

### What standalone means

lain-shell opens a window, renders a PTY, handles input, manages sessions. If every tool it integrates with disappeared tomorrow, lain-shell would still open, still render, still run your shell.

### Zero interference

lain-shell does not touch, modify, or assume control of anything the user did not explicitly hand to it:

- Does not modify shell rc files unless asked
- Does not install global daemons or background services without permission
- Does not assume it is the only terminal on the system
- Coexists with iTerm2, Kitty, Alacritty, tmux, Zellij, any other tool
- Uninstalling lain-shell leaves the system exactly as it was

A user who installs lain-shell alongside their existing setup should feel nothing unexpected. They open lain-shell when they want to. Their other tools continue working.

### The inheritance model

lain-shell inherits the best of what exists rather than reimplementing it:

- **VTE compatibility** from proven terminal parsing (the VTE/ANSI standard, not reimplementing 30 years of escape sequences from scratch)
- **Shell behavior** from whatever shell the user runs — it does not impose a shell
- **OS security primitives** (seccomp, namespaces, macOS Sandbox) rather than building a custom kernel

What lain-shell builds fresh:

- The rendering pipeline that owns the pixels
- The agent protocol and session model (Navi)
- The security enforcement architecture (MOTOKO)
- The configuration model (`.lain/`)
- The agentic session semantics
- The operator agent framework (MAGGI)
- The external connectivity layer (THE WIRED)

This is why lain-shell can be both ambitious and achievable. The hard infrastructure problems (PTY handling, VTE parsing, font shaping) are solved. The problem worth solving — a terminal designed for a world where agents are real — is not.

---

## Distribution Philosophy

### First-class packaging

lain-shell must be installable the way developers install serious tools. Not just a GitHub release binary.

Target package managers:

- **`apt` / `apt-get`** — Debian and Ubuntu
- **`dnf` / `rpm`** — Fedora, RHEL, derivatives
- **`pacman` / `AUR`** — Arch Linux
- **`Homebrew`** — macOS and Linux
- **`cargo install`** — for Rust developers and source builds
- **Nix flake** — for reproducible and declarative environments
- **Direct binary release** — signed, for air-gapped and manual installs

Every release is signed. Every release has a published checksum. Installation is a single command.

### `lain-bootstrap` — the thin installer

> **Idea.** lain-shell ships its own bootstrap layer. A small, standalone, statically linked binary whose only job is to get lain-shell running on any machine and then hand off to MAGGI.

The bootstrap is intentionally dumb:

1. Detect host OS, architecture, available package managers
2. Pull the correct lain-shell binary and verify its signature
3. Set up the minimum environment lain-shell needs to start
4. Fire lain-shell and let MAGGI finish the rest

**MAGGI is the intelligence. The bootstrap is the bridge.** Different RHEL version? MAGGI negotiates. No GPU? MAGGI configures CPU rendering. Missing font? MAGGI installs one. The complexity lives in MAGGI, not the installer.

> **Idea:** The bootstrap could detect Nix and offer to use the Nix path instead. MAGGI can later suggest the switch — "you're on NixOS, want the Nix package? You get atomic rollbacks."

### Two parallel paths

**Path A — Universal:** `apt install lain-shell`, `brew install lain-shell`, direct signed binary, or bootstrap. Static binary, MAGGI handles per-machine adaptation.

**Path B — Nix:** `nix profile install github:nerv/lain-shell` or a home-manager module. Fully declarative, reproducible, every dependency pinned. MAGGI still activates but has less work because Nix already resolved the environment.

Both paths produce the same running lain-shell. They are parallel options for different user preferences.

### Cluster deployment

> **Idea.** For mixed-OS clusters:
>
> - **Immediate:** Static binary + bootstrap via existing tooling (ansible, terraform, scp). MAGGI handles per-node adaptation.
> - **Structured:** Proper package repo support (apt, dnf) for managed nodes.
> - **Declarative:** Nix flake for NixOS-managed nodes. Atomic updates, instant rollbacks.

---

## Offline and Air-Gapped Operation

**Principle.** lain-shell must work in fully offline environments. For HIPAA, defense, and sensitive research contexts, this is the primary use case — not a niche.

When no internet is available:

- Core functions completely — PTY, rendering, multiplexing, config, CLI
- MOTOKO Tier 1 and 2 function completely — no network required
- MAGGI functions if a local model is configured (Ollama or compatible)
- External coding agents requiring API access are unavailable — the system explains this clearly
- Plugin registry unavailable for new installs — `plugins.lock` ensures existing plugins work from cache
- Audit logs remain local — no behavior change

**Principle.** The system never silently degrades. It clearly indicates what is unavailable and what alternatives exist.

---

## Security Philosophy

> *Detailed enforcement architecture lives in `quanta/motoko/`. This section covers the philosophical foundation.*

### Security is real, not theater

MOTOKO is not a feature for marketing. It is a real enforcement layer with structural teeth. The LiteLLM attack is the reference point. The standard MOTOKO is held to: structural prevention, not hypothetical protection.

### Secrets never reach agents as raw values

> **Idea.** A Secret Manager holds credentials encrypted at rest. Agents receive scoped, short-lived proxy tokens that expire with the session. If the pod is killed, the token is immediately revoked. The agent cannot exfiltrate a secret it never held.

### Audit is tamper-evident

**Principle.** The audit log is hash-chained. Modifying any entry invalidates all subsequent entries. Detectable by design.

### The watching layer cannot be silently eliminated

**Principle.** If MOTOKO dies, agent sessions die. There is no state where MOTOKO is absent, agents are running, and nobody knows.

### Opt-out is real but deliberate

**Principle.** Security is opt-out. But opt-out must be explicit, granular, and visible:

- Opt-out is per-feature, per-workspace, per-session — not a global off switch
- Every opt-out is logged and visible in the status overlay
- Team/enterprise policy can prevent opt-out for specific rules
- MAGGI can apply opt-out configurations from pre-defined profiles
- The audit log reflects exactly what was disabled, when, by whom

Opt-out must be real and usable — otherwise the system is paternalistic. But it must be deliberate and visible — otherwise it is not security, it is theater.

### lain-shell's own supply chain

lain-shell itself updates. This creates a trust dependency on NERV as a software publisher. That trust must be bounded and explicit:

- All binaries are signed by NERV
- The user's system verifies signatures before applying updates
- Update channels are configurable — stable, beta, or pinned
- Air-gapped environments receive updates via signed offline packages
- The update mechanism itself is audited

NERV becomes a trusted party. The document acknowledges that explicitly.

### Nix as a complementary security layer

> **Idea.** Nix's security properties are concrete and complementary to MOTOKO:
>
> - **Immutable content-addressed store** — packages in `/nix/store` are read-only, content-addressed. A compromised dependency cannot overwrite itself in place. Complementary to MOTOKO's runtime detection.
> - **Reproducible builds** — same inputs always produce the same binary. Matters for audits and compliance.
> - **No implicit global state** — no writes to `/usr/lib`. No accidental version conflicts.
> - **Atomic rollback** — `nix profile rollback` is instant and total.
> - **Sandboxed builds** — network-isolated, filesystem-isolated. The build cannot phone home.
>
> Nix protects the *installation path*. MOTOKO protects *runtime behavior*. Together they cover build-to-execution.

### Differential privacy for sharing

> **Idea.** When logs must be shared with compliance or security reviewers, an anonymized export with differential privacy preserves aggregate patterns while stripping identifiers. Optional, explicit, off by default.

---

## The `.lain/` Configuration Model

**Principle.** Every project gets a `.lain/` directory alongside `.git/`. A dedicated namespace that never touches the source tree.

```
.lain/
├── permissions.toml     <- security contract (commit this)
├── config.toml       <- layout and agent configuration (commit this)
├── policies.toml        <- behavioral rules (commit this)
├── plugins.lock         <- pinned plugin versions (commit this)
├── agents/              <- conversation history (user decides)
├── knowledge/           <- local RAG index (gitignore - binary)
└── audit/               <- tamper-evident audit log (gitignore - sensitive)
```

The committed files travel with the project. New team members clone the workspace behavior, security contract, and policy set automatically.

### Permissions vs. Policies

**Permissions** answer: what is structurally possible?
- This agent may access `api.openai.com`
- This agent may write to `./src/`
- This agent may read the secret named `OPENAI_KEY`

**Policies** answer: what should happen, and under what conditions?
- Block `git push --force` without confirmation
- Require confirmation before production deploys
- Alert when session cost exceeds $5
- Switch to cheaper model when approaching budget
- Kill agent sessions that attempt SSH key access

Both are explicit, human-readable, and Turing-incomplete by design. Both live in `.lain/`. Both are version-controlled.

### Granularity

Permissions and policies can be applied at any level:

- **User-global** — applies everywhere for this user
- **Per-workspace** — applies to all sessions in a project directory
- **Per-agent** — applies only to a specific agent type or instance
- **Per-session** — applies only for the duration of a single session
- **Per-command** — applies only when specific commands are run

### Pre-defined profiles

> **Idea.** lain-shell ships with a library of pre-defined profiles:
>
> - Security: strict, standard, permissive
> - Compliance: HIPAA, SOC2 (strict + additional specific rules)
> - Workspace: single-pane focused, multi-pane development, agent-heavy
> - Cost: no limits, conservative, strict budget
>
> Profiles are starting points, not cages. Users create and share their own.

---

## The Update Contract

**Principle.** Three invariants. Forever.

1. **User config is never modified by the updater** — updates touch only system defaults
2. **Rollback is always possible** — `lain update rollback` always works, previous binary is kept
3. **Migration is explicit** — schema changes are versioned, failures halt the update, nothing silently breaks

---

## The Raw Escape Hatch

**Principle.** One keybind drops to a completely raw terminal: no overlays, no MAGGI, no MOTOKO semantic analysis, no policy engine. For debugging lain-shell itself. Always logged, never blocked.

The system must never trap the user inside its own abstractions.

---

## Accessibility

**Principle.** lain-shell must not regress the terminal's existing accessibility:

- All overlays have text equivalents — nothing conveyed only through color or animation
- All interactive elements are keyboard-navigable
- High-contrast mode available
- OS `prefers-reduced-motion` and `prefers-contrast` respected
- Screen reader compatibility for overlay elements
- Guided mode is inherently more accessible than a raw prompt

---

## Agent Isolation and Swarm Architecture

**Principle.** As agentic workflows grow, a terminal may host many concurrent agents:

- MAGGI managing the environment
- A coding agent in repository A
- A different coding agent in repository B
- A background analysis agent processing logs
- An automation agent running scheduled tasks
- A testing agent running CI jobs locally

**None of these agents communicate with each other by default.** Each runs in its own isolated pod with its own permission scope, network policy, filesystem view, cost tracking, and audit trail. They are neighbors, not collaborators.

Cross-agent communication requires **explicit, declared permission** in the manifests of both agents. The communication channel is mediated by lain-shell core (through THE WIRED) — agents do not reach each other directly. lain-shell routes the message, enforces the permission, logs the exchange.

MOTOKO watches all agent sessions simultaneously. An anomaly in one session does not automatically affect others — but MOTOKO can correlate events across sessions and detect patterns that span multiple agents.

> **Idea:** This architecture is compatible with agentic swarm frameworks and orchestration patterns. lain-shell does not fight these patterns — it gives them a secure substrate. A user running an agent orchestration framework can route inter-agent messages through THE WIRED and lain-shell enforces the permission model around them.

---

## Multi-Machine Sync

> **Idea.** `~/.config/lain-shell/` is a git repository. The user points it at their own private remote. NERV never operates a sync server. No account required.
>
> **Always sync:** permission manifests, workspace layouts, plugin lists, agent backend config, policies.
>
> **Optional sync, encrypted:** session history, agent logs. Age-encrypted before any push — the remote never sees plaintext.
>
> **Never sync:** secret keys, knowledge vector indexes, raw audit logs, renderer state.
>
> Chezmoi users can manage `~/.config/lain-shell/` with Chezmoi. lain-shell never interferes.

---

## Plugin Ecosystem

> **Idea.** No central NERV-operated marketplace. Default registry is community-operated. Anyone can run their own — community, enterprise, or local air-gapped `file://` path.

**Principle.** Every plugin is cryptographically signed. `plugins.lock` pins exact versions per project.

Plugin trust levels range from sandboxed VM with zero filesystem access to signed native code requiring explicit user approval. MAGGI can manage plugins but cannot elevate trust levels without human confirmation.

### Plugin author experience

> **Idea.** Plugin documentation is a first-class deliverable:
> - What the plugin API exposes
> - What a plugin can do that MAGGI cannot (low-level rendering hooks, custom protocol handlers, persistent background processes)
> - What MAGGI can do that a plugin should not replicate
> - How to write, sign, and publish a plugin
> - How to write configuration schemas
>
> MAGGI can access this documentation and help users understand it.

---

## Cost Management

**Principle.** Cost is a policy, not a feature. An empty rules array means no limits. Every dimension is trackable: per session, per workspace, per agent, per model backend, per time period, lifetime total.

Available actions at threshold: warn, pause and confirm, hard block, automatic backend switch, webhook notification, email alert.

MAGGI surfaces cost proactively. MOTOKO treats unusual cost spikes as behavioral anomaly signals — not just accounting concerns.

---

## First Launch

> **Open question.** When lain-shell launches for the first time, it enters a **configuration moment**. Not a guided tutorial, not automatic setup, but an explicit conversation where the user defines what kind of environment they want.
>
> The philosophy:
> - The user should feel like they are setting up a serious tool, not dismissing a wizard
> - The outcome is a real `.lain/` configuration they understand and own
> - MAGGI can help if a model is configured
> - If no model, a structured CLI flow
> - The result can be a named profile for future workspaces
>
> The specific UX is deliberately left open for dedicated design work.

---

## What lain-shell Is Not

- Not a cloud service — no account required, no telemetry by default, no phone-home
- Not opinionated about your shell — bash, zsh, fish, nushell all work identically
- Not a coding agent — it hosts agents, it is not one
- Not a product that monetizes data
- Not Electron — no web runtime, no Node.js, no large baseline
- Not dependent on Docker — rootless Podman or direct namespaces, no daemon
- Not security theater — every mechanism is structural, not promised
- Not a tmux wrapper — Navi is purpose-built

---

## Platform Target

**Decision.** Linux first. The architecture must be ready for macOS and WSL2, but the initial implementation is entirely Linux-focused.

This means:
- Security enforcement uses Linux primitives (seccomp-BPF, namespaces, auditd)
- Container isolation uses rootless Podman
- Rendering targets Linux display servers (Wayland, X11)
- macOS equivalents (Sandbox, Endpoint Security Framework) are designed for but not implemented initially
- WSL2 is a natural fit since it is Linux — it should work with minimal additional effort

The code must be structured so that platform-specific enforcement is behind traits/interfaces, making the macOS port a matter of implementing the same interfaces with different primitives — not restructuring the architecture.

See ADR-002 for the full rationale.

---

## Open Questions

These require dedicated design work, benchmarking, or real usage data. They are not deferred out of laziness.

1. **First launch UX.** What does the configuration moment feel like? How does MAGGI participate? How does the CLI flow work without a model?

2. **MOTOKO behavioral baseline.** How is the baseline built for a new agent? First session? First week? Per-agent type? Per-project? Architectural implications acknowledged, not answered.

3. **Trust boundary between MAGGI-as-backend and coding-agent.** If MAGGI uses Claude Code as its engine, and Claude Code is also running as a coding agent, how are the two sessions kept strictly isolated at the model-provider level? The architecture assumes separate sessions with separate contexts. Implementation must verify.

4. **MOTOKO Tier 3 model selection.** The smallest model that can reliably distinguish genuine exfiltration from false positive. Empirical question, answered with benchmarking.

5. **Community MOTOKO rule governance.** Who reviews and signs community rules? NERV as interim custodian. Community governance as the goal.

6. **Shared team sessions.** Architecture supports it. UX and permission model need design work.

7. **Navi session model details.** How does session serialization work? What is persisted vs. reconstructed? How does attach/detach interact with agent sessions?

8. **Plugin sandboxing specifics.** Lua VM? WASM? Both? What is the plugin API surface?

9. **Cross-quantum interface definitions.** What are the exact typed interfaces between Core, Navi, MOTOKO, MAGGI, and THE WIRED?

10. **Rendering pipeline architecture.** How do GPU and CPU paths coexist? How does the overlay system compose with the terminal grid?

---

## Ideas Worth Exploring

These are possibilities, not commitments. They may or may not survive implementation.

1. **Mobile companion.** THE WIRED is network-addressable from day one. A future mobile app could approve destructive agent actions from a phone while away from the desk. The architecture should support this even if the app ships much later.

2. **Session recording and replay.** Record terminal sessions (input/output/timing) for review, debugging, or training. Separate from audit logs — this is about reproducing what happened visually.

3. **Agent marketplace.** Not a NERV-operated marketplace, but a protocol for discovering and installing agent configurations. Like how you can share tmux configs, but for agent setups.

4. **Cost prediction.** Before an agent session starts, estimate the likely cost based on historical patterns for similar tasks. Let the user decide whether to proceed.

5. **MOTOKO learning from the community.** Anonymized, opt-in sharing of security patterns and anomaly detections. Privacy-preserving aggregate learning without exposing individual data.

6. **Workspace inheritance.** A team `.lain/` config that individual developers can extend but not override on security-critical settings. Like how `.gitignore` inherits.

7. **Terminal-native diff and merge.** Instead of launching an external tool, render diffs and merge conflicts natively in the terminal with proper syntax highlighting and agent assistance.

---

## Final Statement

lain-shell is a secure, observable, programmable terminal platform built for a world where agents are real participants in software work but must never become its unquestioned rulers.

lain is the platform — the substrate, the shell, the environment. It is worth using without any agent. Navi is the purpose-built multiplexer that understands agents natively. MAGGI is the terminal-native generalist operator agent — pluggable, optional, deeply knowledgeable about the platform. MOTOKO is the quiet watching layer that enforces security structurally, analyzes behavior statistically, and escalates to reasoning only when genuinely warranted. THE WIRED is the structured contract through which external agents and systems connect.

The system is built around truths that do not change: the platform stands alone, security is structural, configuration is explicit, token usage is a real resource, extreme customization is a first-class value, and the platform must be lightweight enough that its intelligence feels like leverage — not weight.

Quality is the only metric. One user who finds lain-shell indispensable validates the project.

*The terminal is the entry point to everything.*
*lain-shell is that terminal.*
*Present day. Present time.*

*— NERV | 2026*
