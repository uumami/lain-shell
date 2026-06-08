## Why

Post-review of the `pre-cargo-init-design` change identified three issues: (1) the audit log write model assumes multiple processes can append to a hash-chained file without coordination — they can't, (2) `lain-navi` depending on `lain-core` forces Navi to compile wgpu/alacritty_terminal/portable-pty just to reference API traits, and (3) Navi's multi-client output broadcast section still says "VTE parser updates the shared cell grid" which contradicts the new Core-owned VTE state design.

## What Changes

- **AuditSink trait**: MOTOKO is the single audit log writer. Other quanta emit mandatory events via an `AuditSink` trait (direct call in dev mode, RPC in production). Write-before-mutate ordering: audit the intent first, then mutate.
- **Trait definitions in lain-types**: Move all inter-quantum API traits (`CorePtyApi`, `NaviApi`, `MotokoApi`, `MaggiApi`, `WiredApi`) into `lain-types/src/traits/`. No quantum crate depends on another. Remove the `lain-navi → lain-core` dependency.
- **Fix Navi output broadcast text**: Rewrite to show Core updating the cell grid and emitting diffs, with Navi routing to clients.

## Capabilities

### New Capabilities
- `audit-sink`: AuditSink trait for single-writer audit log, write-before-mutate ordering, MOTOKO ownership

### Modified Capabilities
- `crate-boundaries`: Remove lain-navi → lain-core dependency, add traits module to lain-types
- `event-delivery`: Update emission pattern from mutate-then-audit to audit-then-mutate
- `pty-ownership`: Fix stale Navi text that contradicts Core-owned VTE state

## Impact

- **docs/systems-architecture.md**: Update event emission code sketch, add AuditSink description
- **docs/systems-design.md**: Update dependency graph (remove navi→core edge), add traits module to lain-types structure
- **docs/quanta/navi/systems-architecture.md**: Rewrite multi-client output broadcast section
- **docs/quanta/motoko/systems-architecture.md**: Document MOTOKO as single audit writer
