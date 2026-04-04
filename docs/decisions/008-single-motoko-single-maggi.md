# ADR-008: Single MOTOKO Process, Single MAGGI Process

## Status

Accepted

## Date

2026-04-04

## Context

As lain-shell supports multiple sessions, a question arises: should MOTOKO and MAGGI run as one instance or one-per-session?

## Decision

**Single MOTOKO process** with internal per-session isolation.
**Single MAGGI process** with per-session conversation contexts.

### MOTOKO: single process

- **Cross-session correlation** is a key capability. MOTOKO must see all sessions simultaneously to detect patterns spanning multiple agents. Per-session MOTOKO makes this hard.
- **Tier 1 enforcement (seccomp, namespaces) is kernel-enforced** and survives MOTOKO process crashes. The blast radius of a MOTOKO crash is losing observation, not losing enforcement.
- **Internal panic isolation** via `catch_unwind` at session analysis boundaries. If one session's statistical analysis panics, other sessions are unaffected.
- **Efficiency**: one policy compilation, one ordered audit log, instead of N duplicates.

### MAGGI: single process

- **MAGGI is one operator.** The user talks to one MAGGI. Cross-session operations ("move this pane to session 2") require knowing both sessions.
- **Shared knowledge** (platform docs, config schemas) is the same across sessions. No need to duplicate.
- **Per-session conversation contexts** provide isolation without separate processes. Each session has its own message history, tool scope, and scratchpad.
- **Cross-session access is explicit** — MAGGI reaches for other sessions' context via tool calls, never automatically.
- **Cost**: one model connection instead of N.

### Memory scoping (four-dimensional)

Both MOTOKO and MAGGI use dimensional scoping for their internal data:
- `user_id` — who
- `session_id` — which session
- `agent_id` — which agent
- `scope` — pane / session / global

This provides logical isolation within a single process.

## Consequences

### Enables

- Cross-session security correlation (MOTOKO)
- Cross-session coordination (MAGGI)
- Efficient resource usage (single process each)
- Clean memory isolation without process overhead

### Costs

- A MOTOKO process crash affects all sessions' observation (but not Tier 1 enforcement)
- A MAGGI process crash affects all sessions' agent access (but MAGGI is optional)
- Internal robustness (panic isolation) must be carefully implemented

### Risks

- If MOTOKO's core event loop has a bug, all observation dies simultaneously
- Mitigation: Tier 1 is kernel-enforced, supervisor pauses agents, MOTOKO restarts

## Alternatives Considered

### MOTOKO per session

More blast radius isolation. Rejected because cross-session correlation is lost, N processes add overhead, and Tier 1 already survives crashes.

### MAGGI per session

Independent agent contexts. Rejected because the user expects one operator, shared knowledge would be duplicated, and cross-session operations become impossible.

### Coordinator + per-session workers

A meta-MOTOKO or meta-MAGGI coordinating per-session workers. Adds complexity without clear benefit over single-process with internal isolation.
