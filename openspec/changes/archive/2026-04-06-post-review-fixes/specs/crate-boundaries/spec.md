## MODIFIED Requirements

### Requirement: lain-types uses internal module structure
`lain-types` SHALL organize types into modules: `ids`, `events`, `command`, `error`, `traits`. The `traits` module SHALL contain all inter-quantum API trait definitions (`CorePtyApi`, `NaviApi`, `MotokoApi`, `MaggiApi`, `WiredApi`, `AuditSink`). Each module SHALL be independently extractable into its own crate.

#### Scenario: Module structure exists
- **WHEN** inspecting `lain-types/src/`
- **THEN** files `ids.rs`, `events.rs`, `command.rs`, `error.rs` exist, and a `traits/` directory exists with `core.rs`, `navi.rs`, `motoko.rs`, `maggi.rs`, `wired.rs`, `audit.rs`

### Requirement: No inter-quantum crate dependencies
No quantum crate SHALL depend on any other quantum crate. All quantum crates SHALL depend only on `lain-types`. The binary crate (`lain-shell`) SHALL compose all quantum implementations via `Arc<dyn Trait>`.

#### Scenario: lain-navi does not depend on lain-core
- **WHEN** inspecting `lain-navi/Cargo.toml`
- **THEN** `lain-core` is NOT listed as a dependency

#### Scenario: lain-navi uses CorePtyApi
- **WHEN** Navi needs to call `CorePtyApi::get_cells`
- **THEN** it imports the trait from `lain_types::traits::core::CorePtyApi`, not from `lain_core`
