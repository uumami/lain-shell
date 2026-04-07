> **⚠ LEGACY DOCUMENT.** This is the original seed philosophy, preserved for historical context. It predates ADR-001 (custom multiplexer — this doc recommends tmux control mode) and ADR-004 (wgpu renderer — this doc recommends ratatui). For the current design, see `docs/philosophy.md`, `docs/systems-architecture.md`, and `docs/decisions/`.

# lain-shell — Complete Philosophy and Product Vision
### Built by NERV. For the agentic age.
> *"Present day. Present time."*

---

## Purpose

lain-shell is not just a terminal emulator and not just an AI shell wrapper. It is a **terminal operating platform**: a lean, programmable, security-first environment where humans, shells, tools, and coding agents can coexist without the platform losing control of itself.

The central ideas:

- The terminal works perfectly with **no agent at all**.
- Agents are first-class citizens when present.
- Security is **opt-out, not opt-in** — the default is maximum restriction.
- The platform stays **lightweight, inspectable, and durable** as agentic behavior grows.
- Configuration is **explicit, human-readable, and version-controlled**.
- The platform is worth replacing tmux/Zellij for reasons that have nothing to do with AI.

This document is the seed. It is not the implementation. Many questions are deliberately left open — they are marked as such. The purpose of this document is to establish the philosophy, name the systems, define the boundaries, and give a coding agent enough direction to begin building correctly.

---

## Why Replace tmux or Zellij

tmux and Zellij are excellent tools. The argument for lain-shell is not that they are bad — it is that they were designed for a world where the terminal is purely a human interface. lain-shell is designed for a world where the terminal is also an agent interface.

**Session semantics.** tmux tracks panes and windows. lain-shell tracks sessions as typed entities: which agent is active, what permissions it holds, what its cost footprint is, what workspace it belongs to. This cannot be bolted onto tmux cleanly.

**Security boundary.** tmux has no permission model. Everything in a tmux session has the same access as the user. lain-shell enforces per-pane, per-agent, per-workspace permission boundaries at the kernel and container level.

**Agent protocol.** tmux communicates via control mode — text-based, fragile for programmatic use. lain-shell exposes a structured MCP/gRPC protocol. External agents get a real API, not a scraper.

**Configuration portability.** tmux config is a flat script. Zellij config is YAML. Neither is designed to travel with a project, be team-shareable in git, or be extended by a security layer. lain-shell's `.lain/` directory is all of these.

**Multiplexer as controlled substrate.** lain-shell controls tmux via its control mode under the hood. The user gets tmux's stability and session compatibility without being limited to its interface model. When better multiplexers exist, swapping the underlying engine does not break the rest of the platform.

**Rendering pipeline ownership.** Both tmux and Zellij own their rendering in ways that make custom overlays difficult. lain-shell owns the rendering pipeline, so agent status overlays, MOTOKO indicators, cost readouts, and permission summaries are rendered as real UI elements — not escape sequence hacks.

The user who switches from tmux gets everything tmux gives (sessions, panes, windows, attach/detach) plus a structured, auditable, permission-aware agent environment on top, a config model that travels with projects, and a renderer that can show agent context without polluting shell output.

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

### THE WIRED

The connectivity layer — bridges and interfaces through which external agents, automation, remote clients, and integrations connect to lain-shell.

### MAGGI

The terminal-native operator agent. Warm, approachable, always present. Named as a quiet Evangelion reference — visible to those who know, invisible to those who don't.

### MOTOKO

The security and observation layer. Named after Major Motoko Kusanagi from Ghost in the Shell — the one who sees everything, questions everything, and operates at the exact boundary between machine, identity, control, and trust.

---

## The High-Level Model

Four conceptual systems:

1. **lain-shell core** — terminal platform, PTY, rendering, multiplexing, config, storage, CLI.
2. **MAGGI** — the terminal-native operator agent.
3. **MOTOKO** — the security and watching layer.
4. **THE WIRED** — external connectivity for outside agents and systems.

These systems are independent. The platform does not collapse if any one of them is absent.

---

## The Philosophy

**The platform comes first.** If every agent disappeared, lain-shell should still be worth using. Fast startup, low memory, reliable PTY, stable multiplexing, strong defaults, clean CLI.

**Agents are first-class, not mandatory.** They deserve structured support, not afterthought APIs. But the system must not depend on them.

**Security must be structural.** The system cannot rely on promises. If something is dangerous, it must be prevented by architecture — kernel restrictions, process isolation, permission manifests, network boundaries, auditable logs.

**Lightweight is a product principle.** Most useful behavior must happen without model invocation. Token costs and latency are not implementation details — they shape the user experience.

**Human readability matters.** Config, permissions, policies, audit logs, rules — all understandable to a human with a text editor. Encryption and signing are good. Black boxes are not.

**The system must age well.** Config must survive years. Team workflows must stay legible in git. The architecture tolerates changing models, providers, and workflows without forcing rewrites.

**Extreme customization is a core value.** Every significant behavior should be configurable. Permission manifests, cost policies, security tiers, agent backends, session layouts, plugin rules — all of it bends to the user's will. The defaults are strong. The escape hatches are real.

---

## What MAGGI Is

MAGGI is the **general-purpose terminal-native operator agent**.

It is the first agent the user talks to. It has no hard domain boundary. If the user asks it something and the tools and permissions allow it, MAGGI helps.

MAGGI can:

- manage sessions, panes, layouts, and workspace configuration
- install, configure, and manage plugins
- create and maintain permission manifests and policies
- write scripts and code when the user asks
- coordinate and hand off to external specialist coding agents
- explain MOTOKO events in plain language and present clear choices
- track and surface cost and resource usage
- operate in guided mode for beginners
- access lain-shell's own documentation to answer questions about itself
- create themes, apply watermarks, customize the visual environment
- help create, edit, and apply pre-defined security and workspace configurations

MAGGI knows how lain-shell works. When a user asks "how do I create a theme using an image as a background" or "write me a policy that blocks outbound connections to anything except npm", MAGGI can answer from documentation and then do it. This is what makes MAGGI useful as an operator agent rather than a generic chatbot.

MAGGI is **not** a coding specialist. Claude Code, Codex, OpenCode, and local coding agents are specialists. MAGGI is the generalist who decides whether to handle something directly or coordinate a specialist.

### MAGGI is one agent by default

