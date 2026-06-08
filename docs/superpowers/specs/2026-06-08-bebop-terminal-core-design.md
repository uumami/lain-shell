# BEBOP — terminal-core design

> STATUS: **Design — agreed through brainstorming dialogue, pending user review.**
> This is the `terminal-core` (codename **BEBOP**) subsystem design. It turns the
> render-core spike's validated learnings into a real architecture for the single
> terminal primitive.
>
> **Naming (locked):** **BEBOP** is the codename for the `terminal-core` *module* —
> one of five modules inside the trusted host binary `lain-shell` (codename
> **LAIN**). BEBOP is the terminal core (PTY/VTE/render/config/CLI), **not** the
> whole product. The whole terminal/product is **LAIN**; the user-facing surface
> (panes/tabs/layout) is **NAVI**. The engineering name `terminal-core` (`core`) is
> the load-bearing identifier in code; BEBOP is flavor for docs/UI.
>
> Inputs that bound this design:
> - Foundation contract: `2026-06-08-new-seed-foundation-design.md` (BEBOP = PTY +
>   VTE + renderer + config + CLI, in the trusted host process; SC-5, SC-6, SC-8,
>   SC-9; trust-boundary decomposition; `lain-types` seam rule).
> - Spike results: `2026-06-08-render-core-spike-results.md` (wgpu+glyphon viable
>   and beats kitty on latency/throughput; cold start ~470ms; GPU RSS floor is
>   wgpu's ~100MB PSS, not our code; `Shaping::Basic`; bundled-font win).
>
> Scope: this document is the architecture + interface design for BEBOP. The
> implementation plan is a separate artifact (writing-plans). Multiplexing,
> isolation, network, the operator agent, and block UX are explicitly out of scope
> (other subsystems / later cycles).

---

## 1. Scope and boundary

**BEBOP is one terminal.** The `terminal-core` crate owns a single terminal
surface: PTY/byte-stream <-> VTE parse + grid + scrollback <-> render, plus
terminal-level config and a minimal dev/test entry. Nothing more.

**Not BEBOP** (other subsystems, same or other process): multiplexing / tabs /
panes / layout (**NAVI**), cage isolation (**GEOFRONT**), network egress (**THE
WIRED**), the operator agent (**MAGI**), and the *visual* block/command UX (a NAVI
/ UX-cycle concern).

**The load-bearing boundary decision — BEBOP does not know where its bytes come
from.** It consumes a byte stream and accepts input bytes through a `ByteStream`
trait (read / write / resize). A local `$SHELL`, an SSH stream, or an **EVA cage
relay** all look identical to BEBOP. This is what keeps it a focused primitive and
lets GEOFRONT/cages plug in later with zero changes to the core.

- Depends only on `lain-types` (the trait seam). No other quantum module is a
  dependency. In-process: no serialization tax (foundation seam rule).
- Exposes a `Terminal` handle that the host (NAVI/LAIN) drives.
- **Deliverable this cycle:** the `terminal-core` crate + a thin `bebop run` dev
  binary that opens one window running `$SHELL` (productionizes the spike) — the
  test vehicle until NAVI exists.

**Platform:** Linux-first (X11 + Wayland via `winit`). `wgpu` keeps macOS/Metal
reachable later; `softbuffer` is cross-platform. No platform is hardcoded.

---

## 2. Contract — the criteria BEBOP owns, with revised budgets

The spike measured two of the foundation's targets as mis-set for a cold
single-process GPU terminal (kitty misses them too). Revised here, with rationale
in the spike results:

| Criterion | Original | **Revised budget for BEBOP** | How met |
|---|---|---|---|
| SC-5 latency | Ghostty-class | input->glyph ~1 frame (≈23ms pipeline / ~50ms vsync) | reactive loop, validated in spike (beats kitty) |
| SC-5 cold start | < 100 ms | **warm-window < 100ms via prewarm; cold process ~470ms accepted** (beats kitty) | prewarm orchestrated by NAVI later; BEBOP is prewarm-friendly; CPU-first-frame kept as a cheap later option |
| SC-6 RSS (GPU) | < 80 MB | **PSS-based ~110 MB** (bundled-font on) | accepted: ~97MB is wgpu's own floor, our stack adds ~9MB; not optimizable in our code |
| SC-6 RSS (headless/CPU) | < 15 MB | **< 15 MB** (spike hit 3-5MB) | CPU/softbuffer render path, no GPU |

These revisions are a deliberate, measured contract change (the alternative —
abandoning wgpu for hand-rolled GL to hit 80MB — was rejected; see §4 and the
spike's decision gate). SC-8 (legible security state) constrains BEBOP's failure
behavior (§8). SC-9 (personalization) drives §6.

---

## 3. Architecture and concurrency

BEBOP is a module in the trusted host process (no IPC inside it). The host owns
the window, event loop, and **one** wgpu device; BEBOP is a **passive,
host-driven** renderer-into-a-target.

```
   host (NAVI/LAIN): owns winit event loop + window + ONE wgpu Device
        |  drives, per pane:
        v
   +----------------------- BEBOP Terminal (one per pane) ------------------+
   | [ByteStream: PTY | ssh | EVA cage relay]                              |
   |        | blocking read  (1 dedicated reader thread)                   |
   |        v                                                              |
   |  bytes channel --(wake)--> host UI thread calls:                      |
   |        feed(bytes) -> Damage   (alacritty_terminal VTE -> grid/      |
   |                                 scrollback, +OSC133 marks)            |
   |        render(target)          (Renderer: GPU wgpu|glyphon or         |
   |                                 CPU softbuffer, into a viewport rect) |
   |        on_input(event) -> bytes/action  (key->PTY encoding)          |
   +-----------------------------------------------------------------------+
```

**Render loop: reactive / damage-driven (DECIDED).** `winit ControlFlow::Wait`;
the loop sleeps until a real event (PTY bytes via an `EventLoopProxy` wake, input,
resize, cursor-blink timer). On wake: drain+coalesce bytes, mark damage (via
`alacritty_terminal`'s damage API — free), render at most once per vblank. Idle =
0% CPU. Rejected: continuous Poll (pins a core, ~1-5W/window) and fixed-cadence
60Hz (wastes idle power *and* adds up to one frame of input latency).

**VTE parse: on the UI thread, bounded drain, exposed as a callable (DECIDED).**
Only the *blocking PTY read* is off-thread. Parse runs where render/input run, so
the grid is single-owner — no mutex, no shared-mutability bugs (correctness is a
security property here). Flood-blocks-input is solved by bounding work per wake
(parse ~2-4ms worth, render, re-arm if more pending). **Key:** parse is exposed as
a bounded callable `feed(bytes) -> Damage`, so the *threading policy is not baked
into BEBOP* — NAVI can later give a hot pane its own worker without any BEBOP
change. Rejected: dedicated parse thread + mutex (alacritty-style) and parse
thread + double-buffer — both buy flood-isolation we don't need yet at the cost of
concurrency complexity, more memory, and a worse multi-pane footprint (YAGNI; the
spike proved throughput is ample).

**Ownership: passive, shared device (DECIDED).** The host owns the loop, window,
and one wgpu `Device`; BEBOP renders into a `RenderTarget` (surface + viewport
rect). N panes share one device -> **RSS stays ~one wgpu floor regardless of pane
count**. Rejected: per-terminal device (the spike measured ~100MB PSS *per* device
-> 10 panes ≈ 1GB) and self-driving window+loop (cannot be composed into panes).
The `bebop run` dev binary is a thin self-driving wrapper around one passive
Terminal.

---

## 4. Render layer

**Dual-backend seam (DECIDED).** One `Renderer` trait, two impls:
- **GPU:** `wgpu 22` + `glyphon 0.6` (default).
- **CPU:** `softbuffer` + `cosmic-text` + `swash`.

**The seam is drawn low — at the glyph-blit layer.** Both backends share one text
engine: `cosmic-text` for shape/layout + `swash` for rasterization. Only the final
step differs — GPU uploads rasterized glyphs to the atlas; CPU blits them into the
softbuffer. So the entire text pipeline is *one code path* and the backends are
pixel-identical; the backend trait is just "put rasterized glyphs to a target."

The CPU backend does triple duty, which is why it is in scope now (not deferred):
1. the real **< 15 MB headless / no-GPU render path** (servers, SSH, CI) — required by SC-6;
2. **crash-safe degradation** when GPU init fails or the device is lost (§8) — a security terminal must never die because a driver is missing;
3. a kept-open path for **CPU-first-frame** cold start (not built this cycle; see below).

**Settled render details:**
- **Bundled font** (skip system font enumeration): saves ~18 MB PSS and ~110 ms of
  cold start (spike-measured). A monospace font ships with BEBOP; system fonts are
  an opt-in fallback for glyph coverage.
- **`Shaping::Basic`** for the grid (correct for monospace; `Advanced` cost
  250-800 ms/frame in the spike) + a **per-line shape cache** keyed by
  (text, attrs) so steady-state frame cost stays under one frame.
- **Prewarm-friendly:** the wgpu device/instance is created by the host and
  reusable; a new Terminal attaches to the shared device and a cheap per-terminal
  surface. NAVI can pool/prewarm later with no BEBOP change.
- **Present mode:** vsync (Fifo) default; Immediate exposed for latency-critical
  use.

**Cold-start stance (DECIDED — option 3 + keep the seam).** Accept cold process
~470 ms this cycle (it already beats kitty's ~600 ms). The CPU-first-frame swap
(paint via softbuffer in <100ms, hand off to GPU when the device is ready) is *not*
built now — it is moderate work with a real dual-surface-on-one-window handoff/
flicker risk, for a once-per-session polish that prewarm will largely make moot.
Because the dual-backend seam already exists, it remains a cheap later add if
cold-launch latency ever proves to matter. SC-5's <100ms is satisfied the right
way: prewarm (NAVI) for the common case.

---

## 5. Terminal model

- **PTY:** `portable-pty`, behind the `ByteStream` trait (§1), so source is
  swappable (local / ssh / cage).
- **VTE + grid + scrollback:** `alacritty_terminal` (validated in the spike; also
  provides the damage API the reactive loop uses). Parse is byte-oriented
  (`vte::ansi::Processor::advance`).
- **OSC 133 semantic marks.** Prompt-start / command-start / command-end (+ exit
  code) are captured as **stable anchors into scrollback** that survive scroll and
  reflow, exposed through the seam as iterable **command-blocks** (command text,
  output range, exit code, timestamps). This is what makes command/output
  addressable for humans (jump-to-prompt, copy-last-output) and, later, for the
  operator agent — without changing the flat-grid render model.

**Marks are advisory, never a security boundary (HARD CONSTRAINT).** OSC 133 marks
are ordinary escape sequences in the PTY stream — any program in the terminal can
forge them. Nothing security-relevant may ever depend on a mark. Security
decisions live in the guardian (**MOTOKO**), not in terminal block metadata. This
constraint is written here so no later cycle is tempted to trust an exit code or
block boundary for enforcement.

**Shell integration: manual opt-in now, auto-inject-ready (DECIDED).** Marks only
exist if the shell emits them. We ship `bebop shell-integration <bash|zsh|fish>`;
the user adds one line to their rc. This is simultaneously the *most secure*
(least action-at-a-distance, explicit consent, zero footprint, trivially
reversible) and the *simplest to build*. Its only cost — marks dark by default —
does not bite this cycle, because the consumers of marks (MAGI, NAVI block UX) do
not exist yet. The mark-capture + `ByteStream` are designed so per-session
**auto-inject** (env/rcfile tricks: bash `--rcfile`/`BASH_ENV`, zsh `ZDOTDIR`
chaining, fish `conf.d`, with secure temp-file handling) drops in later without
rework, when "works by default" becomes a real win. Permanent rc-editing is
rejected outright (a security tool must not write to dotfiles behind the user's
back). Absent integration -> no marks, a perfectly good plain terminal.

---

## 6. Config and input

**Config (SC-9):** human-readable **TOML**, version-controllable, **hot-reloaded
on save** (theme/font apply live; structural changes recreate as needed; a parse
error keeps last-good — §8). BEBOP-level keys: font (family/size/fallback),
theme/palette (16-color + truecolor + cursor), cursor style/blink, scrollback
size, shell, render-backend preference, vsync. NAVI owns pane/layout config
separately.

**Input — two distinct layers (DECIDED):**
1. **Key -> PTY byte encoding** lives in BEBOP, always. Escape sequences, modifier
   encoding, application-cursor/keypad modes, the kitty keyboard protocol. This is
   protocol correctness depending on terminal state — *not* user-configurable
   "keybindings", and not externalizable.
2. **Action bindings** (the personalization surface — copy/paste/scroll/zoom and,
   in NAVI, pane/tab/session/layout; the native/tmux/Zellij sets of SC-9) are
   resolved by a **shared, pure keymap resolver** (`chord + binding-config ->
   action-id`) that lives in the seam (`lain-types`, or a tiny `keymap` crate).
   BEBOP and NAVI both use it; each **registers its own action vocabulary** and
   handlers. One binding engine, one config format, presets as data -> coherent
   personalization across the whole app.

This cycle builds only the resolver *seam* + **BEBOP's terminal actions**; NAVI's
app actions and the tmux/Zellij presets ship with NAVI (YAGNI — no heavy
keybinding framework built prematurely). Unhandled chords bubble up to the host.

**Security of bindings:** the resolver is a pure mechanism with zero privilege —
it only *names* an action. Security-sensitive actions are enforced where they
execute (guardian/isolation), never at the binding. Bindings are human-readable
config under the **config-mirror** rule: the agent may *propose* but cannot
silently rebind to escalate.

---

## 7. Public API — the `lain-types` seam

Trait definitions live in `lain-types`; `terminal-core` implements them. In-process
=> owned types, no serialization tax. Illustrative (not final) shapes:

```rust
// lain-types
pub trait ByteStream: Send {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>; // blocking, off-thread
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn resize(&mut self, cols: u16, rows: u16) -> io::Result<()>;
}

pub trait Renderer {
    fn render(&mut self, grid: &GridView, damage: &Damage, target: &RenderTarget);
    fn kind(&self) -> RendererKind; // Gpu | Cpu
}

pub struct RenderTarget<'a> { /* surface handle + viewport rect (pane region) */ }

pub enum Notice { Info(String), Warn(String), Error { msg: String, action: Option<String> } }

// terminal-core: the handle NAVI drives
impl Terminal {
    pub fn new(cfg: &TermConfig, stream: Box<dyn ByteStream>) -> Self;
    pub fn feed(&mut self, bytes: &[u8]) -> Damage;          // bounded, callable; no thread baked in
    pub fn on_input(&mut self, ev: InputEvent) -> InputOutcome; // Bytes(..) | Action(id) | Unhandled
    pub fn render(&mut self, target: &RenderTarget);
    pub fn resize(&mut self, cols: u16, rows: u16);
    pub fn snapshot(&self) -> GridView;                      // grid + cursor + title
    pub fn command_blocks(&self) -> impl Iterator<Item = CommandBlock>; // OSC133 marks (advisory)
    pub fn notices(&mut self) -> impl Iterator<Item = Notice>;          // drained by host -> SC-8 UI
    pub fn status(&self) -> TermStatus;                      // Running | Closed{code} | Degraded{..}
}
```

The host (NAVI/LAIN) owns the loop/window/device and calls `feed` / `on_input` /
`render` / `resize`; it renders `notices` and `status` in the security-legible UI.
`ByteStream` makes PTY vs cage-relay vs ssh swappable. Any of these traits can be
promoted across a real process boundary later by adding `Serialize` — not now.

---

## 8. Errors and degradation

**Principle: degrade with a *visible* warning over crashing — but never degrade
silently, and fail *visibly* when continuing could mislead.** The most-secure and
best-UX choices coincide here; both reject silent degradation.

- **Cosmetic degradation** (GPU init fail / device lost -> CPU fallback; missing
  font -> bundled fallback): keep running, emit a **warning the user sees**
  ("running on CPU renderer — GPU unavailable"). Low security stakes (rendering is
  not a boundary), but a persistent fallback is surfaced, not hidden.
- **Integrity failures** (contained panic; PTY/child died; config unloadable): the
  affected pane enters an **explicit, visible broken/closed state** — never
  silently vanish, never look normal. "session crashed — exit 139 · press R to
  restart", not a plausible-but-wrong screen. A dead or compromised session must
  *look* dead.
- **Bad config** (parse/validation): keep last-good, emit a non-fatal notice. With
  hot-reload, a typo must never brick the terminal.
- **Panic containment:** per-terminal parse/render is wrapped (`catch_unwind` at
  the terminal boundary) so a panic in one pane is contained and reported, not
  fatal to the host and every other pane.

**Hard constraint (SC-8):** a render fallback or any BEBOP failure must never
obscure or fake a pane's security state (cage/network badge, owned by NAVI/MOTOKO).
If BEBOP cannot faithfully present the terminal or its status, it shows the error
rather than a convincing-but-wrong screen. Fail visible, fail honest.

**Mechanism:** BEBOP *emits* typed `Notice`s (info/warn/error + optional action)
and a `TermStatus` through the seam; **NAVI renders** the toast/badge (visual
design is the UX cycle's job). The notice channel is pure output (terminal ->
host), zero privilege — it cannot itself become an attack surface. Typed errors
(`thiserror`) at the `lain-types` seam; no `anyhow` in the library.

---

## 9. Testing strategy

- **VTE conformance** — table tests feeding known escape sequences, asserting grid
  state (esctest/vttest-style cases); `alacritty_terminal` is also battle-tested
  upstream.
- **Golden-frame** — the CPU backend is deterministic: render known grids, diff
  against stored reference images; GPU spot-checked against CPU.
- **Backend parity** — same grid -> CPU vs GPU pixel diff within tolerance (guards
  the shared-text-engine promise).
- **OSC 133 marks** — simulated integration streams -> assert command-blocks,
  ranges, exit codes, and survival across scroll/reflow.
- **Degradation** — force GPU-init failure -> assert CPU fallback + warning notice;
  kill the child -> assert visible closed state; feed bad config -> assert
  last-good retained + notice.
- **Performance regression gate** — reuse the spike's instrumentation
  (input->present probe, RSS/PSS sampler, throughput/jitter) as an automated bench
  asserting the §2 revised budgets, so latency/RSS/throughput cannot silently
  regress.

---

## 10. Non-goals and deferrals

- **Multiplexing / tabs / panes / layout** -> NAVI. BEBOP renders into one target;
  NAVI composes many.
- **Cage isolation / network / host-tool shims** -> GEOFRONT / THE WIRED. BEBOP
  only sees a `ByteStream`.
- **Prewarm/daemon orchestration** -> NAVI/LAIN (BEBOP is merely prewarm-friendly).
- **CPU-first-frame swap** -> later, behind the existing seam, only if needed.
- **Auto-inject shell integration** -> later, when a mark consumer makes it a win.
- **Block/command *UX*** (folding, re-run, visual blocks) and the toast/badge
  visuals -> NAVI / UX cycle. BEBOP supplies marks + typed notices only.
- **Agent consumption of marks/blocks** -> MAGI, later.

---

## 11. Suggested build order within BEBOP (for writing-plans)

Risk-first, each independently testable:

1. `ByteStream` + PTY + `alacritty_terminal` parse + `feed(bytes) -> Damage`
   (headless; testable with VTE conformance, no render).
2. CPU/softbuffer `Renderer` + shared text engine (cosmic-text + swash) + bundled
   font; golden-frame tests; hit < 15 MB headless.
3. GPU/wgpu+glyphon `Renderer` behind the same trait; backend-parity tests; the
   reactive loop + passive `RenderTarget`; the `bebop run` dev binary.
4. Input: key->PTY encoding + terminal actions + the shared keymap resolver seam.
5. Config (TOML + hot-reload) and the typed `Notice`/`TermStatus` channel + the
   degradation behaviors.
6. OSC 133 mark capture + `command_blocks` + `bebop shell-integration` snippets.
7. The performance regression gate wired to the §2 budgets.

---

## 12. Open items

- Exact bundled font choice (license + glyph coverage) — pick at implementation.
- `lain-types` vs a dedicated `keymap` crate for the resolver — settle when the
  workspace is scaffolded (foundation §10 item 7).
- Truecolor/theme config schema details — refine against the config system when
  it lands.
- Product name / crate prefix (`<product>-core`) — foundation §8, still open.
