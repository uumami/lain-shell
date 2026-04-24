## ADDED Requirements

### Requirement: Mouse events forwarded to PTY when mouse mode is enabled
When the running program has enabled terminal mouse reporting (via `\x1b[?1000h` or similar sequences), mouse button events SHALL be encoded as ANSI mouse sequences and written to the PTY.

#### Scenario: htop mouse click
- **WHEN** htop is running (which enables mouse mode) and the user clicks a process row
- **THEN** htop receives the click and responds (e.g., selects the process)

#### Scenario: fzf mouse selection
- **WHEN** fzf is running and the user clicks a list item
- **THEN** fzf receives the click event and selects the item

#### Scenario: Mouse events not forwarded when mode is off
- **WHEN** no program has enabled mouse mode and the user clicks in the terminal
- **THEN** no mouse escape sequence is written to the PTY; the click begins a selection instead

### Requirement: Mouse motion forwarded when motion mode is enabled
When the running program has enabled mouse motion reporting (`\x1b[?1002h` or `\x1b[?1003h`), cursor movement events SHALL be encoded and forwarded to the PTY.

#### Scenario: Mouse drag in vim
- **WHEN** vim is running with mouse=a and the user drags the mouse
- **THEN** vim receives the motion events and updates its visual selection

### Requirement: SGR mouse encoding used when enabled
When the running program has enabled SGR mouse encoding (`\x1b[?1006h`), mouse sequences SHALL use the SGR format (`\x1b[<Mb;x;yM` / `\x1b[<Mb;x;ym`) instead of X10 format. When SGR is not enabled, X10 format (`\x1b[Mbxy`) SHALL be used.

#### Scenario: SGR encoding for large coordinates
- **WHEN** SGR mouse mode is enabled and the user clicks at column > 95
- **THEN** the correct SGR sequence is sent (X10 encoding cannot represent coordinates above 95+32=127)

### Requirement: Scroll wheel forwarded to PTY when scroll mouse mode is enabled
When the running program has enabled mouse scroll reporting, scroll wheel events SHALL be forwarded as button 4 (up) and button 5 (down) mouse sequences.

#### Scenario: Scroll in vim
- **WHEN** vim is running with mouse=a and the user scrolls the mouse wheel
- **THEN** vim receives scroll events and scrolls its buffer accordingly

#### Scenario: Scroll wheel scrolls viewport when scroll mode is off
- **WHEN** no program has enabled mouse scroll mode and the user scrolls the mouse wheel
- **THEN** the terminal viewport scrolls through history (see terminal-scrollback spec)