Multi-agent voting architectures are philosophically elegant but practically expensive for common interactions. Routing a theme change through three agents is absurd. The default is **one MAGGI instance**. Multi-agent arbitration for genuinely high-consequence decisions is a suggestion for a future advanced mode — not a baseline requirement.

---

## MAGGI's Model Backend — Full Flexibility

MAGGI is a **role**, not a model. The intelligence is always pluggable.

### Using an external coding agent as MAGGI's engine

If the user has no local model and no direct API key but has Claude Code or Codex running, MAGGI can be powered by that agent. Claude Code becomes the reasoning engine for MAGGI's terminal-management domain. There is no architectural conflict — MAGGI's tools, role definition, and context are passed to Claude Code, which executes MAGGI's role alongside its coding capabilities.

**The isolation is critical here.** The Claude Code instance powering MAGGI is a **separate session** from any Claude Code instance running in a repository coding context. These two sessions do not share conversation state, permission scope, or tool access. They happen to use the same model provider but are architecturally distinct agents with distinct identities, distinct permissions, and distinct audit trails. A repository coding session cannot reach MAGGI's session and MAGGI's session cannot reach the repository coding session — unless explicit cross-agent permission is granted.

This distinction matters as agentic swarms become more common. lain-shell is designed to host many concurrent agents — MAGGI, multiple repository coding sessions, background automation agents, analysis agents — without any of them implicitly trusting or accessing the others.

### Local model

A locally hosted model via Ollama or compatible server. Zero marginal cost, fully private, works offline. The natural default when compute is available.

### Remote API model

Claude, GPT-4o, Gemini, or any OpenAI-compatible endpoint. The user provides their own key. MAGGI enforces configured token budgets.

### No model

MAGGI is absent. The CLI, MOTOKO's deterministic tiers, sessions, and plugins all work without it.

The user owns this choice completely.

---

## Agent Isolation and Swarm Architecture

This is one of the most important architectural principles of lain-shell and one of the least obvious.

As agentic workflows grow more sophisticated, a terminal may host many concurrent agents simultaneously:

- MAGGI managing the environment
- A coding agent in repository A
- A different coding agent in repository B
- A background analysis agent processing logs
- An automation agent running scheduled tasks
- A testing agent running CI jobs locally

**None of these agents communicate with each other by default.** Each runs in its own isolated pod with its own permission scope, its own network policy, its own filesystem view, its own cost tracking, and its own audit trail. They are neighbors, not collaborators.

Cross-agent communication requires **explicit, declared permission** in the manifests of both agents. The communication channel itself is mediated by lain-shell core — agents do not reach each other directly. lain-shell routes the message, enforces the permission, and logs the exchange.

This architecture is compatible with current agentic swarm frameworks and orchestration patterns. lain-shell does not fight these patterns — it gives them a secure substrate. A user running an agent orchestration framework can route inter-agent messages through THE WIRED interface and lain-shell enforces the permission model around them.

MOTOKO watches all agent sessions simultaneously. An anomaly in one agent session does not automatically affect others — but MOTOKO can correlate events across sessions and detect patterns that span multiple agents.

---

## What MAGGI Does in Practice

**Environment orchestration.** Prepares workspaces, layouts, panes, environment activation, and repeated workflows on entry to a project directory.

**Documentation access.** MAGGI has access to lain-shell's own documentation and can answer questions about how the platform works, what commands do what, how to create policies, how to configure plugins, and how to customize the environment. This is not optional — a platform this configurable must have an agent that knows it deeply.

**Configuration stewardship.** Creates and maintains `.lain/` files. Explains changes before making them. Requests confirmation for security-relevant modifications.

**Theme and visual customization.** Creates themes, applies image-based backgrounds and watermarks, adjusts colors and layout visuals. A user asking MAGGI to "create a dark theme using this image as a watermark" should get a working result.

**Permission and policy authorship.** When a project needs new access, MAGGI proposes the manifest change, explains exactly what is being granted, and waits for human confirmation. When a policy pattern is needed (blocking outbound traffic except to specific hosts, requiring confirmation before production commands), MAGGI writes it.

**Pre-defined configuration templates.** lain-shell ships with a library of pre-defined configurations — security profiles, workspace layouts, cost policies. MAGGI can apply these with a single request ("apply the HIPAA security profile to this workspace") or help the user create custom templates that can be shared and reused. This is a first-class feature, not an afterthought.

**Code and scripts when asked.** MAGGI writes scripts, config fragments, and helper code on request. No artificial domain restriction.

**Specialist coordination.** For deep autonomous coding, MAGGI connects and coordinates an external coding agent — defines context, sets permission boundaries, hands off, monitors.

**MOTOKO interpretation.** When MOTOKO flags an event, MAGGI translates it into plain language and presents clear choices. MOTOKO detects. MAGGI explains.

**Cost and resource awareness.** Tracks and surfaces token spend and resource usage. Not a blocker by default — honest accounting that the user can configure policies around.

**Guided mode.** For new users, MAGGI explains commands before execution, shows what will happen, and helps the user learn without hiding the underlying reality.

---

## First Launch — Configuration, Not Defaults

When lain-shell launches for the first time, it enters a **configuration moment** — not a guided tutorial, not an automatic setup, but an explicit conversation where the user defines what kind of environment they want.

This moment is a design problem to be solved during implementation, not prescribed here. The philosophy is:

- The user should feel like they are setting up a serious tool, not dismissing a wizard.
- The outcome is a real `.lain/` configuration that they understand and own.
- MAGGI (if a model is configured) can help with this conversation.
- If no model is configured, the configuration moment uses a structured CLI flow.
- The result can be a named profile — "this is my personal development setup" — that can be applied to new workspaces later.

The specific UX of this moment is explicitly left open for dedicated design work.

---

## MOTOKO — The Security and Watching Layer

MOTOKO watches actions, enforces boundaries, records events, and escalates only when necessary. Its job is to be **quiet, cheap, and correct**. Not dramatic.

### Security is real, not theater

This must be stated explicitly. MOTOKO is not a feature for marketing. It is a real enforcement layer with structural teeth.

The LiteLLM supply chain attack (March 2026) is the reference point. A trusted dependency was compromised through a poisoned security scanner. The payload harvested credentials, moved laterally through Kubernetes, and installed persistent backdoors. Severity 9.4/10. Real structural enforcement at the kernel and container level would have caught the credential file access attempt before any data left the system.

That is the standard MOTOKO is held to. Not hypothetical protection. Structural prevention.

