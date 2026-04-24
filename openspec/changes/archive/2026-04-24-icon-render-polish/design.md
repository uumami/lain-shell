## Context

Two independent problems need fixing. The icon investigation revealed three layered issues: the pixel art occupied only ~25% of the 32×32 canvas (centered at x=8, y=10 with a 16×12 glyph), `_NET_WM_ICON` originally carried only one size while well-behaved X11 apps provide several, and GNOME Shell requires `StartupWMClass` to associate the running window with the `.desktop` entry at creation time. Follow-up verification on Pop!_OS GNOME X11 showed a subtler constraint: even with `_NET_WM_ICON`, `WM_CLASS`, and the freedesktop registration path all correct, the Mutter SSD titlebar may still show no visible icon because the left titlebar area is governed by the desktop's `appmenu` layout and extension/theme behavior. The render lag investigation is less certain — the render-quality pass implemented the 60fps frame gate but the user reports both CPU and GPU paths still feel choppy. Likely causes are the GPU present mode not actually engaging vsync, spurious full-surface blits on the CPU path, and edge cases where resize or focus change resets the frame gate state in ways that cause burst redraws.

## Goals / Non-Goals

**Goals:**
- Icon visible in GNOME Alt+Tab, GNOME overview, GNOME taskbar (Pop Shell tiling header)
- Icon visible on Wayland compositors that use the freedesktop icon theme
- `_NET_WM_ICON` provides sizes that can be verified directly on X11 without scaling artefacts
- CPU and GPU renderers feel smooth at 60fps; no visible stutter on resize, focus change, or rapid PTY output
- No new external dependencies

**Non-Goals:**
- Guaranteeing that Pop!_OS GNOME SSD titlebars visibly render the icon
- Animated icon
- SVG / scalable icon (would need `resvg` or similar)
- HiDPI > 2x icon sizes (128×128 is sufficient for all current displays)
- Fixing choppiness caused by terminal programs generating output faster than 60fps (that is working as designed)

## Decisions

### Icon: provide four sizes via nearest-neighbor upscaling

**Decision**: Generate 16×16, 32×32, 48×48, and 128×128 from a single source art definition using nearest-neighbor upscaling. The source art is defined at 32×32 (the natural design size). The 16×16 is hand-crafted separately for legibility. Both are stored as `const` arrays in Rust; no file I/O at build time.

**Why over vector / font rendering**: No additional dependencies. No silent failures when the system font lacks the glyph. Identical output on every machine.

**Why nearest-neighbor over bicubic/bilinear**: Pixel art is designed on a grid; smooth filtering blurs the edges and makes the shape harder to read at 48 and 128px. Hard-pixel scaling preserves the intent.

**`_NET_WM_ICON` format**: The property is a flat array of Cardinals in the form `[w, h, pixel...][w, h, pixel...]`. Sizes must be provided largest-first so Mutter's size selection prefers the closest match. Order: 128, 48, 32, 16.

### Icon canvas: art fills >= 85% of canvas

**Decision**: Redesign the λ_ art so it occupies at least 27 of 32 columns and 28 of 32 rows. Leave a 1–2 pixel border. The current art (16×12 centered) is too small to be recognisable when Mutter scales the icon to its title-bar slot (~18–24px depending on theme).

**Why**: At 16px render size, a 32×32 icon with 25% content maps to a 4×3 pixel shape — invisible as a shape. An icon that fills the canvas maps to a 14×14 pixel shape — readable.

### Frame gate: audit don't redesign

**Decision**: Do not change the frame gate architecture. Instead, audit and fix two specific edge cases:
1. `WindowEvent::Resized` calls `renderer.resize()` which may reconfigure the wgpu surface; if this happens mid-frame or triggers a spurious `RedrawRequested`, the frame gate `last_frame` timestamp is not updated, causing a burst of frames.
2. GPU present mode: verify the `wgpu::PresentMode` in `GpuRenderer::new()` is `AutoVsync` at runtime, not `AutoNoVsync`. If `AutoVsync` is not available, fall back to `Fifo`.

**Why not a full redesign**: The gate itself is correct. The blink timer and frame gate compose via `.min()` already. The user-reported lag is most likely the GPU path submitting frames without vsync (no back-pressure), causing frames to be presented faster than the display can consume them, creating visible tearing or judder.

### Freedesktop registration: always verify, not just on first launch

**Decision**: On every launch, compare the in-memory PNG bytes and `.desktop` content against what is on disk. Rewrite only if the content would differ. This ensures the icon auto-updates after a binary move without requiring the user to manually delete files.

**`update-desktop-database` vs `gtk-update-icon-cache`**: Both are spawned as best-effort fire-and-forget. Neither is required for correct operation at runtime (GTK's in-process icon lookup does not use the cache for `~/.local/share/icons`).

### Verification: prefer shell surfaces and property inspection over SSD titlebars

**Decision**: Treat `xprop` verification of `_NET_WM_ICON`, correct `WM_CLASS`, and correct shell-surface association (Alt+Tab, overview, dock/taskbar) as the normative icon checks. Record Mutter SSD titlebar behavior as environment-specific evidence, not as a guaranteed product invariant.

**Why**: Pop!_OS GNOME X11 can reserve the left titlebar area for `appmenu` layout rather than use it as a simple icon slot. That means a window can be completely correct at the X11 and freedesktop levels while the titlebar still appears iconless.

## Risks / Trade-offs

[Nearest-neighbor at 128×128 looks very pixelated] → Acceptable: app icons are not intended to be viewed at 128px; this size is used by compositors for the overview or dock at 48–64 CSS pixels, which downscales the pixelation. A smoother design can replace it later.

[Content-comparison re-encodes the PNG on every launch] → Negligible: the PNG encode is ~200µs; the file read is ~50µs. Total overhead on launch is <1ms.

[GNOME Shell may not pick up `StartupWMClass` for already-running windows] → No mitigation possible without GNOME Shell restart. Document that a relaunch is required if lain-shell was running before the `.desktop` file was written.

[Correct `_NET_WM_ICON` data may still not produce a visible titlebar icon on every compositor/theme] → Treat X11 property correctness and GNOME shell-surface association as normative, and compositor titlebar rendering as environment verification only.

[GPU vsync: `AutoVsync` may not be supported on all drivers] → Fallback to `Fifo` is universally supported and provides vsync. Log the chosen present mode at startup at `info!` level.

## Migration Plan

No database migrations. No breaking API changes. The freedesktop files are updated in-place on the next launch. Users running lain-shell at the time of update will see the new icon on their next relaunch.
