## 1. Track A — Crate Boundaries and Repo Structure

- [x] 1.1 Update `docs/systems-design.md`: replace the crate boundaries TODO with the decided 7-crate workspace layout (lain-types, lain-core, lain-navi, lain-motoko, lain-maggi, lain-wired, lain-shell binary)
- [x] 1.2 Update `docs/systems-design.md`: replace the repository structure TODO with the concrete directory tree from the design doc
- [x] 1.3 Update `docs/systems-design.md`: add the crate dependency graph (lain-types ← all; lain-core ← lain-navi; lain-shell composes all)
- [x] 1.4 Update `docs/systems-design.md`: note that lain-types uses internal module structure (ids.rs, events.rs, command.rs, error.rs) for future extractability

## 2. Track A — Error Strategy

- [x] 2.1 Update `docs/systems-design.md`: replace the error handling TODO with the decided strategy (thiserror everywhere, LainError in lain-types, no anyhow in libraries)
- [x] 2.2 Add LainError enum sketch to `docs/systems-design.md` with variants: NotFound, PermissionDenied, InvalidConfig, IsolationError, PtyError, BusError, Internal
- [x] 2.3 Document the serialization requirement: LainError derives Serialize + Deserialize for gRPC round-trip
- [x] 2.4 Document the boundary rule: internal errors (io::Error, wgpu::Error) convert to LainError via From at crate boundary

## 3. Track B — Event Delivery Model

- [x] 3.1 Add "Event Delivery Model" section to `docs/systems-architecture.md` describing the two-tier model: mandatory (audit log write) and observable (tokio::broadcast)
- [x] 3.2 Update the event emission code sketch in `docs/systems-architecture.md` (~line 107) to show `audit.write(...).await?` for mandatory events
- [x] 3.3 Rewrite invariant #8 in `docs/systems-architecture.md` to reflect the two-tier model: "Mandatory events are durably written to the audit log before the mutating operation completes."
- [x] 3.4 Update the deployment modes section (~line 523) to clarify that tokio::broadcast is for observable events only, not for mandatory events

## 4. Track B — PTY and VTE State Ownership

- [x] 4.1 Update `docs/quanta/core/systems-architecture.md`: expand CorePtyApi trait to include get_cells, get_scrollback, get_cursor, subscribe_cell_changes
- [x] 4.2 Update `docs/quanta/core/systems-architecture.md`: add a section explicitly stating Core owns PTY fd + alacritty_terminal::Term + scrollback buffer
- [x] 4.3 Update `docs/quanta/navi/systems-architecture.md`: change Pane struct to use PtyId instead of PtyHandle, add doc comment explaining why
- [x] 4.4 Add crash semantics table to `docs/systems-architecture.md`: what survives when each quantum dies (Core death, Navi death, MOTOKO death, MAGGI death)
- [x] 4.5 Update `docs/quanta/navi/systems-architecture.md`: add Navi recovery flow — on restart, read persisted TOML, query Core for live PTYs, reconcile

## 5. Track B — Level 1 Mount Namespace Specification

- [x] 5.1 Add Level 1 mount table to `docs/decisions/009-isolation-levels.md`: exact paths, sources, and modes for everything mounted at Level 1
- [x] 5.2 Add Level 1 seccomp class list to `docs/decisions/009-isolation-levels.md`: blocked syscalls (ptrace, mount, setns, bpf, etc.) and allowed classes (file I/O, process, network, memory)
- [x] 5.3 Fix security escalation diagram in `docs/systems-architecture.md` (~line 648): replace "seccomp blocks the read syscall" with "mount namespace: path not mounted → ENOENT"
- [x] 5.4 Add "configurable per workspace" section to ADR-009: extra_mounts in .lain/permissions.toml, read from safe mirror
- [x] 5.5 Update `docs/open-questions.md`: mark question #31 (Level 1 mount construction) as resolved with pointer to ADR-009

## 6. Transport Discipline and Authority Ladder

- [x] 6.1 Add "Transport Discipline" section to `docs/systems-design.md`: all cross-boundary types derive Serialize + Deserialize, IDs are newtypes, no OS handles in public APIs
- [x] 6.2 Add ordering documentation rule to `docs/systems-design.md`: trait methods document concurrency/sequencing assumptions
- [x] 6.3 Add "Authority and Override Ladder" section to `docs/systems-architecture.md`: runtime invariant > team policy > workspace policy > user preference, with concrete examples table
- [x] 6.4 Add a reference to the override ladder in `docs/philosophy.md` principle #7 (extreme customization): "See Authority Ladder in systems-architecture.md for how customization interacts with security invariants"

## 7. Logging, Testing, and Remaining TODOs

- [x] 7.1 Update `docs/systems-design.md`: replace logging/tracing TODO with decided strategy (tracing crate, separate from MOTOKO audit, log levels defined)
- [x] 7.2 Update `docs/systems-design.md`: replace testing strategy TODO with decided approach (unit per crate, integration in tests/, proptest for VTE, security tests first-class, criterion benchmarks)

## 8. Hygiene Fixes

- [x] 8.1 Fix `docs/philosophy.md`: rename `workspace.toml` to `config.toml` to match systems-architecture.md and ADR-010
- [x] 8.2 Fix `docs/open-questions.md`: renumber duplicate question #13 (MOTOKO section should start at 14)
- [x] 8.3 Fix `docs/quanta/core/philosophy.md`: mark open question 3 (event bus design) as resolved with pointer to ADR-006
- [x] 8.4 Fix `docs/quanta/maggi/philosophy.md`: mark open questions 3, 4, 5 as resolved with pointers
- [x] 8.5 Fix `docs/quanta/navi/philosophy.md`: mark open question 5 (hierarchy) as resolved with pointer to ADR-007
- [x] 8.6 Fix `docs/quanta/the-wired/systems-design.md`: correct token issuer from "Navi" to "THE WIRED's auth engine"
- [x] 8.7 Add deprecation notice header to all three `docs/legacy/` files: "This document predates ADR-001 and ADR-004. See docs/ for current design."
- [x] 8.8 Delete `docs/legacy/lain-shell-philosophy-seed.md` (byte-identical duplicate of lain-shell-philosophy-original.md)
