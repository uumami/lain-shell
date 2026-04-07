## ADDED Requirements

### Requirement: Two-tier event delivery model
The event system SHALL have two tiers: mandatory events (Tier 1) with durable, synchronous delivery, and observable events (Tier 2) with best-effort delivery via `tokio::broadcast`.

#### Scenario: Mandatory event is durable
- **WHEN** a session is created and the mandatory event `SessionCreated` is written
- **THEN** the event is persisted to the audit log (fsync'd) before `create_session` returns `Ok`

#### Scenario: Observable event may be dropped
- **WHEN** a `tokio::broadcast` receiver lags behind
- **THEN** older observable events are dropped for that receiver, and a warning is logged

### Requirement: Mandatory events are audit log writes
Mandatory events SHALL be appended to MOTOKO's audit log as the primary delivery mechanism. The audit log write MUST complete successfully before the mutating operation returns success. If the audit log write fails, the mutating operation SHALL fail or enter a fail-closed state.

#### Scenario: Audit write failure causes operation failure
- **WHEN** the audit log is unreachable (disk full, permission error) and a session creation is attempted
- **THEN** `create_session` returns an error and no session is created

#### Scenario: MOTOKO reads mandatory events from audit log
- **WHEN** MOTOKO starts or recovers from a restart
- **THEN** it can read all mandatory events from the audit log, including events emitted while it was down

### Requirement: Mandatory events are also bridged to broadcast
Every mandatory event SHALL also be sent to the `tokio::broadcast` bus for convenience of non-security consumers (MAGGI, UI, plugins). The broadcast copy is NOT the source of truth — consumers that require completeness MUST read the audit log.

#### Scenario: MAGGI receives lifecycle events via broadcast
- **WHEN** a session is created
- **THEN** MAGGI receives `SessionCreated` on the broadcast bus (best-effort)

#### Scenario: Plugin misses event due to lag
- **WHEN** a plugin's broadcast receiver is lagging and a mandatory event is bridged
- **THEN** the plugin may miss the event, but the audit log still has it

### Requirement: Invariant #8 reflects two-tier model
System invariant #8 SHALL be reworded to: "Mandatory events are durably written to the audit log before the mutating operation completes. The audit log is the source of truth for security and lifecycle events. The observable event bus is best-effort and may drop events for lagging receivers."

#### Scenario: Invariant wording is updated
- **WHEN** reading `systems-architecture.md` invariant #8
- **THEN** it describes the two-tier model, not "the event bus cannot drop mandatory events"