### Three Tiers

**Tier 1 — Deterministic enforcement (always running, zero tokens, near-zero latency)**

Handles the majority of security decisions without any model. Cannot be disabled without explicit opt-out, which is logged and visible.

- seccomp-BPF syscall restrictions applied to agent processes at spawn — immutable once set, cannot be removed by the monitored process, enforced by the kernel
- Network policy at the container level — forbidden packets are dropped before they leave the pod
- Mount namespace restrictions — forbidden filesystem paths are structurally inaccessible, not just policy-blocked
- Compiled pattern matching on PTY output (Aho-Corasick — all patterns simultaneously, microsecond latency, zero tokens)
- Structured event log — every command, access attempt, and block is recorded with timestamp and context

Tier 1 never interrupts a running agent session. It silently allows, silently blocks, or emits events to Tier 2.

**Tier 2 — Statistical and heuristic analysis (zero tokens, periodic or triggered)**

No model invocation. Runs on schedule (default is a suggestion for implementation) or when Tier 1 emits a flag.

- Behavioral baseline comparison — how the baseline is initially established is an open implementation question, acknowledged here
- Output entropy analysis — does the agent's output look like normal coding work or encoded data
- Network frequency anomalies
- Resource consumption patterns relative to stated activity
- Cost tracking against configured thresholds

Verdicts:
- **NORMAL** — session continues uninterrupted
- **ANOMALY** — logged, non-blocking indicator in overlay, queued for postmortem. Session continues.
- **CRITICAL** — session paused (not killed). User sees exactly what triggered it and chooses: allow once, allow and update the manifest, block and kill with forensic snapshot.

If no human is present: CRITICAL defaults to block based on the configured policy.

**Tier 3 — Reasoning analysis (tokens, triggered only)**

Invoked only when genuinely needed:
- Tier 2 CRITICAL that is ambiguous
- End of session if anomalies were logged (async, does not block the terminal)
- Explicit user request: `lain postmortem`
- Scheduled review (configurable)

The Tier 3 reasoner is isolated: its own pod, no network access, no tools, no memory across sessions, no communication with MAGGI or any other agent. It receives a summarized event window — not raw full logs. It outputs a structured verdict with plain English reasoning. Token cost is bounded by design.

### The Postmortem Pattern

The key insight for token economy: **analyze in the quiet moments, not during the work.**

During an active coding session: Tier 1 and 2 only. Near-zero tokens. The session is uninterrupted. Events are recorded.

After the session (or when the user has idle compute): Tier 3 reviews flagged events. The postmortem is brief — what happened, what the likely explanations are, what rule adjustments are recommended. If nothing was flagged, the report is empty.

Work is not interrupted. Security is not theater.

### What happens mid-session

**Tier 1 block:** The syscall fails or the packet drops. The agent receives an OS-level error. It adapts or stops. Nothing visible to the user unless they check the status indicator. Session continues.

**Tier 2 ANOMALY:** Small non-blocking indicator in the status overlay. Session continues completely. Anomaly is queued for postmortem.

**Tier 2 CRITICAL:** Session is **paused, not killed**. The agent receives no new input. The user sees what triggered it and chooses: allow once, allow and update policy, block and kill.

**If MOTOKO itself dies unexpectedly:** All active agent sessions are paused or terminated. The system enters an explicit failure state. Silent loss of the watching layer is not acceptable.

### Why not full eBPF by default

Full eBPF observability introduces real attack surface — elevated privileges, real CVEs in the verifier, complexity that can be exploited. The safer base for most users:

- seccomp-BPF for blocking (immutable, inherited, kernel-enforced)
- auditd on Linux / Endpoint Security Framework on macOS for observation (designed for this, no new attack surface)
- Podman network policies and mount namespaces for isolation

Full eBPF is an opt-in advanced feature for users who understand the tradeoff. Not the default.

### MOTOKO's rules

- Encrypted at rest, decrypted into memory only at runtime
- Human-readable in plaintext form — users can understand what protects them
- Turing-incomplete — pattern matching and conditions only, no general computation (a reasoning agent cannot exploit a rule engine that cannot reason back)
- Signed so tampering is detectable
- Community rule contributions: anonymized submission optional, off by default

### Opt-out means opt-out with clarity

Security is opt-out. But opt-out must be explicit, granular, and visible:

- Opt-out is per-feature, per-workspace, and per-session — not a global off switch
- Every opt-out action is logged and appears in the status overlay
- A team or enterprise policy can prevent opt-out for specific rules
- MAGGI can apply opt-out configurations from pre-defined profiles
- The audit log reflects exactly what was disabled, when, and by whom

Opt-out must be real and usable — otherwise the system becomes paternalistic. But it must also be deliberate and visible — otherwise it is not security, it is theater.

---

## THE WIRED — The Connectivity Layer

THE WIRED is how lain-shell faces outward.

- **MCP server** — for agents that speak MCP natively (Claude Code, OpenCode, others)
- **gRPC bridge** — for programmatic integrations, CI/CD, companion apps
- **Unix socket** — for local scripts, plugins, and tooling

Every surface is a translation layer over the same `lain` CLI internals. One implementation, many clients. Every command through THE WIRED has structured output. `lain schema` dumps the full JSON Schema of every config file. `lain capabilities` tells any connecting agent exactly what the current session permits. `lain doctor` diagnoses configuration, missing dependencies, and security issues in plain language.

Agents never need to scrape terminal text. Every operation is machine-readable by design.

### Mobile and remote

THE WIRED is network-addressable from day one. A future mobile companion, remote dashboard, or shared team session connects over the same gRPC bridge used by local tooling. Auth is token-based. Session buffer serialization is efficient.

The mobile use case is approving a destructive agent action from a phone while away from the desk. The architecture supports it even if the app ships much later. For now this means: do not design THE WIRED in a way that would make this painful to add.

---

## The `.lain/` Repository

Every project gets a `.lain/` directory alongside `.git/`. A dedicated namespace that never touches the source tree.

```
.lain/
├── permissions.toml     ← security contract (commit this)
├── workspace.toml       ← layout and agent configuration (commit this)
├── policies.toml        ← behavioral rules (commit this)
├── plugins.lock         ← pinned plugin versions (commit this)
├── agents/              ← conversation history (user decides)
├── knowledge/           ← local RAG index (gitignore — binary)
└── audit/               ← tamper-evident audit log (gitignore — sensitive)
```

