## ADDED Requirements

### Requirement: PTY creation and shell spawn
lain-core SHALL create a PTY via `alacritty_terminal::tty::new()` and spawn the user's default shell (from `$SHELL` or fallback to `/bin/sh`). The PTY SHALL be handed to alacritty_terminal's `EventLoop` for reading and VTE parsing.

#### Scenario: Shell spawns on startup
- **WHEN** the application starts
- **THEN** a PTY is created, the user's shell is spawned inside it, and the shell prompt appears in the rendered window

#### Scenario: Shell environment is correct
- **WHEN** the user types `echo $SHELL` in the spawned shell
- **THEN** the output is a valid terminal type (e.g., `xterm-256color`)

### Requirement: PTY resize on window resize
When the winit window is resized, the PTY SHALL be resized to match the new cell dimensions (columns x rows, calculated from window pixel size and cell metrics).

#### Scenario: Window resize updates PTY
- **WHEN** the user resizes the window from 80x24 to 120x40 cells
- **THEN** the PTY dimensions update and programs that query terminal size (e.g., `stty size`) report the new dimensions

### Requirement: alacritty_terminal EventLoop manages PTY reading
The PTY read loop SHALL be managed by alacritty_terminal's `EventLoop` on a dedicated thread. lain-core SHALL NOT implement its own PTY read loop.

#### Scenario: Term state updates from PTY output
- **WHEN** a command produces output in the shell
- **THEN** the `Term` cell grid is updated by the EventLoop and the next render frame reflects the new content
