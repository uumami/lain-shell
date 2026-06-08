## Purpose
Define how user keyboard input is translated into terminal actions and PTY byte streams.
## Requirements
### Requirement: Keyboard input forwarded to PTY
winit keyboard events SHALL be translated to byte sequences and written to the PTY. Standard keys (printable characters, Enter, Backspace, Tab, Escape, arrow keys, Ctrl+key, Space) SHALL produce correct escape sequences. PageUp and PageDown SHALL scroll the viewport rather than being forwarded to the PTY (unless overridden by future application mode detection). Ctrl+Shift+C and Ctrl+Shift+V are reserved for clipboard operations and SHALL NOT be forwarded to the PTY.

#### Scenario: Printable character input
- **WHEN** the user presses a printable key (e.g., 'a')
- **THEN** the corresponding byte is written to the PTY and echoed by the shell

#### Scenario: Space key input
- **WHEN** the user presses the space key
- **THEN** a space byte (0x20) is written to the PTY

#### Scenario: Control key input
- **WHEN** the user presses Ctrl+C
- **THEN** SIGINT is sent to the foreground process in the PTY

#### Scenario: Arrow keys produce escape sequences
- **WHEN** the user presses an arrow key
- **THEN** the correct ANSI escape sequence is written to the PTY (e.g., `\x1b[A` for Up)

#### Scenario: PageUp scrolls viewport
- **WHEN** the user presses PageUp
- **THEN** the viewport scrolls up through history; nothing is written to the PTY

#### Scenario: Ctrl+Shift+C reserved for clipboard
- **WHEN** the user presses Ctrl+Shift+C
- **THEN** the selected text is copied to the system clipboard; nothing is forwarded to the PTY

### Requirement: Window close terminates cleanly
When the winit window is closed, the application SHALL terminate the PTY, wait for the shell to exit, and shut down cleanly without orphaned processes.

#### Scenario: Clean shutdown
- **WHEN** the user closes the window
- **THEN** the shell process terminates and the application exits with code 0

