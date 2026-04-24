## ADDED Requirements

### Requirement: Scroll viewport with mouse wheel
The terminal SHALL scroll the viewport through history when the user scrolls the mouse wheel, unless the running program has enabled terminal mouse mode.

#### Scenario: Wheel up scrolls history
- **WHEN** the user scrolls the mouse wheel upward and no program has enabled mouse mode
- **THEN** the viewport moves up through scrollback history by 3 lines per scroll tick

#### Scenario: Wheel down returns to bottom
- **WHEN** the user scrolls the mouse wheel downward while scrolled into history
- **THEN** the viewport moves toward the most recent output

#### Scenario: Wheel at bottom does not error
- **WHEN** the user scrolls down while already at the live viewport bottom
- **THEN** nothing happens and no error occurs

#### Scenario: New output snaps to bottom
- **WHEN** the user is scrolled into history and the shell produces new output
- **THEN** the viewport snaps back to the live bottom (alacritty_terminal handles this automatically)

### Requirement: Scroll viewport with keyboard
The terminal SHALL scroll the viewport via PageUp and PageDown keys when not in an application that handles those keys.

#### Scenario: PageUp scrolls one screen
- **WHEN** the user presses PageUp
- **THEN** the viewport scrolls up by one screen height

#### Scenario: PageDown scrolls one screen
- **WHEN** the user presses PageDown
- **THEN** the viewport scrolls down by one screen height

### Requirement: History buffer is 10,000 lines
The terminal SHALL retain the last 10,000 lines of output above the visible viewport.

#### Scenario: History is available after scroll
- **WHEN** the terminal has produced more than one screen of output
- **THEN** scrolling up reveals previous output up to the history limit
