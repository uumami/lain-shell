## Why

Two visible quality gaps remain. First, the application icon path needed to become objectively correct on X11 and in the freedesktop shell association path; recent verification showed that Pop!_OS GNOME titlebars can still omit a visible icon even when `_NET_WM_ICON`, `WM_CLASS`, and the desktop-entry path are all correct, so titlebar rendering is not a reliable primary acceptance signal. Second, both the CPU and GPU renderers feel choppy and laggy in practice, suggesting frame pacing or present-mode issues that survived the render-quality pass. These affect every user on every display configuration.

## What Changes

- **Multi-size `_NET_WM_ICON`**: provide 16×16, 32×32, 48×48, and 128×128 in a single property so the X11 metadata path is correct and verifiable with `xprop`
- **Icon canvas redesign**: λ_ art fills the full icon canvas (currently occupies ~25% centered); redesign to maximise visual weight at all target sizes
- **Freedesktop registration hardened**: `StartupWMClass`, proper icon at multiple hicolor sizes (16, 32, 48, 128), regenerated on content change; `update-desktop-database` always called after write
- **Verification scope narrowed**: treat GNOME Alt+Tab, overview, dock, and X11 property inspection as the normative icon checks; treat Pop!_OS/Mutter SSD titlebar visibility as environment behavior to record, not a guaranteed outcome
- **Wayland `app_id` set unconditionally**: already in place; confirm it is reached before window is shown
- **Present-mode audit**: verify GPU path uses AutoVsync and that the swap chain is not triggering unnecessary reconfigures; audit CPU path for spurious full-surface blits
- **Frame-gate edge cases**: ensure the 16 ms gate does not accumulate missed frames during resize or focus change; confirm blink timer does not interfere with frame scheduling

## Capabilities

### New Capabilities

- `app-icon`: Multi-size pixel-art icon encoded directly in the binary, registered via `_NET_WM_ICON` for X11 window managers and via hicolor PNG + `.desktop` for freedesktop compositors; protocol correctness and shell-surface association are required, while compositor-specific titlebar rendering is verified separately

### Modified Capabilities

- `frame-pacing`: Existing spec covers the 60fps gate; this change adds requirements for resize and focus-change edge cases where the gate currently misbehaves, and fixes any present-mode configuration that causes visible stutter

## Impact

- `src/main.rs`: `make_icon_rgba()`, `make_window_icon()`, `try_register_desktop_icon()` — redesigned art, multi-size encoding, registration logic; frame gate edge cases in `about_to_wait()` and `window_event()`
- `crates/lain-core/src/renderer/gpu.rs`: present mode confirmation, swap-chain reconfigure audit
- `crates/lain-core/src/renderer/cpu.rs`: surface blit audit
- No new external dependencies required