The committed files travel with the project. New team members clone the workspace behavior, security contract, and policy set automatically.

This behaves like `.github/` — a clearly owned namespace with no interference in source code.

---

## Permissions and Policies

**Permissions** answer: what is structurally possible?

Examples:
- This agent may access `api.openai.com`
- This agent may write to `./src/`
- This agent may read the secret named `OPENAI_KEY`

**Policies** answer: what should happen, and under what conditions?

Examples:
- Block any `git push --force` without confirmation
- Require confirmation before any production Kubernetes deploy
- Alert via webhook when session cost exceeds $5
- Switch to a cheaper model automatically when approaching monthly budget
- Kill any agent session that attempts to access SSH keys

Both are explicit, human-readable, and Turing-incomplete by design. Both live in `.lain/`. Both are version-controlled.

### Granularity

Permissions and policies can be applied at any granularity:

- **User-global** — applies everywhere for this user
- **Per-workspace** — applies to all sessions in a project directory
- **Per-agent** — applies only to a specific agent type or instance
- **Per-session** — applies only for the duration of a single session
- **Per-command** — applies only when specific commands are run

MAGGI can apply any of these. The user can also edit them directly. Pre-defined configuration templates can be applied in one command and modified from there.

### Pre-defined configuration profiles

lain-shell ships with a library of pre-defined profiles:

- Security profiles: strict (maximum restriction), standard (recommended defaults), permissive (minimal restriction, explicit opt-out)
- Compliance profiles: HIPAA, SOC2 (these are the strict profile with additional specific rules)
- Workspace profiles: single-pane focused, multi-pane development, agent-heavy workflow
- Cost profiles: no limits, conservative, strict budget

Profiles are starting points, not cages. MAGGI can apply a profile and the user can modify individual rules from there. Users can create and share their own profiles.

---

## Offline and Air-Gapped Operation

lain-shell must work in fully offline environments. This is not a niche — for HIPAA, defense, and sensitive research contexts, it is the primary use case.

When no internet is available:
- The lain-shell core functions completely — PTY, rendering, multiplexing, config, CLI
- MOTOKO Tier 1 and 2 function completely — no network required
- MAGGI functions if a local model is configured — Ollama or compatible local server
- External coding agents that require API access are unavailable — the system handles this gracefully and explains it
- The plugin registry is unavailable for new installs — `plugins.lock` ensures existing plugins still work from local cache
- Audit logs remain local — no behavior change in offline mode

The system never silently degrades in offline mode. It clearly indicates what is unavailable and what alternatives exist.

---

## Security — The Complete Picture

### Opt-out is real but deliberate

Already described above. Security is opt-out. Opt-out is granular, logged, and visible.

### Secrets never reach agents as raw values

The Secret Manager holds credentials encrypted at rest. Agents receive scoped, short-lived proxy tokens that expire with the session. If the pod is killed, the token is immediately revoked. The agent cannot exfiltrate a secret it never held.

### Audit is tamper-evident

The audit log is hash-chained. Modifying any entry invalidates all subsequent entries. Detectable by design.

### Differential privacy for sharing

When logs must be shared with a compliance auditor or security researcher, an anonymized export is available. Differential privacy preserves aggregate patterns while stripping individual identifiers. Optional, explicit, off by default.

### The watching layer cannot be silently eliminated

If MOTOKO dies, agent sessions die. There is no state where MOTOKO is absent, agents are running, and nobody knows.

### lain-shell's own update supply chain

lain-shell itself updates. This creates a trust dependency on NERV as a software publisher. That trust must be bounded and explicit:

- All lain-shell binaries are signed by NERV
- The user's system verifies the signature before applying any update
- Update channels are configurable — stable, beta, or pinned to a specific version
- Air-gapped environments can receive updates via signed offline packages
- The update mechanism itself is audited — what changed, what was verified, what was applied

NERV becomes a trusted party in this model. The document acknowledges that trust explicitly rather than hiding it.

---

## Multi-Machine Sync

`~/.config/lain-shell/` is a git repository. The user points it at their own private remote. NERV never operates a sync server. No account required.

**Always sync:** permission manifests, workspace layouts, plugin lists, agent backend configuration, policies.

**Optional sync, encrypted:** session history, agent logs. Age-encrypted before any push — the remote never sees plaintext.

**Never sync:** secret keys, knowledge vector indexes, raw audit logs, renderer state.

Chezmoi users can manage `~/.config/lain-shell/` with Chezmoi. lain-shell never interferes.

---

## Plugin Ecosystem

No central NERV-operated marketplace. The default registry is community-operated. Anyone can run their own — community, enterprise private, or local air-gapped `file://` path.

Every plugin is Ed25519-signed. `plugins.lock` pins exact versions per project — teams get identical versions.

Plugin trust levels range from sandboxed Lua VM with zero filesystem access to signed native code requiring explicit user approval. MAGGI can manage plugins but cannot elevate trust levels without human confirmation.

### Plugin authors and documentation

A plugin author building for lain-shell needs to know:
- What the plugin API exposes (what lain-shell surfaces for plugins to call)
- What a plugin can do that MAGGI cannot (low-level rendering hooks, custom protocol handlers, persistent background processes)
- What MAGGI can do that a plugin should not try to replicate
- How to write and publish a signed plugin
- How to write configuration schemas for their plugin

This documentation is a first-class deliverable of lain-shell — not an afterthought. MAGGI can access it and help users understand it. Plugin authors building with lain-shell should feel as supported as library authors building with a well-documented SDK.

---

## Rendering and Overlays

lain-shell owns the rendering pipeline. This is a core reason it exists as its own platform.

Owning the renderer means:
- Agent status indicators without escape sequence hacks
- MOTOKO alerts as real UI elements, not printed text
- Cost and resource readouts in the status bar
- Permission summaries for active agent sessions
- Guided mode overlays for beginners
- Diff views and structured agent output rendered natively
- Theme and visual customization including image-based backgrounds and watermarks

The GPU render path targets performance parity with the fastest existing terminals. The CPU fallback ensures operation in headless, SSH, and resource-constrained environments. On small screens or constrained contexts, the overlay can be minimized to a single-line status summary without losing any functionality.

---

## Cost Management

Cost is a policy, not a feature. An empty rules array means no limits. Every dimension is trackable: per session, per workspace, per agent, per model backend, per time period, lifetime total.

