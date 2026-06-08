## 1. AuditSink — single writer

- [x] 1.1 Add AuditSink trait description to `docs/systems-architecture.md` in the event delivery model section: MOTOKO is single writer, other quanta call AuditSink
- [x] 1.2 Update event emission code sketch in `docs/systems-architecture.md`: change to write-before-mutate ordering (audit_sink.emit before session_manager.create)
- [x] 1.3 Update `docs/quanta/motoko/systems-architecture.md`: document MOTOKO as single audit log writer, owns hash chain
- [x] 1.4 Add AuditSink to the lain-types traits module listing in `docs/systems-design.md`

## 2. Trait definitions in lain-types

- [x] 2.1 Update `docs/systems-design.md`: add `traits/` module to lain-types structure (core.rs, navi.rs, motoko.rs, maggi.rs, wired.rs, audit.rs)
- [x] 2.2 Update `docs/systems-design.md`: remove `lain-navi → lain-core` dependency from the dependency graph and rules
- [x] 2.3 Update `docs/systems-design.md`: update lain-navi row in crate table to remove "Depends on lain-core"

## 3. Fix stale Navi VTE text

- [x] 3.1 Rewrite multi-client output broadcast section in `docs/quanta/navi/systems-architecture.md`: Core updates cell grid and emits diffs, Navi routes to clients
