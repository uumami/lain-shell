## ADDED Requirements

### Requirement: Concrete Cargo.toml files validate dependency graph
Each quantum crate's `Cargo.toml` SHALL list only `lain-types` as an internal dependency. No quantum crate SHALL depend on any other quantum crate. The binary crate SHALL depend on all quantum crates.

#### Scenario: lain-core depends only on lain-types internally
- **WHEN** inspecting `crates/lain-core/Cargo.toml`
- **THEN** `lain-types` is the only workspace dependency listed; no other `lain-*` crate appears

#### Scenario: cargo check validates no circular dependencies
- **WHEN** running `cargo check` at the workspace root
- **THEN** the build succeeds with no circular dependency errors
