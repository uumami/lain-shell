# ADR-001: Custom Multiplexer (Navi) Instead of tmux Substrate

## Status

Accepted

## Date

2026-04-03

## Context

The original seed document proposed using tmux control mode as the session engine, with lain-shell owning the UI and tmux owning session management. The reasoning was pragmatic: tmux is battle-tested, universally installed, and provides stable session semantics.

However, analysis revealed a fundamental mismatch between what lain-shell needs from a multiplexer and what tmux provides.

### What lain-shell needs

- **Typed session metadata** — agent identity, permission scopes, cost tracking, workspace association
- **Per-pane security context** — permission boundaries enforced at the OS level, per-pane
- **Agent lifecycle events** — structured, typed events for create, pause, resume, kill
- **Container-backed panes** — panes that run inside isolated pods, not just naked PTYs
- **Structured IPC** — typed communication between multiplexer and other quanta
- **Session-scoped audit trails** — per-session tamper-evident logs
- **First-class pane identity** — typed pane objects, not integer IDs with string metadata

### What tmux provides

- String-based key-value metadata
- No permission model
- No agent lifecycle concept
- No container integration
- Text-based control mode protocol (fragile for programmatic use — the seed document itself noted this)
- Integer pane IDs

### The contradiction

The seed document criticized tmux control mode as "text-based, fragile for programmatic use" and then recommended building on it. The wrapper required to bridge the gap between tmux's model and lain-shell's model would be thicker than the substrate itself.

## Decision

Build a custom multiplexer — **Navi** — purpose-built for lain-shell.

Navi inherits the best ideas from tmux and Zellij:
- Session semantics (attach/detach, session persistence)
- Keyboard-driven workflow
- Pane splits and layout management
- tmux-compatible keybindings (as an optional keymap)

But it builds on a foundation designed for agents:
- Typed session model with first-class agent identity
- Per-pane security contexts integrated with MOTOKO
- Container-backed pane support native to the architecture
- Structured event emission for all lifecycle operations
- Direct integration with Core's PTY management (no translation layer)

The hard parts of multiplexer engineering that tmux has solved over decades (PTY edge cases, terminal compatibility) are addressed by using `portable-pty` for PTY handling and `alacritty_terminal` for VTE parsing — these crates encode the same decades of knowledge.

## Consequences

### Enables

- Native agent-aware session management without translation overhead
- Direct integration with MOTOKO for per-pane security
- Container-backed panes as a first-class concept
- Typed, structured interfaces between Navi and other quanta
- Full control over session serialization and persistence
- No external dependency on tmux being installed

### Costs

- More implementation work than wrapping tmux (though likely comparable, since the wrapper would be substantial)
- No inherited tmux ecosystem (plugins, scripts that assume tmux)
- Must solve session persistence, attach/detach, and layout management from scratch
- Must handle edge cases that tmux handles through decades of patches

### Risks

- Unknown unknowns in multiplexer engineering — edge cases we haven't anticipated
- Users who expect tmux compatibility may be disappointed if the compatibility layer is incomplete

## Alternatives Considered

### tmux control mode substrate

Wrapping tmux via its control mode protocol. Rejected because the translation layer would be thicker than the substrate, and every lain-shell feature would be fighting tmux's model.

### Zellij as substrate

Zellij is written in Rust and has a WASM plugin architecture. However, it has less universal installation than tmux, and the same fundamental mismatch applies: Zellij's model is not agent-aware. Additionally, embedding Zellij as a library is not a supported use case.

### Fork tmux

Forking tmux and adding agent-awareness directly. Rejected because tmux is C, and the architectural changes needed go beyond what a fork could cleanly achieve. The session model would need to be fundamentally restructured.
