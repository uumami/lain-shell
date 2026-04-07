## ADDED Requirements

### Requirement: Cargo workspace with seven member crates
The project SHALL be organized as a Cargo workspace with exactly seven member crates: `lain-types`, `lain-core`, `lain-navi`, `lain-motoko`, `lain-maggi`, `lain-wired`, and the binary crate `lain-shell` (at workspace root `src/`).

#### Scenario: Workspace compiles with all members
- **WHEN** `cargo build` is run at the workspace root
- **THEN** all seven crates compile without errors

#### Scenario: Each quantum maps to one crate
- **WHEN** inspecting the crate layout
- **THEN** Core maps to `lain-core`, Navi to `lain-navi`, MOTOKO to `lain-motoko`, MAGGI to `lain-maggi`, THE WIRED to `lain-wired`

### Requirement: lain-types is the sole shared types crate
All cross-crate types (IDs, events, errors, command model) SHALL live in `lain-types`. Every quantum crate SHALL depend on `lain-types`. No quantum crate SHALL re-export or duplicate types that belong in `lain-types`.

#### Scenario: ID type used across crates
- **WHEN** `lain-navi` needs to reference a `SessionId`
- **THEN** it imports `lain_types::ids::SessionId`, not a locally defined copy

#### Scenario: No circular dependencies
- **WHEN** inspecting the dependency graph
- **THEN** no circular dependencies exist between any crates

### Requirement: lain-types uses internal module structure
`lain-types` SHALL organize types into modules: `ids`, `events`, `command`, `error`. Each module SHALL be independently extractable into its own crate if `lain-types` grows beyond ~2000 lines.

#### Scenario: Module structure exists
- **WHEN** inspecting `lain-types/src/`
- **THEN** files `ids.rs`, `events.rs`, `command.rs`, `error.rs` exist with `pub mod` declarations in `lib.rs`

### Requirement: lain-navi depends on lain-core
`lain-navi` SHALL depend on `lain-core` to access PTY, render, and isolation APIs. No other quantum crate SHALL depend on `lain-core` directly — they interact via trait objects composed in the binary.

#### Scenario: lain-wired does not depend on lain-core
- **WHEN** inspecting `lain-wired/Cargo.toml`
- **THEN** `lain-core` is not listed as a dependency

### Requirement: Integration tests in workspace root
Cross-crate integration tests SHALL live in a top-level `tests/` directory, separate from per-crate unit tests.

#### Scenario: Integration test directory exists
- **WHEN** inspecting the workspace root
- **THEN** a `tests/` directory exists for cross-crate tests
