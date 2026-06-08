## Why

The entire rendering strategy for lain-shell rests on an unvalidated assumption: that we can wire `portable-pty` → `alacritty_terminal` → `wgpu` (via `glyphon` + `cosmic-text`) into a correct, performant terminal. Every other piece of the architecture (sessions, security, agents, protocols) is known-solvable with established crates. The GPU rendering pipeline is the one where we won't know until we try. A vertical spike proves this before investing in the broader system.

This also establishes the Cargo workspace structure documented in `systems-design.md`, validating the 7-crate dependency graph with no inter-quantum dependencies.

## What Changes

- Create the Cargo workspace root with all 7 member crates
- Implement a minimal PTY spawn + VTE parse + wgpu render pipeline in `lain-core`
- Wire it up in `src/main.rs` with a winit event loop
- Result: a window that opens a shell, accepts keyboard input, renders colored output, handles resize
- Text rendering isolated behind an internal `TextRenderer` trait so glyphon can be swapped out later
- Skeleton `lib.rs` files for lain-navi, lain-motoko, lain-maggi, lain-wired (no implementation)

## Capabilities

### New Capabilities
- `workspace-structure`: Cargo workspace layout, 7-crate skeleton, dependency graph validation
- `pty-spawn`: Create PTY via portable-pty, spawn user shell, hand to alacritty_terminal EventLoop
- `wgpu-cell-rendering`: Render terminal cell grid via glyphon/wgpu — glyph atlas, background rects, cursor
- `input-routing`: Forward winit keyboard events to PTY, handle resize

### Modified Capabilities
- `crate-boundaries`: Concrete Cargo.toml files now exist — validates the documented dependency graph
- `pty-ownership`: Concrete implementation of Core owning PTY fd + Term + scrollback via alacritty_terminal

## Impact

- **New files**: ~30 files (Cargo.toml files, skeleton lib.rs, spike implementation)
- **Dependencies added**: wgpu 29, winit 0.30, alacritty_terminal 0.26, portable-pty 0.9, cosmic-text 0.18, glyphon (git), tokio, thiserror, serde
- **No existing code affected** (greenfield — no code exists yet)
- **Risk**: glyphon git dep is unreleased (main branch targets wgpu 29 + cosmic-text 0.18). Published 0.10.0 is pinned to wgpu 28. We use git until 0.11.0 ships, then switch.
