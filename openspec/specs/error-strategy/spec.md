## ADDED Requirements

### Requirement: LainError is the cross-boundary error type
All public trait methods that cross crate boundaries SHALL return `Result<T, LainError>` where `LainError` is defined in `lain-types/src/error.rs`.

#### Scenario: Trait method returns LainError
- **WHEN** `NaviApi::create_session` is called and fails
- **THEN** the error type is `LainError`, not a Navi-internal error type

#### Scenario: Internal errors are converted at boundary
- **WHEN** a `std::io::Error` occurs inside `lain-core` during PTY creation
- **THEN** it is converted to `LainError::PtyError` before crossing the crate boundary

### Requirement: LainError is serializable
`LainError` SHALL derive `serde::Serialize` and `serde::Deserialize`. It MUST round-trip through JSON without information loss.

#### Scenario: Error round-trips through JSON
- **WHEN** a `LainError::NotFound { resource: "session", id: "abc" }` is serialized to JSON and deserialized back
- **THEN** the resulting error is identical to the original

#### Scenario: Error maps to gRPC status
- **WHEN** a `LainError` is returned over gRPC transport
- **THEN** it maps to an appropriate gRPC status code (NotFound → NOT_FOUND, PermissionDenied → PERMISSION_DENIED, Internal → INTERNAL)

### Requirement: thiserror for all error definitions
All error enums SHALL use `thiserror::Error` derive macro. `anyhow` SHALL NOT be used in library crates. `anyhow` is permitted only in the binary entry point (`src/main.rs`).

#### Scenario: Library crate does not use anyhow
- **WHEN** inspecting `lain-navi/Cargo.toml`
- **THEN** `anyhow` is not listed as a dependency

### Requirement: Errors carry actionable context
Every `LainError` variant SHALL include enough context to be actionable without a stack trace: at minimum, a human-readable message identifying the resource, operation, or reason for failure.

#### Scenario: Error message identifies the resource
- **WHEN** a session with ID "work" is not found
- **THEN** the error message contains both "session" and "work"
