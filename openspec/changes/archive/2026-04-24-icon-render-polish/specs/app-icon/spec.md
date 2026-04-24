## ADDED Requirements

### Requirement: _NET_WM_ICON provides multiple sizes
On X11, the application SHALL set `_NET_WM_ICON` with four sizes in a single property: 128×128, 48×48, 32×32, and 16×16. Sizes SHALL be ordered largest-first. Each size SHALL be generated from the canonical pixel art by nearest-neighbor scaling, except 16×16 which SHALL be hand-crafted for legibility.

#### Scenario: xprop confirms four icon sizes
- **WHEN** lain-shell is running on X11 and `xprop -id <wid> _NET_WM_ICON` is executed
- **THEN** the output lists four "Icon (NxN):" entries: 128×128, 48×48, 32×32, and 16×16

#### Scenario: Compositor-specific titlebar rendering is verified separately
- **WHEN** lain-shell runs as a non-GTK X11 window and Mutter provides server-side decorations
- **THEN** `_NET_WM_ICON` correctness is established by inspecting the property, and any visible titlebar icon result is treated as environment verification rather than proof of property correctness

#### Scenario: Shell surfaces remain the primary user-facing verification
- **WHEN** lain-shell is associated correctly with its desktop entry and icon theme assets
- **THEN** the expected user-facing icon checks are GNOME shell surfaces such as Alt+Tab, overview, and dock/taskbar, rather than compositor-specific SSD titlebar rendering

### Requirement: Icon art fills the canvas
The pixel art SHALL occupy at least 85% of the icon canvas width and height at the 32×32 reference size. No significant empty border SHALL exist that reduces the visual weight of the icon when scaled down.

#### Scenario: Art spans nearly full width
- **WHEN** the 32×32 RGBA buffer is generated
- **THEN** at least 27 of the 32 columns contain at least one foreground pixel

#### Scenario: Art spans nearly full height
- **WHEN** the 32×32 RGBA buffer is generated
- **THEN** at least 28 of the 32 rows contain at least one foreground pixel

### Requirement: Transparent background
The icon background SHALL use alpha=0 (fully transparent) so it composites correctly on both light and dark compositor themes without a visible square border.

#### Scenario: Background pixels are transparent
- **WHEN** inspecting the RGBA buffer returned by `make_icon_rgba()`
- **THEN** all pixels that are not part of the λ_ shape have RGBA = (0, 0, 0, 0)

#### Scenario: Foreground pixels are fully opaque
- **WHEN** inspecting the RGBA buffer
- **THEN** all λ_ shape pixels have alpha = 255

### Requirement: Freedesktop icon registration
The application SHALL write a 32×32 PNG and a `.desktop` file to `~/.local/share/icons/hicolor/32x32/apps/lain-shell.png` and `~/.local/share/applications/lain-shell.desktop` respectively. Files SHALL be regenerated whenever their on-disk content would differ from the current expected content.

#### Scenario: PNG written on first launch
- **WHEN** lain-shell is launched and no PNG exists at the target path
- **THEN** a valid PNG file is written and `file <path>` reports "PNG image data, 32 x 32"

#### Scenario: .desktop file contains required fields
- **WHEN** the `.desktop` file is written
- **THEN** it contains `Icon=lain-shell`, `StartupWMClass=lain-shell`, and `Type=Application`

#### Scenario: File is updated when content changes
- **WHEN** the binary has moved and the `Exec=` path in the `.desktop` file would differ
- **THEN** the `.desktop` file is rewritten on the next launch with the new path

#### Scenario: Registration is idempotent when content matches
- **WHEN** lain-shell is launched and the files already exist with correct content
- **THEN** no writes occur and no cache tools are re-invoked

### Requirement: Wayland app_id is set
On Linux, the window SHALL have its Wayland `app_id` (and X11 `WM_CLASS`) set to `"lain-shell"` before the window is shown, so compositors can match the window to `lain-shell.desktop`.

#### Scenario: WM_CLASS is lain-shell on X11
- **WHEN** lain-shell is running on X11 and `xprop -id <wid> WM_CLASS` is executed
- **THEN** the output is `WM_CLASS(STRING) = "lain-shell", "lain-shell"`

#### Scenario: app_id matches StartupWMClass
- **WHEN** `StartupWMClass` in the `.desktop` file is inspected
- **THEN** it equals the WM_CLASS instance field ("lain-shell")
