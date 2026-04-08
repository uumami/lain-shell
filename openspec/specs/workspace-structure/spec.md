## ADDED Requirements

### Requirement: Cargo workspace with 7 member crates
The repository SHALL have a root `Cargo.toml` defining a Cargo workspace with members: `crates/lain-types`, `crates/lain-core`, `crates/lain-navi`, `crates/lain-motoko`, `crates/lain-maggi`, `crates/lain-wired`, and the binary at the workspace root.

#### Scenario: Workspace compiles clean
- **WHEN** running `cargo check` at the workspace root
- **THEN** all 7 crates compile without errors

#### Scenario: Binary crate depends on all quantum crates
- **WHEN** inspecting the root `Cargo.toml` (binary)
- **THEN** it lists lain-types, lain-core, lain-navi, lain-motoko, lain-maggi, and lain-wired as dependencies

### Requirement: Skeleton crates have minimal lib.rs
Each skeleton quantum crate (lain-navi, lain-motoko, lain-maggi, lain-wired) SHALL have a `Cargo.toml` that depends only on `lain-types` and a `src/lib.rs` that compiles.

#### Scenario: Skeleton crate compiles independently
- **WHEN** running `cargo check -p lain-navi`
- **THEN** it compiles without errors and produces no warnings about unused dependencies

### Requirement: lain-types contains minimal shared types
For the spike, `lain-types` SHALL define at minimum: `PtyId` (newtype over u64), `LainError` enum with serializable variants, and the `error` module. Full trait definitions (CorePtyApi, NaviApi, etc.) are deferred to post-spike.

#### Scenario: PtyId is serializable
- **WHEN** serializing a `PtyId` to JSON and deserializing it back
- **THEN** the round-trip produces an identical value

#### Scenario: LainError is serializable
- **WHEN** serializing any `LainError` variant to JSON and deserializing it back
- **THEN** the round-trip produces an identical error with preserved variant and context