Available actions at threshold: warn, pause and confirm, hard block, automatic backend switch to cheaper model, webhook notification, email alert.

MAGGI surfaces cost proactively. MOTOKO treats unusual cost spikes as behavioral anomaly signals — not just accounting concerns.

---

## The Lightweight Guarantee

The lain-shell core — renderer, PTY, multiplexer, MOTOKO Tier 1 and 2, the CLI — targets under 80 MB with GPU rendering and under 15 MB headless. Hard constraint, not aspiration.

Agent pods are separate OS processes. Their memory is not lain-shell's memory. A 200 MB Claude Code pod does not affect the terminal's RSS. The architectural separation is what makes the lightweight guarantee achievable alongside powerful agent capabilities.

MAGGI wakes on events and user input. It does not run a continuous inference loop. When idle it consumes near-zero resources.

---

## The Raw Escape Hatch

One keybind drops to a completely raw terminal: no overlays, no MAGGI, no MOTOKO semantic analysis, no policy engine. For debugging lain-shell itself. Always logged, never blocked.

The system must never trap the user inside its own abstractions.

---

## The Update Contract — Three Invariants Forever

1. User config is never modified by the updater — updates touch only system defaults
2. Rollback is always possible — `lain update rollback` always works, previous binary is kept
3. Migration is explicit — schema changes are versioned, failures halt the update, nothing silently breaks

---

## Accessibility and Universality

lain-shell must not regress the terminal's existing accessibility:
- All overlays have text equivalents — nothing conveyed only through color or animation
- All interactive elements are keyboard-navigable
- High-contrast mode available
- OS `prefers-reduced-motion` and `prefers-contrast` respected automatically
- Screen reader compatibility for overlay elements
- Guided mode is inherently more accessible than a raw prompt

---

## Open Questions — Held Deliberately

These are not deferred out of laziness. They require dedicated design work, real usage data, or implementation-time decisions.

**First launch UX.** What does the configuration moment feel like? How does MAGGI participate if present? How does the CLI flow work if not? This is a design problem to be solved during implementation.

**MOTOKO behavioral baseline establishment.** How is the baseline built for a new agent? First session? First week? Per-agent type? Per-project? This is an implementation question with real architectural implications — acknowledged here, not answered.

**The trust boundary between MAGGI-as-Claude-Code-backend and Claude-Code-as-coding-agent.** If MAGGI uses Claude Code as its engine, and Claude Code is also running as a coding agent, how are the two sessions kept strictly isolated at the model-provider level? This is a known question. The architecture assumes they are separate sessions with separate contexts. The implementation must verify and enforce this.

**MOTOKO Tier 3 model selection.** The smallest model that can reliably distinguish genuine exfiltration from false positive. An empirical question answered with benchmarking during development.

**Community MOTOKO rule governance.** Who reviews and signs community rules? NERV as interim custodian. Community governance as the goal. Organizational question, not architectural.

**Shared team sessions.** Architecture supports it. UX and permission model need dedicated design work.

**Anonymous community rule contributions.** Worth designing for even if not shipped initially.

---


---

## Standalone First — A Core Design Principle

lain-shell is a **complete, self-contained terminal**. This is not a feature. It is a foundational design constraint that shapes every other decision.

### What standalone means

lain-shell does not require an existing terminal to host it. It does not wrap iTerm2, Kitty, Alacritty, or any other terminal emulator. It is not a shell plugin, a tmux skin, a Zellij layout, or a wrapper around something else. It is a terminal. It opens a window, it renders a PTY, it handles input, it manages sessions. Everything else — agents, security layers, multiplexing, overlays — is built on top of that independent foundation.

If every tool lain-shell integrates with disappeared tomorrow, lain-shell would still open, still render, still run your shell.

### Zero interference with existing environments

lain-shell does not touch, modify, or assume control of anything the user did not explicitly hand to it.

- It does not modify your shell's rc files unless you ask it to
- It does not install global daemons or background services without permission
- It does not assume it is the only terminal on the system
- It can coexist with iTerm2, Kitty, Alacritty, tmux, Zellij, and any other tool simultaneously
- Uninstalling lain-shell leaves the system exactly as it was before installation

A user who installs lain-shell alongside their existing setup should feel nothing unexpected. They open lain-shell when they want to. Their other tools continue working. There is no conflict.

### First-class distribution

lain-shell must be installable in the way developers already install serious tools. Not just a GitHub release binary — a properly maintained package in the repositories developers trust.

Target package managers:

- **`apt` / `apt-get`** — Debian and Ubuntu — the widest Linux user base
- **`dnf` / `rpm`** — Fedora, RHEL, and derivatives
- **`pacman` / `AUR`** — Arch Linux
- **`Homebrew`** — macOS and Linux
- **`cargo install`** — for Rust developers and source builds
- **Nix flake** — for reproducible and declarative environments
- **Direct binary release** — signed, for air-gapped and manual installs

Every package manager release is signed. Every release has a published checksum. Installation is a single command. No curl-pipe-bash required unless the user explicitly chooses it.

### Extreme customizability as a first-class value

lain-shell is not opinionated about how you use it. The defaults are strong — a user who installs it and touches nothing should have an excellent experience. But every significant behavior should be changeable.

This means:

- **Visual layer** — themes, colors, fonts, transparency, image backgrounds, watermarks, status bar layout, overlay positions, animation preferences, everything
- **Behavior layer** — keybindings, split behavior, session persistence, scroll behavior, URL handling, bracket paste mode, all of it
- **Agent layer** — which model powers MAGGI, what tools it has, what its system prompt says, how much it costs before pausing, when it speaks and when it is silent
- **Security layer** — which MOTOKO tiers are active, what rules run, what opt-outs are in effect, what the postmortem schedule is
- **Plugin layer** — the entire plugin system exists to extend customization beyond what the core ships
- **Distribution layer** — users can fork the configuration, maintain their own dotfiles, share profiles with teams, and apply named configurations to new workspaces in one command

The philosophy is: **lain-shell bends to the user**. Not the other way around. Opinionated defaults lower the cost of starting. Deep customization removes the ceiling on what the platform becomes.

### Easy to update, safe to update

Because lain-shell is a standalone application distributed through real package managers, updates follow the same path as any other system tool:

```
apt upgrade lain-shell
brew upgrade lain-shell
```

The update contract from the main document applies unconditionally: user config is never touched, rollback is always available, migrations are explicit and versioned. Updating lain-shell should feel like updating any other trusted tool — routine, automatic if desired, never surprising.

