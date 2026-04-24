## ADDED Requirements

### Requirement: Mouse selection copies to PRIMARY
When the user completes a mouse selection (button released), the selected text SHALL be written to the system PRIMARY selection (X11 PRIMARY atom / Wayland zwp_primary_selection_v1). If the platform does not support PRIMARY, this is a silent no-op.

#### Scenario: Select and middle-click paste
- **WHEN** the user click-drags to select text and then middle-clicks in the terminal
- **THEN** the selected text is pasted into the PTY

#### Scenario: Selection cleared on new click
- **WHEN** the user clicks without dragging
- **THEN** any existing selection is cleared

### Requirement: Ctrl+Shift+C copies selection to CLIPBOARD
When the user presses Ctrl+Shift+C and a selection is active, the selected text SHALL be written to the system CLIPBOARD.

#### Scenario: Copy selection to clipboard
- **WHEN** the user has selected text and presses Ctrl+Shift+C
- **THEN** the selected text is available to paste in other applications via Ctrl+V

#### Scenario: No selection is a no-op
- **WHEN** the user presses Ctrl+Shift+C with no active selection
- **THEN** nothing happens and the clipboard is unchanged

### Requirement: Ctrl+Shift+V pastes CLIPBOARD into PTY
When the user presses Ctrl+Shift+V, the current CLIPBOARD contents SHALL be written to the PTY as if typed.

#### Scenario: Paste from clipboard
- **WHEN** the user presses Ctrl+Shift+V
- **THEN** the clipboard text is written to the PTY and processed by the shell

#### Scenario: Empty clipboard is a no-op
- **WHEN** the clipboard is empty and the user presses Ctrl+Shift+V
- **THEN** nothing is written to the PTY

### Requirement: OSC52 clipboard sequences are handled
When a running program sends an OSC52 escape sequence to copy text to the clipboard (e.g., vim/nvim with `"+y`), the terminal SHALL write the decoded text to the system CLIPBOARD.

#### Scenario: Vim clipboard yank
- **WHEN** the user yanks to the system register in vim (`"+y`)
- **THEN** the yanked text is available in the system clipboard for pasting in other applications

#### Scenario: OSC52 paste request
- **WHEN** a program sends an OSC52 paste request escape sequence
- **THEN** the terminal reads the CLIPBOARD and writes the encoded response to the PTY

### Requirement: PtyWrite and TextAreaSizeRequest events are handled
Programs that query terminal attributes (device attributes, text area size) SHALL receive correct responses. These events from alacritty_terminal SHALL be forwarded to the PTY.

#### Scenario: nvim text area size query
- **WHEN** nvim queries the terminal text area size via escape sequence
- **THEN** the terminal responds with the correct pixel dimensions

#### Scenario: Device attribute query
- **WHEN** a program sends a DA1 or DA2 query
- **THEN** the terminal responds via the PTY write path

### Requirement: Window title updates from escape sequences
When a running program sets the terminal window title via OSC 0 or OSC 2 escape sequences, the winit window title SHALL be updated accordingly.

#### Scenario: Shell sets window title
- **WHEN** the shell sets the title (e.g., `echo -ne "\033]0;my title\007"`)
- **THEN** the window title bar shows "my title"
