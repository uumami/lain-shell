## ADDED Requirements

### Requirement: Keyboard input forwarded to PTY
winit keyboard events SHALL be translated to byte sequences and written to the PTY. Standard keys (printable characters, Enter, Backspace, Tab, arrow keys, Ctrl+key) SHALL produce correct escape sequences.

#### Scenario: Printable character input
- **WHEN** the user presses a printable key (e.g., 'a')
- **THEN** the corresponding byte is written to the PTY and echoed by the shell

#### Scenario: Control key input
- **WHEN** the user presses Ctrl+C
- **THEN** SIGINT is sent to the foreground process in the PTY

#### Scenario: Arrow keys produce escape sequences
- **WHEN** the user presses an arrow key
- **THEN** the correct ANSI escape sequence is written to the PTY (e.g., `\x1b[A` for Up)

### Requirement: Window close terminates cleanly
When the winit window is closed, the application SHALL terminate the PTY, wait for the shell to exit, and shut down cleanly without orphaned processes.

#### Scenario: Clean shutdown
- **WHEN** the user closes the window
- **THEN** the shell process terminates and the application exits with code 0