Auto-update is opt-in. The default is to notify and let the user decide. Enterprise and air-gapped environments can pin to specific versions and receive offline signed packages.

### The inheritance model

lain-shell inherits the best of what exists rather than reimplementing it:

- It inherits **session semantics** from tmux (via control mode), not by replacing tmux's engine
- It inherits **VTE compatibility** from the `alacritty_terminal` crate, not by re-implementing 30 years of ANSI standards
- It inherits **shell behavior** from whatever shell the user runs — it does not impose a shell
- It inherits **OS security primitives** (seccomp, namespaces, macOS Sandbox) rather than building its own kernel

What lain-shell builds fresh is the integration layer: the rendering pipeline that owns the pixels, the agent protocol, the security enforcement architecture, the configuration model, and the agentic session semantics. It stands on the shoulders of what already works and builds only what does not yet exist.

This is why lain-shell can be both ambitious and achievable. The hard infrastructure problems are solved. The problem worth solving — a terminal designed for a world where agents are real — is not.

---

## What lain-shell Is Not

- Not a cloud service — no account required, no telemetry by default, no phone-home
- Not opinionated about your shell — bash, zsh, fish, nushell all work identically
- Not a coding agent — it hosts agents, it is not one
- Not a product that monetizes your data
- Not Electron — no web runtime, no Node.js, no large baseline
- Not dependent on Docker — rootless Podman, no daemon
- Not security theater — every mechanism is structural, not promised

---

## Final Philosophy Statement

lain-shell is a secure, observable, programmable terminal platform built for a world where agents are real participants in software work but must never become its unquestioned rulers.

lain is the platform — the substrate, the shell, the environment. It is worth using without any agent. MAGGI is the terminal-native generalist operator agent that manages the environment, knows the platform deeply, and helps the user do whatever they need — including coordinate specialist coding agents. MAGGI's intelligence is entirely pluggable: a local model, a remote API, or even Claude Code itself if that is what the user has, running in a session that is strictly isolated from any repository coding session. MOTOKO is the quiet watching layer that enforces security structurally at the kernel and container level, analyzes behavior statistically without touching tokens, and escalates to reasoning only when something genuinely warrants it. THE WIRED is the machine-readable, structured contract through which external agents and systems connect.

The system is built around truths that do not change: the platform stands alone without agents, security is structural not promised, configuration is explicit and human-readable, token usage is a real resource, extreme customization is a first-class value, and the platform must be lightweight enough that its intelligence feels like leverage — not weight.

This is the seed. The implementation will answer many of the questions left open here. The philosophy will not change.

*The terminal is the entry point to everything.*
*lain-shell is that terminal.*
*Present day. Present time.*

*— NERV | 2026*

---

## Suggested Stack — Starting Points for Investigation

> *These are starting points, not decisions. Each section names three candidate options with brief rationale. The final stack will be chosen during implementation through benchmarking, profiling, and real usage. Treat this as a map of the territory, not a commitment.*

---

### Core Language

The terminal core, MOTOKO enforcement layer, and THE WIRED need to be fast, memory-safe, and close to the kernel. Three candidates:

**Rust** — the strongest candidate. Zero-cost abstractions, no GC pauses (critical for a real-time render loop), excellent PTY and syscall crates, and every modern high-performance terminal (Alacritty, WezTerm, Ghostty) is built on it. The ecosystem for what lain-shell needs is mature. Recommended starting point.

**C++ (modern, C++20/23)** — maximum performance ceiling, direct control over everything, and deep ecosystem for terminal and rendering work. The tradeoff is memory safety complexity and slower iteration. Worth knowing is an option if specific performance requirements demand it.

**Zig** — emerging, interesting memory model, excellent C interop, comptime is genuinely powerful. Still maturing for production systems. Worth watching, probably too early for a foundational bet.

*Recommendation to investigate: Rust first. Revisit if specific subsystems show friction.*

---

### PTY and Process Management

