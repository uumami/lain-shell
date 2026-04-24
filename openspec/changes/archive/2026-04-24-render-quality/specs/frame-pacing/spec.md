## ADDED Requirements

### Requirement: PTY wakeups are coalesced behind a frame gate
The renderer SHALL NOT redraw on every PTY `Wakeup` event. Instead, a `needs_redraw` dirty flag SHALL be set, and the actual redraw SHALL be deferred to the next frame boundary (16.7ms / 60fps). Multiple wakeups arriving within a single frame interval SHALL produce exactly one redraw.

#### Scenario: Burst of wakeups within one frame
- **WHEN** the PTY sends 10 Wakeup events within 5ms
- **THEN** the renderer produces exactly one frame covering all 10 updates

#### Scenario: Sparse wakeups across multiple frames
- **WHEN** Wakeup events arrive 20ms apart
- **THEN** each Wakeup produces one rendered frame at the next frame boundary

### Requirement: Keystroke redraws bypass the frame gate
A `KeyboardInput` event SHALL trigger an immediate `request_redraw()` call, bypassing the 60fps frame gate. This ensures keystrokes are visually echoed at the earliest possible opportunity.

#### Scenario: Key pressed while frame gate is waiting
- **WHEN** a key is pressed and the next frame boundary is 12ms away
- **THEN** a redraw is requested immediately, not deferred to the next boundary

### Requirement: Frame gate uses ControlFlow::WaitUntil
The event loop SHALL use `ControlFlow::WaitUntil(last_frame_time + frame_interval)` to schedule frame-gated redraws. The event loop SHALL NOT busy-spin.

#### Scenario: No events and nothing dirty
- **WHEN** no PTY output, no keystrokes, no cursor blink tick
- **THEN** the event loop sleeps until the next cursor blink deadline or OS event

### Requirement: Frame interval is 16.7ms (60fps target)
The frame gate interval SHALL be 16.7ms, targeting 60 frames per second. This value SHALL be a named constant, not a magic number inline.

#### Scenario: Frame interval constant is defined
- **WHEN** the code compiles
- **THEN** a constant named `FRAME_INTERVAL` of type `Duration` with value 16.7ms exists in the event loop module
