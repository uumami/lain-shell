## Context

lain-shell has comprehensive architecture documentation but zero code. The design specifies a 7-crate Cargo workspace with a wgpu-based GPU rendering pipeline. Before building the full system, we need to validate the riskiest integration: PTY → VTE → GPU rendering.

Research into existing Rust GPU terminal renderers found:
- **Rio/Sugarloaf**: ~14,000 LOC renderer, vendored font shaper. Full wgpu, production quality, heavy.
- **Alacritty**: ~3,250 LOC renderer, raw OpenGL, no shaping.
- **glyphon**: 1,628 LOC crate wrapping cosmic-text + wgpu. Atlas management, instanced rendering, simple prepare/render API.
- **Zed terminal**: delegates to GPUI (50k+ LOC framework). Terminal adapter is ~2,342 LOC.

glyphon provides the best leverage for a spike — it solves atlas management and wgpu pipeline setup, leaving terminal-specific concerns (cell grid mapping, backgrounds, cursor) to us.

alacritty_terminal 0.26.0 provides its own `EventLoop` that runs on a dedicated thread, reads PTY output, parses VTE, and updates `Term`. We don't need to manage the PTY read loop ourselves.

## Goals / Non-Goals

**Goals:**
- Validate the full pipeline: portable-pty → alacritty_terminal → cosmic-text → glyphon → wgpu
- Establish the Cargo workspace with all 7 crate skeletons
- Produce a window that functions as a basic terminal (prompt, input, colored output, resize)
- Isolate text rendering behind an internal trait for future replacement

**Non-Goals:**
- Multiple panes, tabs, sessions (Navi scope)
- Security, isolation, auditing (MOTOKO scope)
- Agent integration (MAGGI scope)
- Network protocols (THE WIRED scope)
- softbuffer fallback (next spike)
- Overlays, themes, status bars (post-spike)
- Performance optimization (spike proves correctness, not speed)
- Full lain-types API surface (only minimal IDs and errors for now)

## Decisions

### 1. glyphon via git dependency, behind internal trait

Use glyphon from its git main branch (targets wgpu 29 + cosmic-text 0.18). Published 0.10.0 requires wgpu 28, which is one major version behind.

The text rendering implementation lives behind an internal `TextRenderer` trait within lain-core. This is not the inter-quantum `CoreRenderApi` from lain-types — it's a crate-internal abstraction.

```
crate-internal:  trait TextRenderer { prepare, render, resize }
                   └── GlyphonRenderer (spike implementation)
                   └── CustomAtlasRenderer (future replacement)

inter-quantum:   trait CoreRenderApi (in lain-types, not implemented in spike)
```

**Alternative considered**: Pin wgpu 28 + glyphon 0.10.0 from crates.io. Rejected because cosmic-text 0.15 is 3 minor versions behind, missing improvements we'd want.

**Alternative considered**: Skip glyphon, build custom atlas renderer (~2,500 LOC). Rejected for the spike — adds time without reducing risk. We can always replace glyphon later behind the trait.

### 2. alacritty_terminal EventLoop owns PTY reading

alacritty_terminal provides `EventLoop` which runs on its own thread, reads the PTY, parses VTE, and updates `Term` state. We use this directly rather than building our own PTY read loop.

Our responsibilities:
- Create the PTY via portable-pty and spawn the user's shell
- Hand the PTY to alacritty_terminal's EventLoop
- On each render frame: lock `Arc<FairMutex<Term>>`, call `renderable_content()`, read cells
- Forward keyboard input from winit to PTY via the EventLoop's sender

```
Thread 1: alacritty EventLoop
  PTY fd read → VTE parse → update Term (behind FairMutex)

Thread 2: winit event loop (main thread)
  lock Term → read cells → prepare glyphon → wgpu render pass
  winit keyboard → PTY write via EventLoopSender
```

**Alternative considered**: Use alacritty_terminal::Term directly with manual PTY reading on a tokio task. Rejected — reinventing what EventLoop already provides, with worse correctness.

### 3. Workspace structure matches docs exactly

The Cargo workspace creates all 7 crates from `systems-design.md`. Five are skeleton-only (empty lib.rs). This validates the dependency graph compiles clean.

```
Cargo.toml (workspace)
crates/
  lain-types/    → PtyId, LainError, error.rs (minimal)
  lain-core/     → spike implementation
  lain-navi/     → skeleton
  lain-motoko/   → skeleton
  lain-maggi/    → skeleton
  lain-wired/    → skeleton
src/main.rs      → winit event loop, wires PTY + VTE + renderer
```

### 4. lain-core internal structure for the spike

```
crates/lain-core/src/
├── lib.rs
├── pty.rs              portable-pty spawn, shell detection
├── terminal.rs         Arc<FairMutex<Term>> wrapper, cell iteration
└── renderer/
    ├── mod.rs          TextRenderer trait definition
    ├── glyphon.rs      GlyphonRenderer: cells → cosmic-text → glyphon → wgpu
    ├── rect.rs         wgpu quad pipeline for backgrounds, cursor, underlines
    └── pipeline.rs     frame orchestration: lock term → prepare → render pass
```

### 5. Dependency versions

| Crate | Version | Source |
|---|---|---|
| wgpu | 29.0.1 | crates.io |
| winit | 0.30.13 | crates.io |
| alacritty_terminal | 0.26.0 | crates.io |
| portable-pty | 0.9.0 | crates.io |
| cosmic-text | 0.18 | crates.io |
| glyphon | main branch | git (unreleased, targets wgpu 29 + cosmic-text 0.18) |
| tokio | 1.x | crates.io (rt-multi-thread, macros) |
| thiserror | 2.x | crates.io |
| serde | 1.x | crates.io (derive) |

## Risks / Trade-offs

**[glyphon git dep is unreleased]** → If main breaks or diverges, we're stuck. Mitigation: glyphon is 1,628 LOC; worst case we vendor it or replace with custom renderer behind the trait. The trait boundary makes this a contained change.

**[glyphon's abstraction designed for editors, not terminal grids]** → cosmic-text `Buffer` may fight per-cell control. Mitigation: the spike will reveal whether we need to bypass Buffer and use lower-level cosmic-text APIs (shape_run, SwashCache directly). This is a finding, not a failure.

**[alacritty_terminal API could be awkward to integrate]** → 0.26.0 is brand new. Its EventLoop expects specific event types. Mitigation: Zed already integrates this crate successfully; their terminal_element.rs is a reference.

**[portable-pty aging dependencies]** → Uses old winapi/bitflags 1.x. Mitigation: Linux-only for now, so Windows deps don't matter. If it causes issues, replace with rustix-openpty (which alacritty_terminal already depends on).

**[Render performance unknown]** → This spike proves correctness, not performance. If rendering is slow, we optimize after (damage tracking, text run caching, etc.). The architecture supports incremental optimization.
