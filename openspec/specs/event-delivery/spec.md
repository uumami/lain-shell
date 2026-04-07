## MODIFIED Requirements

### Requirement: Mandatory events use write-before-mutate ordering
The event emission pattern SHALL audit the intent before mutating state. The code sketch in `systems-architecture.md` SHALL show `audit_sink.emit()` before the mutation, not after.

#### Scenario: Emission order in code sketch
- **WHEN** reading the event emission code sketch in `systems-architecture.md`
- **THEN** it shows `audit_sink.emit(intent).await?` BEFORE `session_manager.create()`, not after
