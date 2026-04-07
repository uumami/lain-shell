## ADDED Requirements

### Requirement: MOTOKO is the single audit log writer
Only MOTOKO SHALL write to the audit log file. Other quanta SHALL emit mandatory events via the `AuditSink` trait, which routes to MOTOKO's writer.

#### Scenario: Two quanta emit events concurrently
- **WHEN** Navi and Core both emit mandatory events at the same time
- **THEN** MOTOKO serializes the writes, maintaining hash chain integrity, with no interleaved bytes

### Requirement: AuditSink trait in lain-types
An `AuditSink` trait SHALL exist in `lain-types/src/traits/audit.rs`. All quanta SHALL use this trait to emit mandatory events. The trait SHALL have a single method: `async fn emit(&self, event: MandatoryEvent) -> Result<()>`.

#### Scenario: Audit sink unavailable
- **WHEN** a quantum calls `audit_sink.emit()` and MOTOKO is unreachable
- **THEN** the call returns an error and the mutating operation fails (fail-closed)

### Requirement: Write-before-mutate ordering
Quanta SHALL audit the intent before performing the mutation. If the audit write fails, the mutation SHALL NOT occur. If the mutation fails after a successful audit, the audit log contains an unmatched intent (acceptable diagnostic information).

#### Scenario: Audit succeeds, mutation fails
- **WHEN** `audit_sink.emit(SessionCreate)` succeeds but `session_manager.create()` fails
- **THEN** the audit log shows an unmatched SessionCreate intent, and MOTOKO can detect this as a failed operation
