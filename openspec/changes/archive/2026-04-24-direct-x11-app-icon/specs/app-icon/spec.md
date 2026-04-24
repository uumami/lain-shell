## MODIFIED Requirements

### Requirement: _NET_WM_ICON provides multiple sizes
On X11, the application SHALL set `_NET_WM_ICON` with four sizes in a single property: 128x128, 48x48, 32x32, and 16x16. Sizes SHALL be ordered largest-first. Each size SHALL be generated from the canonical pixel art by nearest-neighbor scaling, except 16x16 which SHALL be hand-crafted for legibility.

#### Scenario: xprop confirms four icon sizes
- **WHEN** lain-shell is running on X11 and `xprop -id <wid> _NET_WM_ICON` is executed
- **THEN** the output lists four icon entries: 128x128, 48x48, 32x32, and 16x16

#### Scenario: Compositor-specific titlebar rendering is verified separately
- **WHEN** lain-shell runs as a non-GTK X11 window under a compositor that provides server-side decorations
- **THEN** protocol correctness is established by `_NET_WM_ICON` inspection, and any titlebar icon rendering result is treated as environment verification rather than proof of property correctness
