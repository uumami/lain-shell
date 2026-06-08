## Why

The current icon work assumes `winit` can emit a multi-size `_NET_WM_ICON`, but the active `winit` API only accepts a single raster size. That leaves the X11 window-icon path incomplete and makes the current `icon-render-polish` change overstate what has actually been implemented.

## What Changes

- Add a direct X11 icon writer that sets `_NET_WM_ICON` manually for X11 windows using four sizes in one property: 128x128, 48x48, 32x32, and 16x16.
- Keep the current Wayland and freedesktop registration path: `app_id` / `WM_CLASS`, `.desktop`, and hicolor theme icons remain the source of shell-level association.
- Make backend-specific behavior explicit: on Wayland, no direct X11 icon property is written; on X11, the application bypasses `winit::set_window_icon()` for `_NET_WM_ICON`.
- Add engineering documentation that records “untested but well-used” integration patterns, including the direct X11 icon path, with explicit verified and unverified claims.
- Tighten acceptance language so protocol correctness is required, while compositor-specific titlebar rendering is treated as environment verification rather than a guaranteed outcome.

## Capabilities

### New Capabilities
- `x11-window-icon`: backend-specific icon handling for X11 windows, including direct multi-size `_NET_WM_ICON` publication
- `engineering-uncertainty-log`: repository documentation for standard integration patterns that are adopted before they are validated across the full target environment matrix

### Modified Capabilities
- `app-icon`: narrow the existing icon guarantees so shell association and protocol correctness are normative, while compositor-specific titlebar rendering is documented as verification-dependent

## Impact

- `src/main.rs`: split X11 and Wayland icon handling, replace the current single-size `winit` icon path on X11
- `Cargo.toml` and/or crate manifests: add a direct X11 client dependency if needed for property writes
- New icon helper module or platform-specific helper code for `_NET_WM_ICON` packing and X11 property updates
- OpenSpec change artifacts under `openspec/changes/direct-x11-app-icon/`
- `docs/open-questions.md`: add an “Untested But Well-Used” section for adopted but not fully validated integration patterns
