# ADR-004: wgpu for GPU Rendering (Not GPUI)

## Status

Accepted

## Date

2026-04-03

## Context

lain-shell needs a GPU rendering pipeline that can render terminal cell grids, overlays (MOTOKO indicators, agent status, cost readouts), themes with image backgrounds, and split pane layouts. Two primary candidates:

1. **`wgpu`** — low-level GPU abstraction (WebGPU standard). Gives access to the GPU pipeline. You build everything on top.
2. **GPUI** — Zed's full UI framework. Includes `gpui-terminal` (a working terminal view backed by `alacritty_terminal`), layout system, text rendering, compositing.

## Decision

Use `wgpu` directly. Do not use GPUI.

### Core owns the pixels

The philosophy states that Core owns the rendering pipeline. This is a primary reason lain-shell exists as its own platform. GPUI would own the pixels — Core would be a tenant in someone else's framework.

### GPUI doesn't give you what makes lain-shell different

GPUI gives you a basic terminal view. But lain-shell's differentiators — overlay compositor, split pane rendering with per-pane status, MOTOKO alert layers, image backgrounds, theme engine, agent output panels — are all built on top regardless. With GPUI, you build them fighting an abstraction designed for a code editor. With wgpu, you build them directly.

### Dependency risk

GPUI is maintained by one company (Zed Industries) for one product (Zed). It is not a general-purpose framework. If Zed pivots GPUI's API for editor-specific needs, lain-shell must adapt or fork. `wgpu` implements the W3C WebGPU standard — the API is standardized and has multiple implementations.

### The cost is bounded

Building a terminal renderer on wgpu means:
- Glyph atlas (texture cache for rendered glyphs)
- Cell grid renderer (vertex buffer → draw call)
- Compositor (layer terminal + overlays + status)
- Shaders (WGSL, a few hundred lines)

This is ~4-8 weeks of work. It is a one-time cost. Once built, the renderer is stable infrastructure that rarely changes. The churn is in features above the renderer, which are custom regardless.

### Ghostty validates the approach

Ghostty built its renderer from scratch on GPU APIs and is the fastest terminal in existence. The approach is proven.

## Consequences

### Enables

- Full control over every pixel — overlays, themes, and agent UI rendered as native GPU elements
- No dependency on another project's framework decisions
- Long-term stability (W3C standard)
- Performance ceiling limited only by our implementation

### Costs

- 4-8 weeks additional upfront work vs. using GPUI
- Must build glyph cache, cell renderer, compositor from scratch
- Must study Ghostty and Zed source for rendering architecture patterns

### Risks

- Building a correct, fast terminal renderer is non-trivial
- Mitigation: Study Ghostty (open source) and Zed's terminal view. The patterns are known.

## Alternatives Considered

### GPUI

Zed's framework. Gets to a working terminal in ~1 week. Rejected because it creates a framework dependency that contradicts Core's pixel ownership, doesn't support lain-shell's overlay and theming needs without fighting the abstraction, and depends on a single company's product roadmap.

### Raw Vulkan/Metal

Maximum control, maximum work. wgpu abstracts this correctly. No benefit to going lower.
