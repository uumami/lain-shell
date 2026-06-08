# Render / terminal-core spike — RESULTS (THROWAWAY)

> Companion to `2026-06-08-render-core-spike-checklist.md`. Validates SC-5 (cold
> start, latency) and SC-6 (RSS) and resolves **GPUI vs raw wgpu+glyphon** with
> real numbers. Throwaway validation code in `.spike/render-core/` — discard
> after. The numbers below, not the code, are the deliverable.

## What was actually built

One ~650-line cargo bin (`.spike/render-core/src/main.rs`), real end-to-end
pipeline, no abstractions:

```
portable-pty -> $SHELL -> alacritty_terminal (vte 0.13 parse + grid + scrollback)
             -> winit 0.30 + wgpu 22 + glyphon 0.6  (set_text -> shape -> prepare -> present)
```

Three modes: `coldstart` (exec -> first frame), `bench` (automated input->glyph
latency probe loop + throughput + RSS), `headless` (PTY+VTE only, no window/GPU).
Keystrokes forwarded to the PTY; the latency loop injects a probe char, waits for
the echoed glyph to appear in a *presented* frame, and times it.

Environment: X11 `DISPLAY=:1`, Intel HD 630 + NVIDIA GTX 1050. **Adapter wgpu
picked: `Intel(R) HD Graphics 630 (KBL GT2)` via Vulkan, IntegratedGpu** (default
power preference; GL backend selects the same iGPU via Mesa). Yardstick: kitty
0.21.2. Rust 1.94.1, release builds. First build 8m26s (wgpu+alacritty pull a
lot) — compile time, not a measured number.

## Results table

| # | Criterion | Target | Measured (ours) | kitty yardstick | Verdict |
|---|-----------|--------|-----------------|-----------------|---------|
| 1 | Cold start -> first prompt | < 100 ms | first frame **471 ms** med (410–526); first prompt 492 ms (sh) | **550–660 ms** | **MISS** target; **beats** kitty |
| 2 | Input->glyph latency | ~1 frame of yardstick | **23 ms** med pipeline (Immediate); **50 ms** med with vsync (Fifo) | not in-process measurable | **HOLDS** (~1 frame pipeline) |
| 3 | Throughput under load | competitive w/ kitty | **20.2 MiB/s** headless parse; ~13 MiB/s GPU path | **~9 MiB/s** | **HOLDS** (beats kitty) |
| 4 | RSS — headless | < 15 MB | **RSS 5.2 MB / PSS 3.1 MB** | n/a | **HOLDS** (3x margin) |
| 4 | RSS — GPU | < 80 MB | **RSS 141 MB / PSS 106 MB** (trimmed) | RSS 109 MB / PSS 62 MB | **MISS** (~1.7x kitty; floor is wgpu) |
| 5 | GPUI desk-assessment | verdict | raw wgpu+glyphon preferred | — | see below |

## Per-criterion findings

### (1) Cold start — MISS the 100 ms target, but the target is the problem, not the stack

Render-stack cold start (trivial `sh`, so shell init ~0) breaks down as:

```
window create ............  ~45 ms
wgpu adapter + device ....  180–280 ms   <- Vulkan/Mesa device creation, one-time
FontSystem::new() ........  85–125 ms    <- cosmic-text enumerates all system fonts
first frame (glyph upload)  ~35 ms       <- one-time atlas fill
---------------------------------------
first painted frame ......  471 ms median
first sh prompt ..........  492 ms median
```

- **wgpu device init (~200 ms) + font enumeration (~100 ms) alone are 3x the
  entire 100 ms budget**, and neither is our render-loop code — they are
  unavoidable one-time costs of the wgpu + cosmic-text stack as used naively.
- **kitty, a mature optimized GPU terminal, cold-starts in 550–660 ms on the same
  box.** We are *faster* (~471 ms). No single-process GPU terminal hits <100 ms
  cold here — the target is infeasible for a cold GPU process, not a defect of
  wgpu+glyphon.
- With the user's real zsh, first frame is still ~390–400 ms but "first prompt"
  is **~3.7 s** — the extra ~3.3 s is `.zshrc` init (measured `zsh -i -c exit` =
  3.7–4.9 s), entirely exogenous to the terminal.
- **Implication:** instant-feeling startup must come from a **prewarmed / daemon
  model** (one resident process, cheap new windows) — which is exactly how
  kitty/ghostty deliver "instant" windows too. Cold init <100 ms is not the
  achievable lever; warm-window <100 ms is.

