## ADDED Requirements

### Requirement: All cross-boundary types are serializable
Every type that appears in a public trait method signature (parameters or return types) SHALL derive `serde::Serialize` and `serde::Deserialize`. This ensures dev-mode (in-process) and production-mode (gRPC) use identical APIs.

#### Scenario: ID type is serializable
- **WHEN** `SessionId` is serialized to JSON
- **THEN** it produces a valid JSON value (integer or string UUID) that deserializes back identically

#### Scenario: Non-serializable handle does not cross boundary
- **WHEN** inspecting any public trait in `lain-core`, `lain-navi`, etc.
- **THEN** no method parameter or return type contains a raw file descriptor, channel sender, or OS handle

### Requirement: IDs are newtypes over serializable primitives
All identifier types (`SessionId`, `PaneId`, `TabId`, `PtyId`, `AttachId`, `IsolationHandle`) SHALL be newtypes over `u64` or `uuid::Uuid`. They SHALL derive `Serialize`, `Deserialize`, `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`.

#### Scenario: PtyId is a newtype
- **WHEN** inspecting `lain-types/src/ids.rs`
- **THEN** `PtyId` is defined as `pub struct PtyId(pub u64)` or similar newtype wrapper

#### Scenario: IsolationHandle is serializable
- **WHEN** Navi persists an `IsolationHandle` to TOML
- **THEN** it serializes as a plain value, not an OS-level process or namespace handle

### Requirement: Ordering assumptions are documented
Every public trait method that has ordering dependencies SHALL document them. Methods safe to call concurrently SHALL be marked as such. Methods that require sequencing SHALL state the precondition.

#### Scenario: create_pane documents ordering
- **WHEN** reading the doc comment for `NaviApi::create_pane`
- **THEN** it states that the session and tab must exist (precondition), and that concurrent `create_pane` calls within the same tab are safe

#### Scenario: destroy_session documents cascading
- **WHEN** reading the doc comment for `NaviApi::destroy_session`
- **THEN** it states that all tabs and panes within the session are destroyed, and that concurrent operations on those panes may fail

### Requirement: Dev mode is gRPC-without-the-wire
The in-process (dev mode) implementation of all quantum traits SHALL behave identically to the gRPC implementation in terms of API contract: same types, same error variants, same documented ordering constraints. The only difference SHALL be transport (direct function call vs. serialized message).

#### Scenario: Error from dev mode matches gRPC error
- **WHEN** `NaviApi::create_session` fails in dev mode with `LainError::NotFound`
- **THEN** the same call in production mode over gRPC also returns `LainError::NotFound` with the same fields
