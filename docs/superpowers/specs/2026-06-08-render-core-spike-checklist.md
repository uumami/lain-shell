# Render / terminal-core spike — checklist (THROWAWAY)

> Purpose: validate the **pleasant** half of the north star before designing the
> terminal core. The `sealed`-cage spike proved the security ceiling; this proves
> the terminal itself can be fast and light. Validates SC-5 (cold start < 100 ms,
> Ghostty-class input->glyph latency) and SC-6 (RSS < 80 MB with GPU, < 15 MB
> headless), and resolves the **GPUI vs raw wgpu+glyphon** decision with real
> numbers. Throwaway validation code — keep it light, optimize for learning,
> discard after. Do NOT build production structure here.
>
> Foundation contract: `2026-06-08-new-seed-foundation-design.md` (SC-5, SC-6,
> and §5 "rendering is the riskiest/longest piece"; GPUI-vs-wgpu open item).

## Scope decisions (locked in brainstorming, 2026-06-08)

- **wgpu+glyphon gets a REAL build** (hard numbers). **GPUI is desk-assessed**, not
  built — asymmetric effort, enough to decide.
- **Latency method:** in-process (`input-read -> grid -> draw-submit ->
  present-complete`, median/p99 + jitter under load), with **kitty** (installed;
  ghostty/alacritty if added) measured identically as the yardstick.
  Absolute keyboard-to-photon latency is acknowledged **out of scope** (no
  photodiode).
- **Pipeline is real minimal end-to-end:** `portable-pty` -> real `$SHELL` ->
  `alacritty_terminal` (VTE + grid + scrollback) -> `wgpu`+`glyphon` render.
  Enough to run a real shell, `cat` a big file, and a vtebench-style stream.
- **Headless variant in scope** (PTY+VTE, no window/GPU) for the < 15 MB target.

## Environment status (DONE 2026-06-08)

- Local X11 session (`DISPLAY=:1`, not SSH) — a windowed GPU app can run.
- GPUs: Intel HD 630 (iGPU) + NVIDIA GTX 1050 Mobile; `/dev/dri/{card0,card1,
  renderD128,renderD129}` present. wgpu will select an adapter (likely Intel via
  Vulkan/GL) — **record which adapter** and frame numbers accordingly.
- Rust 1.94.1 / cargo present. `kitty` installed (the measured yardstick).
- `vulkaninfo`/`glxinfo` not installed (mesa-utils) — optional, wgpu enumerates
  adapters itself.

## Still needed before first measurement (do in the spike session)

- [ ] One throwaway cargo bin in `.spike/render-core/` pulling `winit`, `wgpu`,
      `glyphon`, `portable-pty`, `alacritty_terminal`.
- [ ] GPU path: winit window + wgpu surface + glyphon rendering the
      `alacritty_terminal` grid each frame; keystrokes forwarded to the PTY.
- [ ] Headless path: same PTY+VTE pipeline, no window/GPU.
- [ ] Instrumentation: timestamps at process-exec, first-frame, input-read,
      draw-submit, present-complete; RSS sampler; throughput counter.
- [ ] (Optional) install `ghostty` and/or `alacritty` as extra yardsticks.

## Exit criteria (measure, write the numbers down)

1. [ ] **Cold start -> first prompt.** Wall-clock from process exec to the first
   rendered frame containing the shell prompt. Target **< 100 ms** (SC-5). Median
   over N launches. (Risk: wgpu adapter+device init alone can eat tens of ms —
   this criterion exists to expose whether < 100 ms is realistic on this stack.)
2. [ ] **Input->glyph latency (in-process).** `input-read -> present-complete` of
   the frame showing the echoed glyph; report median/p99 and frame jitter while
   streaming output. Yardstick: same workload on **kitty** (and ghostty/alacritty
   if installed). Target: within ~1 frame (~16.7 ms @ 60 Hz) of the yardstick.
3. [ ] **Throughput under load.** Stream a large output (`cat` big file /
   vtebench-style escape stream): sustained bytes/sec, dropped-frame behavior,
   and latency-under-load. Compare to kitty.
4. [ ] **RSS.** Steady-state resident memory: with GPU (target **< 80 MB**, SC-6)
   and headless (target **< 15 MB**, SC-6). Report both.
5. [ ] **GPUI desk-assessment (verdict, not a number).** Dependency mass, RSS
   floor, standalone-embed viability, pixel ownership, build weight — enough to
   decide GPUI vs raw wgpu without a second build.

## Decision gate

- **Holds** (cold start < 100 ms, latency within ~1 frame of kitty, competitive
  throughput, RSS within targets, wgpu+glyphon viable) -> commit to **raw
  wgpu+glyphon** for the terminal core; proceed to terminal-core (**BEBOP**)
  design in risk order.
- **Breaks** -> identify the failing target and cause (adapter init? glyphon?
  font shaping? our render loop?) and decide: optimize, switch text stack
  (cosmic-text/swash direct, or reconsider GPUI), or relax the target. We learned
  it with ~300 lines, not month four.

## Discipline

- Throwaway. No crates, no trait layer, no abstractions — one cargo bin that
  renders a real shell and prints numbers. Reuse libraries; build nothing
  reusable.
- One concern at a time; the five exit criteria can each be a separate tiny
  measurement.
- Capture the numbers in a short results note; that note (not the code) is the
  deliverable.
