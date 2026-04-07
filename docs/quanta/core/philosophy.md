# Core — Philosophy

> *The terminal itself. The foundation everything else stands on.*

---

## Purpose

Core is the terminal. It opens a window, spawns a PTY, renders a grid, handles input, manages configuration, provides the CLI. Every other quantum depends on Core. Core depends on none of them.

If you stripped away MAGGI, MOTOKO, Navi, and THE WIRED, Core would still be a functional terminal. This is the litmus test for whether something belongs in Core or in another quantum.

---

## Principles

### Core must be fast

Startup must feel instant. Rendering must match or beat the fastest existing terminals (Alacritty, Ghostty, Kitty). Input latency must be imperceptible. These are not aspirations — they are constraints.

### Core owns the pixels

The rendering pipeline belongs to Core. This is a primary reason lain-shell exists as its own platform. Owning the renderer means overlays, status indicators, agent output, MOTOKO alerts, and themes are rendered as real UI elements — not escape sequence hacks layered on top.

### Core is the config authority

The `.lain/` directory, user-global config, config parsing, validation, and schema publication all belong to Core. Other quanta read config through Core's API.

### Core does not reason

Core never invokes a model. It is deterministic. It parses, renders, spawns, routes, stores, and serves. Intelligence belongs to MAGGI. Observation belongs to MOTOKO. Session management belongs to Navi. Core is infrastructure.

---

## What Core Owns

- PTY spawn and lifecycle management
- Terminal emulation (VTE/ANSI parsing, cell grid state)
- Rendering pipeline (GPU and CPU paths)
- Input handling (keyboard, mouse, paste)
- Configuration model (`.lain/`, user-global, schema validation)
- Storage (state, cache, data directories)
- CLI interface (`lain` command)
- Font rendering and shaping
- Theme engine

---

## What Core Does Not Own

- Session multiplexing (Navi)
- Agent lifecycle (MAGGI / Navi)
- Security enforcement (MOTOKO)
- External API (THE WIRED)
- Model invocation (MAGGI)

---

## Open Questions

1. **Rendering architecture.** How do GPU and CPU paths coexist behind a trait? How does the overlay compositor work?
2. **Config hot-reload.** Which config changes take effect immediately vs. requiring a restart?
3. ~~**Event bus.**~~ ✓ Resolved. Hybrid model: direct trait calls for request/response, two-tier event bus for observation (mandatory audit writes + observable broadcast), dedicated channel for MOTOKO. See ADR-006 and `systems-architecture.md`.
