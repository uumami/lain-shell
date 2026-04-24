## 1. X11 icon writer

- [x] 1.1 Add a Linux/X11-only helper that detects X11 window handles and writes `_NET_WM_ICON` directly
- [x] 1.2 Build the `_NET_WM_ICON` payload with four sizes in largest-first order: 128x128, 48x48, 32x32, and 16x16
- [x] 1.3 Route X11 window icon handling through the new helper while keeping Wayland on the existing desktop-entry association path

## 2. Integration and verification

- [x] 2.1 Add or wire the minimal direct X11 dependency needed for property writes
- [x] 2.2 Verify the project still builds cleanly after the X11-specific path is added
- [x] 2.3 Verify on an X11 session that `xprop -id <wid> _NET_WM_ICON` reports all four icon sizes

## 3. Documentation

- [x] 3.1 Add an “Untested But Well-Used” section to `docs/open-questions.md`
- [x] 3.2 Document the direct X11 `_NET_WM_ICON` path with verified and unverified claims
- [x] 3.3 Update icon-related change notes or comments so compositor-specific titlebar rendering is no longer described as a guaranteed outcome
