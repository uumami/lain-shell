# Navi — Philosophy

> *The multiplexer and session manager. Your interface to everything.*

---

## Purpose

Navi is the multiplexer for the agentic age. It manages sessions, panes, windows, layouts, attach/detach, and session persistence — not as generic PTY containers, but as **typed entities** that understand agent identity, permission scopes, cost tracking, and isolated processes (configurable per-pane isolation levels).

Named after the personal computers in Serial Experiments Lain. The Navi is the interface through which Lain connects to the Wired, manages her windows, and gains power over her environment.

---

## Why Build Custom

tmux and Zellij are excellent for human-only terminal multiplexing. Navi exists because lain-shell needs a multiplexer that understands:

- **Agent identity** — which agent owns which pane, what model powers it, what role it plays
- **Permission scopes** — per-pane, per-session boundaries enforced at the OS level
- **Isolation backing** — panes that run inside isolated environments (namespace, container, or air-gapped), not just naked PTYs
- **Cost tracking** — per-session token and resource accounting
- **Structured lifecycle events** — typed events for session create, pause, resume, kill, attach, detach
- **Session metadata** — typed, queryable, not just string key-value pairs

None of these can be cleanly bolted onto tmux. The wrapper would be thicker than the substrate. So Navi is built from scratch, inheriting the best ideas (session semantics, keyboard-driven workflow, attach/detach) but on a foundation designed for agents from the start.

See ADR-001 for the full decision rationale.

---

## Principles

### Sessions are typed, not generic

A Navi session is not just "a collection of panes." It carries identity: what agent is active, what permissions apply, what workspace it belongs to, what its cost footprint is. This metadata is first-class, queryable, and part of the session contract.

### Attach/detach is fundamental

Like tmux, Navi sessions survive terminal disconnection. You can detach, close the window, SSH back in, and reattach. This is not optional — it is a core feature that makes the terminal a persistent workspace.

### Keyboard-driven by default

Navi must be fully operable from the keyboard. Mouse support is additive, not required. The default keybindings should feel natural to tmux users while being discoverable to new users.

> **Idea:** Ship a "tmux compatibility" keymap that maps tmux's prefix-based shortcuts to Navi's equivalents. Muscle memory is valuable.

> **Idea:** Also consider a "Zellij-style" discoverable mode where available keybindings are shown in a status bar until the user disables the training wheels.

### Navi does not reason

Like Core, Navi is deterministic. It manages sessions and panes. It does not invoke models. MAGGI may tell Navi what to do ("open a new session with this layout"), but Navi executes — it does not decide.

### Panes can be heterogeneous

A single Navi session can contain:
- A plain shell pane (bash, zsh, fish)
- An agent pane (Claude Code, Codex, etc.)
- An isolated pane (namespace, container, or air-gapped — per ADR-009)
- A read-only pane (log viewer, status monitor)

Each pane type has different security properties, lifecycle behavior, and metadata. Navi handles all of them.

---

## What Navi Owns

- Session lifecycle (create, destroy, persist, restore)
- Pane management (split, close, resize, navigate, zoom)
- Window/tab management
- Layout engine (splits, arrangement, templates)
- Attach/detach
- Session persistence and serialization
- Keybinding dispatch for multiplexer operations
- Session templates and workspace profiles
- Session metadata and queryable state

---

## What Navi Does Not Own

- PTY creation and I/O (Core)
- Rendering (Core — Navi tells Core what to render where)
- Security enforcement (MOTOKO)
- Agent intelligence (MAGGI)
- External connectivity (THE WIRED)

---

## Open Questions

1. **Session persistence format.** How are sessions serialized? What is persisted (layout, working directories, environment) vs. reconstructed?
2. **Agent session lifecycle.** When a pane runs an agent, how does Navi interact with MOTOKO for security context? How does "pause agent" work at the Navi level?
3. ~~**Container-backed panes.**~~ ✓ Resolved. Generalized to isolation levels (ADR-009). Navi requests isolation from Core's Isolation Manager. Level 0-3 spectrum.
4. **Layout engine.** Binary tree splits (like tmux)? Something more flexible? How do saved layouts interact with session templates?
5. ~~**Multi-window model.**~~ ✓ Resolved. Session → Tab → Pane hierarchy (ADR-007). Tabs replace tmux's "windows" to avoid confusion with OS windows. See `quanta/navi/systems-architecture.md`.

---

## Ideas

1. **Session templates.** Named, shareable configurations that define a workspace: specific panes, agents, permissions, layout. `navi apply dev-backend` sets up your whole workspace.
2. **Session groups.** Group related sessions (e.g., "all sessions for project X") for batch operations.
3. **Session sharing.** Multiple users attached to the same session with permission-scoped views. One user sees everything; another sees only their panes.
4. **Pane linking.** Output from one pane feeds as input to another. Structured pipes between agent panes.
