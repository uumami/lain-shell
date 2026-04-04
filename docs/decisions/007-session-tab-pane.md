# ADR-007: Session → Tab → Pane Hierarchy

## Status

Accepted

## Date

2026-04-04

## Context

Navi needs a hierarchy for organizing the user's terminal workspace. tmux uses Session → Window → Pane. The term "window" is ambiguous — it conflicts with the OS window concept.

## Decision

Use **Session → Tab → Pane** hierarchy.

- **Session**: persistent workspace, survives detach/reattach. Named. Contains tabs.
- **Tab**: full-screen view within a session. Switching tabs replaces the entire visible area. Named.
- **Pane**: split within a tab. Contains a PTY and optionally an agent pod.

This mirrors tmux's model exactly but uses modern terminology. "Tab" is universally understood from browsers and editors. Zellij uses "tab" for the same concept.

The tmux-compatible keymap maps tmux's "create window" to Navi's "create tab." The mapping is 1:1.

## Consequences

### Enables

- Clear, unambiguous terminology
- Familiar to users of modern terminals, browsers, editors
- Clean API: `navi session create`, `navi tab create`, `navi pane split`

### Costs

- tmux users must mentally translate "window" to "tab" (minor)

## Alternatives Considered

### tmux terminology (Session → Window → Pane)

Rejected. "Window" is ambiguous with OS windows.

### Custom hierarchy (Workspace → View → Split)

Considered. Rejected as unnecessarily unfamiliar. Tab and Pane are already industry standard.
