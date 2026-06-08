## Why

The lain-shell design docs are strong at the boundary level — five quanta with clear ownership, 10 accepted ADRs, trait interfaces defined. But five cross-cutting TODOs in `systems-design.md` and three correctness gaps identified by multiple independent reviews block initializing the Cargo workspace. Specifically: crate boundaries are undefined, the error strategy is unresolved, the event bus invariant (#8: "cannot drop mandatory events") is unenforceable with the chosen technology (`tokio::broadcast`), PTY ownership on crash is ambiguous, and the default isolation level (Level 1) conflates seccomp with path-level file policy. These must be resolved before writing code, because they shape every crate's public API surface.

## What Changes

- **Crate boundaries and repo structure**: Define the Cargo workspace layout — which crates exist, what each owns, where shared types live. Resolve the `lain-types` question (single shared crate with module structure vs. multiple micro-crates).
- **Error strategy**: Define the error type hierarchy across crate boundaries. All error types must round-trip through gRPC (serializable from day one), because dev-mode (in-process) and production-mode (separate processes) must use identical trait APIs.
- **Event delivery model**: Replace the single `tokio::broadcast` bus with a two-tier model — mandatory events as synchronous audit log writes (durable, fail-closed), observable events as best-effort broadcast (lossy, acceptable). Fix invariant #8 to be enforceable.
- **PTY and VTE state ownership**: Clarify that Core owns PTY file descriptors, VTE terminal state (`alacritty_terminal::Term`), and scrollback buffers. Navi stores `PtyId` (not `PtyHandle`), queries cells via `CorePtyApi`. Document crash semantics: Core death = PTYs die; Navi death = topology lost, PTYs survive.
- **Level 1 mount namespace specification**: Replace the misleading seccomp-blocks-file-paths diagram with an honest security model. Mount namespace controls what files are visible. Seccomp controls which syscall classes are available. Provide the exact mount table for Level 1.
- **Concurrency/transport discipline**: Establish that all trait API types must be serializable (`serde::Serialize + Deserialize`), no non-serializable handles cross API boundaries, and ordering assumptions are documented per-call.
- **Authority and override ladder**: Add a precedence section to `systems-architecture.md`: `runtime invariant > team policy > workspace policy > user preference`, with concrete examples resolving the customization-vs-security tension.
- **Hygiene fixes**: Fix `workspace.toml`→`config.toml` naming, duplicate question #13, resolved-but-unlisted questions in quanta philosophy files, legacy file deprecation notices, token issuer contradiction.

## Capabilities

### New Capabilities
- `crate-boundaries`: Cargo workspace layout, crate ownership map, shared types strategy, module structure for `lain-types`
- `error-strategy`: Cross-crate error hierarchy, gRPC round-trip requirement, `thiserror` vs `anyhow` decision, error conversion discipline
- `event-delivery`: Two-tier event model (mandatory audit writes + observable broadcast), delivery guarantees per tier, invariant #8 fix
- `pty-ownership`: Core owns PTY+VTE+scrollback, Navi holds PtyId, `CorePtyApi` with cell-grid query, crash semantics for each quantum
- `level1-mount-spec`: Exact mount namespace table, seccomp syscall class list, what is visible vs invisible, honest security escalation diagram
- `transport-discipline`: Serialization requirement for all API types, ordering guarantees per transport, dev-mode = "gRPC without the wire" principle

### Modified Capabilities

_(No existing specs to modify — this is the first OpenSpec change in this project.)_

## Impact

- **docs/systems-design.md**: Resolves all 5 TODOs (crate boundaries, error handling, logging sketch, testing sketch, repo structure)
- **docs/systems-architecture.md**: New sections for authority ladder and event delivery model. Fix invariant #8 wording. Fix security escalation diagram (seccomp/path conflation). Add override precedence.
- **docs/quanta/navi/systems-architecture.md**: Update `Pane` struct (`PtyId` not `PtyHandle`), document crash semantics
- **docs/quanta/core/systems-architecture.md**: Add `get_cells`/`get_scrollback` to `CorePtyApi`, document VTE state ownership
- **docs/decisions/009-isolation-levels.md**: Add Level 1 mount table, clarify seccomp role
- **docs/open-questions.md**: Resolve #31 (Level 1 mount construction), fix duplicate #13 numbering
- **docs/philosophy.md**: Fix `workspace.toml`→`config.toml`, add override ladder reference
- **Legacy files**: Add deprecation notices to all 3 files in `docs/legacy/`
- **Quanta philosophy files**: Mark resolved open questions in core, maggi, navi
