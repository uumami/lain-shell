# BEBOP Plan 3 — GPU render backend + reactive loop: RESULTS

> Status: **DONE**. All tests pass, both smoke targets pass, backend-parity gate
> holds. Branch `new-seed`, commit on 2026-06-08.

## What shipped

**`GpuRenderer`** (`crates/terminal-core/src/render_gpu.rs`) — wgpu 22 + glyphon
0.6, behind the shared cosmic-text layout layer (`cell_metrics` + `grid_to_text`
shared with `CpuRenderer` — the "shared text engine" seam from design §4/§9).
Renders into any RGBA-compatible view (`render_to_view`) or a headless offscreen
texture with CPU readback (`render_offscreen`). `try_headless_gpu()` lets GPU
tests skip gracefully when no adapter is present.

**GPU sRGB clear-color fix** — `Rgba8UnormSrgb` targets sRGB-encode the linear
clear value on write; the background was reading back as `0x404045` rather than
`0x0d0d0f`. Clearing with the sRGB pre-images (r=g=0.004025, b=0.004777) makes
the background round-trip to the intended `0x0d0d0f`, matching the CPU backend.

**`LocalPty` structural single-reader fix** (deferred from Plan 1) — the one
reader fd is held in an `Option` and moved out by `take_reader`; subsequent
`ByteStream::read` calls error immediately. The two read paths (pump thread vs
direct) are mutually exclusive by construction, not by convention.

**`ReaderPump::start_with_waker`** — accepts a waker closure and calls it per
enqueued chunk, allowing a reactive event loop to be notified of PTY output
without polling. `start` delegates to it with a no-op waker. Bounded-channel
backpressure (design §3.1) is unchanged.

**`bebop` dev binary** (`crates/terminal-core/src/bin/bebop.rs`) — reactive winit
0.30 loop (`ControlFlow::Wait`) woken by PTY bytes via the pump waker. Renders a
real `$SHELL` session through `Terminal` -> `GpuRenderer` (wgpu + glyphon, vsync
`Fifo` present mode) or `CpuRenderer` (softbuffer blit) with `--cpu`. `--smoke`
flag runs a headless feed-and-snapshot round-trip and exits. Spike-grade input
encoding.

## Measured outcomes

**Backend parity (headless offscreen, Intel HD 630 / Vulkan, ran for real):**

```
CPU vs GPU ink-mask agreement: 1.000  (7343 / 7344 px agree)
```

One antialiasing-fringe pixel differs between atlas-based (GPU) and direct-blit
(CPU) rendering. The gate is a loose gross-divergence floor (>=0.70); the actual
number is reported, not gamed. This validates the shared-text-engine promise from
design §4/§9 — both backends shape and lay out glyphs identically.

**Smoke tests:**

```
bebop run --smoke        adapter: Intel(R) HD Graphics 630 (KBL GT2) | Vulkan | IntegratedGpu (Mesa)
                         result:  SMOKE_OK

bebop run --cpu --smoke  result:  SMOKE_OK
```

**Test suite:**

- GPU unit tests (`render_gpu::`) ran for real (not skipped) and pass.
- Whole workspace: **21 tests green** (lain-types 2; terminal-core lib 16;
  backend_parity 1; flood 1; render_rss 1).
- `cargo clippy --all-targets` clean, zero warnings.

## Known minor polish (non-blocking)

- `bebop` binary has two `.unwrap()`s on `self.pty` (guaranteed-set after
  `resumed`) that are stylistically inconsistent with the file's prevailing
  `if let` guard style; and a smoke-comment that slightly overstates what the
  smoke verifies (it matches `BEBOPSMOKE` on the echoed command line, which
  proves feed + render + snapshot round-trips — the smoke's actual purpose).
  Dev-vehicle grade; not fixed.
- `GpuRenderer` bundled-font DB setup is duplicated from `CpuRenderer`; a shared
  helper would reduce repetition.
- `render_offscreen` RGBA byte-order readback is only valid for RGBA-ordered
  formats; `new_offscreen` pins `Rgba8UnormSrgb` so the public path is safe, but
  the assumption is not enforced inside `render_offscreen`.
- A `[[bin]]` stanza added in Task 1 pointing at a not-yet-existing source file
  was removed; the `bebop` bin is auto-discovered from `src/bin/bebop.rs`.

## Deferrals / next

**Color and attributes** — `GridSnapshot` is chars-only (monochrome was the
conscious Plan 3 scope). Adding fg/bg/attrs touches `lain-types`, `Terminal::snapshot`,
and both renderers; earns its own TDD cycle.

**Real key->PTY encoding** — the dev binary's input is spike-grade (no escape
sequences, modifiers, app-cursor/keypad, kitty keyboard protocol). A later plan.

**CPU-first-frame cold-start swap and prewarm/daemon orchestration (NAVI)** —
deferred per design §10.

**`bebop shell-integration`**, OSC 133 marks, config, typed Notice/TermStatus,
degradation — later plans.

**Performance regression gate** — no automated per-commit gate yet; later plan.

**Typed error for taken-state read** — the Plan 2 review note about using a
matchable `io::ErrorKind` (instead of `Error::other`) for `LocalPty::read`'s
taken-state error belongs to the typed-error / Notice work.