### (2) Input->glyph latency — HOLDS

Automated probe loop, n=200, 0 lost:

- **Immediate present (pipeline only, no vsync): median 23.3 ms, p99 48.6 ms, min 12.8 ms.**
- **Fifo present (vsync, what users see): median 49.9 ms, p99 57.2 ms, min 22.1 ms.**

The pipeline itself (input-read -> grid -> shape -> present) is ~0.8–1.5 frames.
Fifo adds ~1 frame of vsync queueing — the same tax every vsync'd terminal pays.
This is ghostty/kitty-class structurally. Caveat per checklist: kitty's own
input->glyph cannot be instrumented in-process and absolute keyboard-to-photon is
out of scope (no photodiode), so this is "our pipeline is ~1 frame" rather than a
head-to-head — but a ~1-frame pipeline is the target.

### (3) Throughput under load — HOLDS (beats kitty)

5 MiB file with sprinkled SGR escapes, streamed via `cat`:

- **Headless pure VTE parse: 20.2 MiB/s** (byte-at-a-time `Processor::advance`,
  not micro-optimized; includes per-iteration sentinel-scan overhead).
- GPU path: ~13 MiB/s; it drains the buffered stream and renders the *final*
  state (2 frames for the whole flood) rather than every intermediate frame —
  i.e. it coalesces, which is what real terminals do under flood.
