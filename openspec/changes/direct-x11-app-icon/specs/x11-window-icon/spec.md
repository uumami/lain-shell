## ADDED Requirements

### Requirement: X11 windows publish a multi-size _NET_WM_ICON
When lain-shell runs on X11, the application SHALL write `_NET_WM_ICON` directly on the X11 window with four icon sizes in a single property: 128x128, 48x48, 32x32, and 16x16. Sizes SHALL be ordered largest-first.

#### Scenario: xprop confirms four icon sizes on X11
- **WHEN** lain-shell is running on X11 and `xprop -id <wid> _NET_WM_ICON` is executed
- **THEN** the output lists four icon entries: 128x128, 48x48, 32x32, and 16x16

### Requirement: X11 icon payload uses the canonical icon buffers
The X11 `_NET_WM_ICON` payload SHALL be generated from the application’s canonical icon buffers, with 16x16 using the hand-crafted small icon and the other sizes using the main icon art.

#### Scenario: 16x16 icon remains the handcrafted variant
- **WHEN** the `_NET_WM_ICON` payload is built for an X11 window
- **THEN** the 16x16 image block uses the application’s dedicated 16x16 icon buffer rather than a downscaled 32x32 image

#### Scenario: larger icon sizes use the main icon art
- **WHEN** the `_NET_WM_ICON` payload is built for an X11 window
- **THEN** the 32x32, 48x48, and 128x128 image blocks are derived from the application’s canonical main icon art

### Requirement: Wayland does not depend on the X11 icon writer
When lain-shell runs on Wayland, the application SHALL NOT require the X11 `_NET_WM_ICON` writer for shell-level icon association.

#### Scenario: Wayland launch skips X11 icon property writes
- **WHEN** lain-shell runs on Wayland
- **THEN** the application does not attempt to write `_NET_WM_ICON` and continues relying on `app_id`, `.desktop`, and icon-theme registration for shell association
