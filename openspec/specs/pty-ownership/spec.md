## MODIFIED Requirements

### Requirement: Navi output broadcast reflects Core-owned VTE
The multi-client output broadcast section in `quanta/navi/systems-architecture.md` SHALL state that Core updates the cell grid and emits cell diffs. Navi SHALL route diffs to clients based on view state. Navi SHALL NOT parse VTE or update cell grids.

#### Scenario: Output broadcast description
- **WHEN** reading the multi-client output broadcast section
- **THEN** it says Core feeds PTY bytes into Term, emits cell diffs, and Navi routes diffs to clients — not "VTE parser updates the shared cell grid"

## ADDED Requirements (from vertical-spike-pty-rendering)

### Requirement: Core owns PTY fd and Term via alacritty_terminal
lain-core SHALL own the PTY file descriptor (via `alacritty_terminal::tty::new()`) and the terminal state (via `alacritty_terminal::Term`). The `Term` SHALL be wrapped in `Arc<FairMutex<Term>>` for safe access from the render thread.

#### Scenario: Term is accessible for rendering
- **WHEN** the render loop needs cell data
- **THEN** it locks the `Arc<FairMutex<Term>>` and calls `renderable_content()` to get an iterator over visible cells

#### Scenario: PTY write handle is accessible for input
- **WHEN** keyboard input arrives from winit
- **THEN** bytes are sent to the PTY via alacritty_terminal's `EventLoopSender`