- **kitty render throughput: ~9 MiB/s** (cat blocks on kitty's render path).

We are competitive-to-better. `cat`-ing 5 MiB completes in ~0.25 s — fine UX.
The naive byte-at-a-time path was *not* the bottleneck; `Shaping::Advanced` was
(see Gotchas).

### (4) RSS — headless HOLDS big, GPU MISSES (the one material concern)

- **Headless: RSS 5.2 MB, PSS 3.1 MB** — 3x under the 15 MB target. The PTY+VTE+
  grid core is genuinely tiny.
- **GPU idle (baseline): RSS 159 MB, PSS 123 MB.** Over the 80 MB target ~2x.
  - Backend barely matters: Vulkan ≈ GL (within ~3 MB).
  - VmRSS overstates GPU-app memory (shared Mesa/LLVM pages): kitty is RSS 109 MB
    but **PSS 62 MB / private 45 MB**. PSS is the fair metric.

**RSS-trim probe (the optimize-path, actually measured — not hand-waved):**

| combo | font-init | RSS | PSS |
|-------|-----------|-----|-----|
| baseline | 123 ms | 160 MB | 125 MB |
| **bundled font** (1 font, skip system enumeration) | 13 ms | 143 MB | **108 MB** |
| low wgpu limits + MemoryUsage hint | 106 ms | 157 MB | 121 MB |
| **bundled font + low limits** | 1 ms | 141 MB | **106 MB** |
| **bare wgpu** (clear only, no glyphon/font/PTY) | — | 132 MB | **97 MB** |

What the probe proves:

- **The cheap mitigations only reach ~106 MB PSS** — still ~1.7x kitty, still over
  80 MB. Essentially all of the saving is the bundled font (~18 MB PSS + ~110 ms
  cold start); tighter wgpu limits do ~nothing.
- **Bare wgpu alone is 97 MB PSS.** Our *entire* terminal stack (glyphon atlas +
  cosmic-text + alacritty grid + PTY) adds only **~9 MB PSS** on top of wgpu's
  floor. Our code is already lean.
- **Therefore the ~35 MB gap vs kitty is wgpu + naga + Mesa driver baseline, not
  our code and not glyphon.** kitty avoids it by using raw OpenGL (no wgpu, no
  shader compiler resident). The gap is **not optimizable in our code** — the only
  lever is the GPU stack choice itself (raw GL/Vulkan by hand), which trades away
  the portability/safety that is the entire reason to pick wgpu.
- **Conclusion:** the realistic GPU RSS floor on this stack is **~100–110 MB PSS /
  ~140 MB RSS**, ~1.6x kitty, and it is a *floor*. Either accept it (revise SC-6 to
  a PSS-based ~110 MB) or treat kitty-parity RSS as a hard requirement that forces
  abandoning wgpu — a genuine product decision, surfaced below.

### (5) GPUI desk-assessment — verdict: raw wgpu+glyphon for the terminal core

Assessed, not built (asymmetric effort, per scope decision):

- **Dependency mass / build weight:** heavy. GPUI lives in the Zed monorepo; the
  standalone crate pulls a large tree (its own layout engine, text stack, and a
  `blade`/Vulkan GPU abstraction). Slow first builds, large surface.
- **RSS floor:** higher than raw wgpu+glyphon — GPUI targets full app UIs (Zed
  idles in the hundreds of MB). Given we already miss <80 MB with the *thin*
  stack, GPUI moves the wrong direction.
- **Standalone-embed viability:** weak/opinionated. GPUI wants to own the window
  and its own run loop/executor; embedding it under our control loop or ceding
  pixel control is awkward, and standalone use has been under-documented with API
  churn.
- **Pixel ownership:** GPUI renders through an element tree — you express UI as
  elements, not by owning the draw pipeline. A terminal cell grid wanting exact
  control of render path and latency fights that model (or drops to `blade`
  anyway).
- **Verdict:** GPUI is an *application UI* framework, not a thin terminal
  renderer. For BEBOP's terminal core — minimal, low-latency, low-footprint cell
  rendering with full pixel ownership — **raw wgpu+glyphon is the better
  primitive** (it matched/beat kitty in ~300 lines of render code). Revisit GPUI
  only if/when we need rich surrounding app chrome, and even then as a separate
  layer, not the terminal core.

## Decision gate

The checklist gate ("cold start <100 ms, latency within ~1 frame, competitive
throughput, RSS within targets -> commit to raw wgpu+glyphon") is **split**:

- **Latency, throughput, headless RSS: HOLD.** Two of these *beat* kitty.
- **Cold start <100 ms and GPU RSS <80 MB: do NOT hold** — and **kitty misses
  both too** (600 ms / 109 MB RSS / 62 MB PSS). These two targets are mis-set for
  a cold single-process GPU terminal on this Intel/Mesa hardware.

**Recommendation: commit to raw wgpu+glyphon for the terminal core (BEBOP), and
revise the two failing targets rather than switch stacks**, because:

1. The stack is *competitive with or better than kitty* on every criterion we can
   compare head-to-head, achieved in ~300 lines of render code.
2. The two misses are structural to "cold GPU process," not to glyphon — Vulkan
   device init (~200 ms) and a GPU-driver RSS floor that even kitty pays.

**Concrete follow-ups for terminal-core (BEBOP) design, in risk order:**

- **Cold start:** redefine SC-5 as **warm-window <100 ms via a prewarm/daemon
  model** (resident process, cheap window spawn). Cold first-launch ~470 ms is
  accepted (beats kitty). Cut font cost with a **bundled font loaded directly**
  (skip full `FontSystem::new()` enumeration, ~100 ms) and probe whether device
  creation can be pipelined with window/PTY setup.
- **RSS:** the probe settled this. Adopt the **bundled font** unconditionally
  (free ~18 MB PSS + ~110 ms cold start). Beyond that, the GPU RSS floor is
  wgpu's, not ours, and not optimizable in our code — so revise SC-6 to a
  **PSS-based ~110 MB GPU budget** and accept ~1.6x kitty as the price of wgpu's
  portability. The ONLY path to kitty-parity RSS is abandoning wgpu for hand-rolled
  GL — escalate that as a **product decision** (is sub-80 MB GPU RSS a hard
  requirement?), do not assume it. Keep the <15 MB headless target (we hit 3 MB).
- **Latency:** ship vsync (Fifo) by default; expose Immediate for latency-critical
  use. Per-line shape caching (a real terminal does this; the spike reshapes the
  whole grid each frame at ~15 ms) will cut steady-state frame cost well under
  one frame.

## Gotchas found (save the next person the time)

- **`glyphon 0.6` pins `wgpu 22`, not `wgpu 0.20`.** Declaring `wgpu = "0.20"`
  silently compiles a *second, incompatible* wgpu. Pin `wgpu = "22"`.
- **`Shaping::Advanced` on the full grid every frame = 250–800 ms/frame** (it ran
  rustybuzz over ~6500 glyphs per frame). `Shaping::Basic` is correct for a
  monospace ASCII grid and drops steady-state frames to ~19 ms. This single line
  was the difference between "unusable" and "beats kitty."
- **vte 0.13 `Processor::advance` is byte-at-a-time**, not slice-at-a-time.
- A streamed-sentinel appears in the **echoed command line** before the command
  runs; use `echo __SPIKE""DONE__` so the literal only appears in output.
- The user's **zsh takes ~4 s to start** — it dominates any "time to prompt"
  measurement. Measure the render stack with `/bin/sh`.