**`portable-pty`** (WezTerm's extracted crate) — cross-platform PTY handling, battle-tested, designed to be reusable outside WezTerm. Strong first choice.

**`nix` crate** — direct POSIX bindings in Rust. More control, more work, but no abstraction overhead. Good if `portable-pty` proves limiting.

**Direct `libc` bindings** — maximum control, zero abstraction. Only if both above prove insufficient for specific requirements. Likely overkill.

*Recommendation to investigate: `portable-pty` as starting point.*

---

### VTE Parser (ANSI/VT escape sequence handling)

**`alacritty_terminal`** — the VTE parser and cell grid state machine extracted from Alacritty. Designed to be reusable. Used by Alacritty, Zed, and others. Vttest-compliant. Strong first choice — implementing VTE correctly from scratch takes months.

**`vte` crate** — lower-level ANSI parser. More control over the state machine, less batteries-included. Good if `alacritty_terminal`'s grid model proves constraining.

**Custom implementation** — only if the above cannot support specific rendering requirements (e.g., deeply custom cell semantics for agent overlay integration). Very high implementation cost.

*Recommendation to investigate: `alacritty_terminal`. Custom only if necessary.*

---

### Session Multiplexer Substrate

**tmux (control mode)** — lain-shell controls tmux via its control mode protocol. Stable, universally installed, battle-tested session semantics, detach/attach, and persistence. Recommended — lain-shell owns the UI, tmux owns the session engine.

**Zellij** — written in Rust, WASM plugin system, modern architecture. Less universally installed. Worth watching as an alternative substrate, especially for the plugin architecture influence.

**Custom multiplexer** — full control over session semantics, but enormous implementation cost for something tmux already does well. Only if tmux control mode proves fundamentally limiting for the agent-aware session model.

*Recommendation to investigate: tmux control mode. Design the abstraction layer so the substrate can be swapped.*

---

### GPU Rendering

**`wgpu`** — WebGPU-over-Metal/Vulkan/DX12/WebGPU. Cross-platform, Rust-native, used by Ghostty and others. Full control over the render pipeline. Strong candidate for owning the renderer completely.

**GPUI** (Zed's GPU UI framework) — wraps `wgpu`/Metal and includes a `gpui-terminal` crate backed by `alacritty_terminal`. Higher abstraction, faster to a working terminal, less control. Worth evaluating for development speed.

**`softbuffer` + CPU rasterization** — pure CPU fallback, no GPU dependency. Critical for headless and SSH contexts. Should exist as a fallback regardless of which GPU path is chosen.

*Recommendation to investigate: `wgpu` for the GPU path with `softbuffer` as the CPU fallback. Design behind a trait so both coexist.*

---

### Font Rendering and Shaping

**`cosmic-text`** — handles bidirectional text, ligatures, font fallback, emoji. Pure Rust. Strong candidate.

**`rustybuzz` + `swash`** — `rustybuzz` for shaping (HarfBuzz port), `swash` for rasterization. More control, more composition. Good if `cosmic-text` proves limiting.

**`fontdue`** — fast pure-Rust rasterizer, simpler feature set. Good for the CPU fallback path where full shaping is less critical.

*Recommendation to investigate: `cosmic-text` as the primary path.*

---

### Async Runtime

**`tokio`** — the dominant async runtime in the Rust ecosystem. Non-blocking PTY reads, AI API calls, MCP server, gRPC — all naturally fit here. Strong first choice.

**`async-std`** — alternative runtime, similar feature set. Less ecosystem momentum than tokio currently.

**`smol`** — lightweight, composable. Worth considering if tokio proves too heavy for the core, but unlikely given tokio's performance profile.

*Recommendation to investigate: `tokio`. This is not a controversial choice.*

---

### IPC and External Protocol (THE WIRED)

**`tonic`** (gRPC over tokio) — typed, schema-driven, works over network and Unix socket, strong Rust ecosystem. Good for THE WIRED's gRPC bridge.

**MCP SDK** — the Model Context Protocol SDK for exposing lain-shell as an MCP server. Allows Claude Code, OpenCode, and any MCP-compatible agent to connect natively. Should coexist with gRPC.

**Unix socket + custom protocol** — for local plugins and tooling where gRPC overhead is unnecessary. Simple, fast, already part of the plan.

*Recommendation to investigate: `tonic` for gRPC, MCP SDK for agent protocol, Unix socket for local tooling. All three coexist.*

---

### Configuration Parsing

**`serde` + TOML (`toml` crate)** — human-readable, git-friendly, well-supported in Rust. Fits the philosophy. Strong first choice for `.lain/` files and user config.

**`mlua`** — Lua scripting for programmable configuration (WezTerm-style). Powerful for advanced users. Consider as an optional layer on top of TOML for users who want full scriptability.

**JSON Schema validation** — not a config format but a validation layer. All config files should have published JSON Schemas so MAGGI and external tools can validate and generate them programmatically. `lain schema` outputs these.

*Recommendation to investigate: TOML as the default format, JSON Schema for machine validation, Lua as an optional advanced layer.*

---

### Process Isolation (Agent Pods)

**Rootless Podman** — OCI containers without a daemon, without root. Strong security properties, well-supported on Linux. The recommended default for agent process isolation.

**Linux namespaces directly** — mount, network, PID namespaces without a full container runtime. More control, more implementation work. Consider for the lightweight isolation tiers where a full OCI container is overkill.

**macOS Sandbox (`sandbox-exec`)** — the macOS equivalent of seccomp/namespaces for process restriction. Required for macOS support. Investigate for parity with the Linux security model.

*Recommendation to investigate: Rootless Podman for full isolation, direct namespaces for lightweight tiers, macOS Sandbox for the macOS path.*

---

### Security Enforcement

**seccomp-BPF** — applied to agent processes at spawn time by lain-shell core. Immutable, inherited by child processes, kernel-enforced. The primary blocking mechanism. Not optional.

**auditd (Linux) / Endpoint Security Framework (macOS)** — for rich observation without the attack surface of full eBPF. The recommended observation layer.

**`aya`** (Rust eBPF framework) — for the advanced opt-in eBPF observability path. Investigate as a future feature, not as the default. Understand the privilege requirements and attack surface before committing.

*Recommendation to investigate: seccomp-BPF + auditd/ESF as the default security stack. eBPF as a future opt-in.*

---

### MAGGI Model Backend (Local)

**Ollama** — the dominant local model server, supports most quantized models, good Rust client support, zero-friction for users. Strong default for local MAGGI backend.

**llama.cpp server** — lower level, more control, excellent quantization support. Good alternative if Ollama proves limiting for specific model requirements.

**candle** (Hugging Face Rust ML framework) — run models directly in Rust without a separate server process. Interesting for tight integration but higher implementation complexity.

*Recommendation to investigate: Ollama as the default local backend. Design MAGGI's model interface as an abstract trait so backends are swappable.*

---

### MAGGI Agent Framework

**Direct tool-calling over the model API** — MAGGI's tools are the `lain` CLI commands, exposed as JSON Schema function definitions. The model calls them. Simple, explicit, no framework dependency. Recommended starting point.

**`rig` (Rust agent framework)** — Rust-native agent framework with tool-calling support. Worth evaluating for reducing boilerplate.

**Custom thin orchestration layer** — if the above prove insufficient for MAGGI's multi-tool, multi-step reasoning needs. Keep it thin — the power comes from the tools, not the framework.

*Recommendation to investigate: Direct tool-calling first. Framework only if the boilerplate becomes a real burden.*

---

### Vector Store (MAGGI's knowledge / RAG)

**`qdrant`** — fast, Rust-based vector database, can run embedded or as a separate process. Good fit for local-first knowledge indexing.

**`lancedb`** — embedded vector store, no separate process, good Rust support. Simpler operational model than qdrant.

**`tantivy`** — full-text search in Rust. Not a vector store, but relevant for the parts of MAGGI's knowledge retrieval that are keyword-based rather than semantic.

*Recommendation to investigate: `lancedb` for semantic search (embedded, simple), `tantivy` for keyword search. Qdrant as an option if lancedb proves limiting.*

---

### Memory Allocator

**`jemalloc`** — reduced fragmentation, better multi-threaded performance than the system allocator for long-running processes. Used by Firefox, Redis, and others. Strong candidate as the global allocator for the terminal core.

**`mimalloc`** — Microsoft's allocator, excellent benchmark numbers, good Rust integration. Worth comparing against jemalloc.

**System allocator** — the default. Use as the baseline for measurement. Replace if profiling shows meaningful wins elsewhere.

*Recommendation to investigate: Profile with system allocator first, then benchmark jemalloc and mimalloc. Set one at project start and measure.*

---

### Audit Log

**Append-only JSONL with hash chaining** — simple, human-readable, tamper-evident, inspectable with any text tool. Fits the philosophy. Implement from scratch — it is not complex.

**`sled`** (embedded key-value store) — if the audit log needs efficient querying (e.g., `lain postmortem --since 1h --severity critical`). Overkill if JSONL + grep is sufficient.

**SQLite via `rusqlite`** — structured query support for complex audit queries. More powerful than JSONL, less human-readable for direct inspection. Consider if query complexity grows.

*Recommendation to investigate: Append-only JSONL as the starting point. Add SQLite if query requirements grow beyond what simple filtering handles.*

---

### Dashboard and Overlay UI

**`ratatui`** — the dominant TUI framework in Rust. Rich widget set, good performance, active community. For the in-terminal overlay panels, agent status displays, and Mission Control views.

**Custom rendering over `wgpu`** — since lain-shell owns the GPU renderer, complex overlays (agent graphs, MOTOKO event feeds, cost charts) can be rendered as native GPU UI rather than TUI. More work, more capability.

**Web-based companion dashboard** — a separate local web UI served by lain-shell for complex visualizations that do not fit in a terminal overlay. Optional, ship later.

*Recommendation to investigate: `ratatui` for terminal overlays. Custom GPU rendering for complex visual elements. Web dashboard as a future addition.*

---

*End of suggested stack. These are starting points. Investigate, benchmark, and decide during implementation. The philosophy does not change based on which crates are chosen.*


---

## Bootstrap and Distribution Philosophy

### `lain-bootstrap` — The Thin Installer

lain-shell ships its own bootstrap layer. Not chezmoi, not a third-party installer, not a curl-pipe-bash script that does too much. A small, standalone, statically linked binary whose only job is to get lain-shell running on any machine and then hand off to MAGGI.

The bootstrap is intentionally dumb. It does the minimum:

1. Detect host OS, architecture, and available package managers
2. Pull the correct lain-shell binary and verify its signature
3. Set up the minimum environment lain-shell needs to start
4. Fire lain-shell and let MAGGI finish the rest

**MAGGI is the intelligence. The bootstrap is the bridge.** The bootstrap does not need to handle every edge case on every OS. It only needs to get far enough that MAGGI can take over. Different RHEL version with an old library? MAGGI negotiates with that machine. Cluster node with no GPU? MAGGI configures the CPU render path. Missing font? MAGGI installs one. The bootstrap stays small and auditable by design — the complexity lives in MAGGI, not in the installer.

This is a better model than chezmoi for lain-shell's purposes because chezmoi is a human-operated tool. lain-bootstrap fires MAGGI immediately and MAGGI becomes the agent that adapts to the specific machine. Per-machine configuration is an agent problem, not an installer problem.

### Two Parallel Paths — Neither Is Second-Class

**Path A — Universal:** `curl -fsSL install.lain.sh | sh`, `apt install lain-shell`, `brew install lain-shell`, or a direct signed binary download. Static binary, bootstrap fires, MAGGI handles the rest. Works on any machine, any OS, no prerequisites.

**Path B — Nix:** `nix profile install github:nerv/lain-shell` or a home-manager module. Fully declarative, reproducible, every dependency pinned. MAGGI still activates but has less work to do because Nix already resolved the environment correctly.

Both paths produce the same running lain-shell. The codebase only cares about producing a correct binary. Nix handles its own dependency resolution. The bootstrap handles everyone else. These are not competing paths — they are parallel options for different user preferences and operational environments.

The bootstrap is also the right place to detect the Nix case: if Nix is present on the machine, the bootstrap can offer to use the Nix path instead of the static binary path. The user chooses. MAGGI can later suggest the switch — "you are on NixOS, want me to configure the Nix package? You get atomic rollbacks and reproducible updates."

### Cluster Deployment

For a cluster with mixed OS (Ubuntu, RHEL, Arch, Alpine, or anything else), the recommended strategy is layered:

**Immediate:** Static binary + bootstrap. Works on every node. Deployed via any existing tooling — ansible, terraform, scp, whatever the cluster already uses. MAGGI handles per-node adaptation after first launch.

**Structured:** Proper package repo support (apt, dnf) for nodes that use package managers. The static binary install and the package manager install are the same binary — the package manager just handles updates and verification.

**Declarative:** Nix flake for cluster nodes that benefit from full reproducibility. If nodes are NixOS-managed or use home-manager, lain-shell becomes part of the declarative system config. Updates are atomic, rollbacks are instant.

The key architectural insight: **bootstrap gets you to a running terminal, MAGGI finishes the job**. The installer does not need to be smart. The agent is smart.

---

## Nix as a Security Layer

Nix's security properties are concrete and relevant to lain-shell specifically — not generic packaging benefits.

### Immutable content-addressed store

Packages in `/nix/store` are content-addressed and read-only. A path is derived from the hash of its contents. A compromised dependency cannot overwrite itself in place — the path changes if the content changes, which means the change is detectable. This is directly complementary to MOTOKO's supply chain threat model. Nix reduces the attack surface for dependency tampering at the installation level. MOTOKO catches it at runtime. Together they cover the full lifecycle.

### Reproducible builds

The same flake input always produces the same binary, verifiably. You can audit exactly what went into a build — every dependency, every version, every compile flag. This matters for security reviews, compliance requirements, and air-gapped environments where you need to be certain about what you are running.

### No implicit global state

Nix does not write to `/usr/lib` or `/usr/local`. Each package lives in isolation in the store. No accidental version conflicts, no dependency on something another package silently installed. The environment is exactly what is declared.

### Atomic rollback

If a lain-shell update introduces a problem, `nix profile rollback` is instant and total. The update contract in this document — user config untouched, rollback always available, migrations explicit — is how Nix works by default. On Nix, the contract is not a promise, it is a structural property.

### Sandboxed builds

Nix builds run in a network-isolated, filesystem-isolated sandbox. The build cannot phone home, cannot read secrets, cannot access anything outside its declared inputs. This is meaningful supply chain protection for the build process itself — relevant given the LiteLLM-class attacks that MOTOKO is designed to detect at runtime.

### The honest boundary

Nix protects the *installation and update path* — how lain-shell and its dependencies arrive on a machine. It does not replace MOTOKO, which protects *runtime behavior* — what agents do after lain-shell is running. These are complementary, not redundant. A lain-shell installation can have both Nix's installation guarantees and MOTOKO's runtime enforcement simultaneously. On a Nix-managed system, the combination covers the full threat surface from build to execution.

