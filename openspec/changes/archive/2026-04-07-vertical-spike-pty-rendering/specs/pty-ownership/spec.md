## ADDED Requirements

### Requirement: Core owns PTY fd and Term via alacritty_terminal
lain-core SHALL own the PTY file descriptor (via portable-pty) and the terminal state (via `alacritty_terminal::Term`). The `Term` SHALL be wrapped in `Arc<FairMutex<Term>>` for safe access from the render thread.

#### Scenario: Term is accessible for rendering
- **WHEN** the render loop needs cell data
- **THEN** it locks the `Arc<FairMutex<Term>>` and calls `renderable_content()` to get an iterator over visible cells

#### Scenario: PTY write handle is accessible for input
- **WHEN** keyboard input arrives from winit
- **THEN** bytes are sent to the PTY via alacritty_terminal's `EventLoopSender`
