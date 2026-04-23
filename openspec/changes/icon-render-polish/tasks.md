## 1. Icon art redesign

- [x] 1.1 Redesign `make_icon_rgba()` so the λ_ art fills at least 27 of 32 columns and 28 of 32 rows (use the full canvas width; remove the x_offset=8 centering, or grow the glyph strokes to match)
- [x] 1.2 Hand-craft a 16×16 variant of the λ_ art stored as a separate `const ART_16: &[&[u8]]`; verify it is legible as a standalone symbol
- [x] 1.3 Add a `make_icon_rgba_at(scale: u32) -> Vec<u8>` helper that nearest-neighbor upscales the 32×32 art to an arbitrary multiple (2× → 64, 4× → 128)
- [x] 1.4 Add `make_icon_rgba_16() -> Vec<u8>` using `ART_16` directly

## 2. Multi-size _NET_WM_ICON

- [x] 2.1 Replace `make_window_icon()` (single 32×32) with a function that builds a combined `Icon` from all four sizes (128, 48, 32, 16) concatenated in the `_NET_WM_ICON` Cardinals format
- [x] 2.2 Verify with `xprop -id <wid> _NET_WM_ICON` that all four sizes appear in the output after the change

## 3. Freedesktop registration hardening

- [x] 3.1 Write hicolor PNG at all four sizes: `~/.local/share/icons/hicolor/{16x16,32x32,48x48,128x128}/apps/lain-shell.png`
- [x] 3.2 Ensure `try_register_desktop_icon()` compares on-disk content byte-for-byte and rewrites only when changed (already done for 32×32; extend to cover all four sizes)
- [x] 3.3 Confirm `.desktop` file always contains `StartupWMClass=lain-shell` and `Icon=lain-shell`; rewrite if either field is missing or wrong

## 4. GPU present mode fix

- [x] 4.1 In `GpuRenderer::new()`, locate where `wgpu::SurfaceConfiguration` is built and confirm `present_mode` is `PresentMode::AutoVsync`; if it is `AutoNoVsync` or `Immediate`, change it to `AutoVsync`
- [x] 4.2 Log the resolved present mode at `info!` level after `surface.configure()` (use `config.present_mode`)
- [x] 4.3 Confirm `cargo build` succeeds with no warnings

## 5. Frame gate resize / focus edge cases

- [x] 5.1 Audit `WindowEvent::Resized` handler: confirm that `renderer.resize()` does NOT reset `last_frame`; if it does, move the `last_frame` update to only the `RedrawRequested` handler
- [x] 5.2 Audit `WindowEvent::Focused` handler: confirm that gaining focus does NOT call `window.request_redraw()` unconditionally when `needs_redraw` is false; gate the redraw request on `needs_redraw || cursor_focused` change

## 6. Verification

- [x] 6.1 `xprop _NET_WM_ICON` shows four sizes: 128×128, 48×48, 32×32, 16×16
- [x] 6.2 On the target Pop!_OS GNOME X11 environment, record whether Mutter SSD title bar shows a visible icon, and do not use this as the sole correctness check
- [ ] 6.3 GNOME Alt+Tab shows the λ_ icon for lain-shell
- [x] 6.4 `~/.local/share/applications/lain-shell.desktop` contains `StartupWMClass=lain-shell`
- [x] 6.5 GPU renderer log line shows present mode at startup
- [ ] 6.6 Resize the window rapidly: no burst of frames; CPU and GPU both feel smooth
- [ ] 6.7 `cat /etc/passwd` in rapid succession: no stutter or tearing visible
