## Context

Three issues found in post-review of the pre-cargo-init-design change. All are correctness fixes to existing design docs.

## Goals / Non-Goals

**Goals:**
- Fix audit log concurrency: MOTOKO single writer, AuditSink trait, write-before-mutate
- Fix Navi-Core coupling: traits in lain-types, no inter-quantum crate dependencies
- Fix stale VTE text in Navi output broadcast section

**Non-Goals:**
- Full audit log format design (hash chain specifics, rotation, etc.)
- Changing the seven-crate count
- Any Rust code

## Decisions

### Decision 1: AuditSink trait — MOTOKO is single writer

MOTOKO owns the audit log file. Other quanta emit mandatory events via an `AuditSink` trait. MOTOKO serializes writes, maintains the hash chain, and fsyncs.

```rust
// In lain-types/src/traits/audit.rs
#[async_trait]
trait AuditSink: Send + Sync {
    async fn emit(&self, event: MandatoryEvent) -> Result<()>;
}
```

In dev mode: `AuditSink` is an in-process call to MOTOKO's writer (serialized via mutex or mpsc). In production: RPC to MOTOKO's process over Unix socket.

**Write-before-mutate ordering:**

```rust
async fn create_session(&self, params: SessionParams) -> Result<SessionId> {
    // 1. Audit the intent — if this fails, nothing happens
    self.audit_sink.emit(MandatoryEvent::SessionCreate {
        params: params.clone(),
    }).await?;

    // 2. Mutate — audit already recorded the intent
    let id = self.session_manager.create(params.clone()).await?;

    // 3. Observable: best-effort
    let _ = self.bus.send(Event::SessionCreated { id, params });

    Ok(id)
}
```

If mutation fails after audit, MOTOKO sees an unmatched intent — useful diagnostic, not a bug. If audit fails, mutation never happens — fail-closed.

### Decision 2: All inter-quantum traits in lain-types

Move all API trait definitions into `lain-types/src/traits/`. This eliminates the `lain-navi → lain-core` dependency.

Updated lain-types module structure:
```
lain-types/src/
├── lib.rs
├── ids.rs
├── events.rs
├── command.rs
├── error.rs
└── traits/
    ├── mod.rs
    ├── core.rs      CorePtyApi, CoreRenderApi, CoreIsolationApi, CoreConfigApi, CoreBlockApi, CoreRouterApi
    ├── navi.rs       NaviApi
    ├── motoko.rs     MotokoApi
    ├── maggi.rs      MaggiApi
    ├── wired.rs      WiredApi
    └── audit.rs      AuditSink
```

Updated dependency graph:
```
lain-types ←── lain-core
     ↑
     ├── lain-navi     (NO dependency on lain-core)
     ├── lain-motoko
     ├── lain-maggi
     └── lain-wired
              │
       lain-shell (binary, depends on all, wires via Arc<dyn Trait>)
```

No inter-quantum dependencies. Each quantum implements traits from lain-types. The binary composes them.

### Decision 3: Fix Navi output broadcast

Replace "VTE parser updates the shared cell grid" with the correct flow: Core owns VTE parsing and cell grid, emits cell diffs; Navi routes diffs to clients based on view state.

## Risks / Trade-offs

**[lain-types grows larger with traits] → Acceptable.** Traits are zero-cost (no heavy dependencies). The traits module adds ~200 lines of trait definitions. If lain-types needs splitting later, the traits/ module extracts cleanly.

**[AuditSink adds latency to every mutation] → Acceptable.** In dev mode it's an in-process call (~microseconds). In production it's a Unix socket RPC (~100μs). Mutations (session create, pane split) happen at human speed, not hot-path speed.
