## Context

The current application sets `WM_CLASS` / `app_id` correctly and installs `.desktop` and hicolor icon assets on launch. However, the X11 per-window icon path still goes through `winit::window::Icon::from_rgba(...)`, which accepts a single bitmap and therefore serializes `_NET_WM_ICON` as a single `[w, h, pixels...]` block. The active `icon-render-polish` change assumes four sizes are already being emitted, but the code path does not support that today.

This change focuses on the narrowest technical fix: preserve `winit` as the windowing abstraction, but bypass its single-size icon path on X11 by writing `_NET_WM_ICON` directly after window creation. Wayland behavior remains unchanged because `_NET_WM_ICON` is not relevant there. The user also wants the resulting design documented honestly: if we cannot validate the behavior locally on the target desktop environment, the docs must say so explicitly rather than imply full confirmation.

## Goals / Non-Goals

**Goals:**
- Publish a protocol-correct, multi-size `_NET_WM_ICON` for X11 windows
- Keep current freedesktop registration behavior for shell association intact
- Separate X11 protocol correctness from compositor-specific titlebar rendering expectations
- Document the direct X11 path as untested until validated on real target environments
- Keep the implementation contained to a small platform-specific helper rather than a windowing rewrite

**Non-Goals:**
- Patching or forking `winit`
- Introducing GTK just to manage app icons
- Changing the icon art itself
- Guaranteeing that Pop!_OS GNOME 42 or Mutter SSD titlebars visibly render the icon
- Changing Wayland window-icon behavior beyond the existing `app_id` and desktop-entry path

## Decisions

### Decision 1: Use a direct X11 property write for `_NET_WM_ICON`

The application will keep creating its window through `winit`, but on X11 it will bypass `window.set_window_icon(...)` for `_NET_WM_ICON` and write the property manually.

Why:
- `winit` only exposes a single-size `Icon::from_rgba(...)` API, so the current abstraction cannot satisfy the four-size requirement.
- `_NET_WM_ICON` is a simple EWMH property with a stable wire format.
- A narrow platform escape hatch is lower risk than patching the windowing layer.

Alternatives considered:
- Keep relying on `winit` and accept a single-size icon: rejected because it does not meet the intended behavior.
- Patch or fork `winit`: rejected as too heavy for a contained application-level requirement.

### Decision 2: Keep Wayland and shell association on the current freedesktop path

The application will continue using `WM_CLASS` / `app_id`, `.desktop`, and hicolor icon registration for shell-level app association. No direct icon property write will be attempted on Wayland.

Why:
- `_NET_WM_ICON` is an X11 concern.
- The current desktop-entry association work is already the right path for GNOME Shell and other freedesktop environments.
- This keeps the change narrowly focused on the missing X11 behavior.

Alternatives considered:
- Unify X11 and Wayland behind a new custom icon subsystem: rejected because it adds abstraction without solving a real Wayland problem.

### Decision 3: Pack four icon sizes largest-first using existing RGBA buffers

The helper will pack `128x128`, `48x48`, `32x32`, and `16x16` icon buffers into one `_NET_WM_ICON` payload in largest-first order.

Why:
- Largest-first ordering matches the intended selection behavior for consumers of the property.
- The application already has the required icon buffers in memory.
- The property format is deterministic and easy to verify with `xprop`.

Alternatives considered:
- Write only `128x128`: rejected because it preserves the current limitation.
- Generate additional sizes lazily at write time only on X11: acceptable but not necessary as a design distinction because the buffers already exist conceptually in the current icon path.

### Decision 4: Add an “Untested But Well-Used” section to repository docs

`docs/open-questions.md` will gain a dedicated section for integration patterns that are standard or widely used but not yet validated in the project’s target environment matrix. The direct X11 `_NET_WM_ICON` path will be recorded there until real-user or local-environment validation exists.

Why:
- Linux desktop integration often has a gap between protocol correctness and compositor behavior.
- The project needs a place to record “we believe this is right, but have not yet validated it here” without burying that uncertainty in chat history.
- This prevents specs and change docs from silently overstating confidence.

Alternatives considered:
- Leave the uncertainty only in the change design: rejected because it will be easy to lose once the change is archived.
- Add no standing documentation pattern: rejected because similar issues will recur.

## Risks / Trade-offs

[Direct X11 integration adds platform-specific code] -> Keep the helper isolated, Linux/X11-only, and small enough to audit easily.

[The X11 library choice may add dependency surface] -> Prefer one small direct dependency for property writes instead of broader toolkit additions.

[Protocol correctness may not translate into visible GNOME titlebar icons] -> Make `xprop` verification normative and document compositor-visible rendering as environment verification.

[Lack of local target-environment testing may leave the change partially unverified] -> Record the path under “Untested But Well-Used” with explicit verified and unverified items.

[A second active icon-related change already exists] -> Keep this change narrowly scoped to the X11 direct-write gap and the associated documentation cleanup so overlap remains tractable.

## Migration Plan

The change is runtime-only and has no data migration. On systems running X11, the new helper writes `_NET_WM_ICON` after window creation. On Wayland, behavior remains unchanged. If the direct-write path causes regressions, rollback is straightforward: remove the helper and fall back to the current single-size `winit` icon path.

## Open Questions

- Which direct X11 client library is the best fit for a minimal property write in this workspace?
- Should the helper live in `src/main.rs` temporarily or in a dedicated platform module from the start?
- How much verification evidence is required before the “untested” label can be removed from the docs?
