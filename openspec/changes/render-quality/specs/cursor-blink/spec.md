## ADDED Requirements

### Requirement: Cursor blinks at 500ms intervals
The cursor SHALL alternate between visible and hidden states every 500ms while the window is focused. The blink cycle is driven by `ControlFlow::WaitUntil`; no separate thread or OS timer is required.

#### Scenario: Cursor visible after 500ms
- **WHEN** the cursor was last shown as visible at time T
- **THEN** at time T+500ms the cursor is hidden and a redraw is requested

#### Scenario: Cursor hidden after 1000ms
- **WHEN** the cursor was last shown as visible at time T
- **THEN** at time T+1000ms the cursor is visible again

### Requirement: Any keypress resets cursor to visible and restarts the blink timer
On any `KeyboardInput` event with state `Pressed`, the cursor SHALL be set to visible and the blink timer SHALL restart from that moment. This ensures the user can always see the cursor immediately after typing.

#### Scenario: Key pressed while cursor is hidden
- **WHEN** the cursor is in the hidden phase and a key is pressed
- **THEN** the cursor becomes visible immediately
- **THEN** the next hide event is 500ms from the keypress, not from the previous blink

### Requirement: Cursor blink stops and shows solid when window loses focus
When the window loses focus (`WindowEvent::Focused(false)`), the cursor SHALL be set permanently visible and the blink timer SHALL be cancelled. The cursor SHALL appear as a non-blinking solid block while unfocused.

#### Scenario: Window loses focus
- **WHEN** `WindowEvent::Focused(false)` fires
- **THEN** cursor is visible, blink timer is not rescheduled

#### Scenario: Window regains focus
- **WHEN** `WindowEvent::Focused(true)` fires
- **THEN** blink timer restarts from the moment of focus regain

### Requirement: Cursor blink state is applied after cell_grid.extract()
The blink visibility SHALL be applied by overriding `cell_grid.cursor.visible` after `extract()` returns, using the App-level `cursor_blink_visible` field. The `extract()` function signature SHALL NOT be changed.

#### Scenario: Extract sets visible = true, blink overrides to false
- **WHEN** `extract()` completes and `cursor_blink_visible` is false
- **THEN** `cell_grid.cursor.visible` is set to false before `render_frame()` is called
