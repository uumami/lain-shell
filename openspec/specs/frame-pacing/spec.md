# frame-pacing Specification

## Purpose
TBD - created by archiving change render-quality. Update Purpose after archive.
## Requirements
### Requirement: PTY wakeups are coalesced behind a frame gate
The renderer SHALL NOT redraw on every PTY `Wakeup` event. Instead, a `needs_redraw` dirty flag SHALL be set, and the actual redraw SHALL be deferred to the next frame boundary (16.7ms / 60fps). Multiple wakeups arriving within a single frame interval SHALL produce exactly one redraw. A resize event or focus change SHALL NOT reset `last_frame` or cause burst redraws outside the normal gate cadence.

#### Scenario: Burst of wakeups within one frame
- **WHEN** the PTY sends 10 Wakeup events within 5ms
- **THEN** the renderer produces exactly one frame covering all 10 updates

#### Scenario: Sparse wakeups across multiple frames
- **WHEN** Wakeup events arrive 20ms apart
- **THEN** each Wakeup produces one rendered frame at the next frame boundary

#### Scenario: Resize does not burst frames
- **WHEN** the window is resized continuously (e.g. dragging the corner)
- **THEN** frames are produced at most at the 60fps frame rate; no frame burst occurs after resize

#### Scenario: Focus change does not burst frames
- **WHEN** the window loses and regains focus
- **THEN** at most one frame is produced per focus event; the blink timer restarts cleanly without triggering a rapid sequence of redraws

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

### Requirement: GPU renderer uses vsync present mode
The GPU renderer SHALL use `wgpu::PresentMode::AutoVsync` as its preferred present mode. If `AutoVsync` is not available on the current adapter, it SHALL fall back to `PresentMode::Fifo`. The chosen mode SHALL be logged at `info!` level on startup.

#### Scenario: Present mode is vsync on a standard display
- **WHEN** lain-shell starts on a display with a standard driver (Mesa, NVIDIA proprietary)
- **THEN** the log contains "present mode: AutoVsync" or "present mode: Fifo" and frames are not produced faster than the display refresh rate

#### Scenario: No tearing or judder during rapid PTY output
- **WHEN** `cat /dev/urandom | head -c 100000` produces burst PTY output
- **THEN** the display updates smoothly without visible tearing or judder

