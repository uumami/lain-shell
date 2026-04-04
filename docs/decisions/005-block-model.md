# ADR-005: Block Model (Semantic Scrollback) in Core

## Status

Accepted

## Date

2026-04-03

## Context

Traditional terminals treat scrollback as a continuous stream of bytes. An agent or user wanting to query "what was the output of the third command" must scrape raw text and guess where commands begin and end.

Warp introduced the "block model" — each command and its output form a discrete, structured unit. This idea is powerful but Warp is closed source and tied to its cloud platform.

Modern shells already emit shell integration escape sequences (OSC 133) that mark command boundaries:
- `OSC 133 ; A` — prompt start
- `OSC 133 ; B` — command start (user pressed enter)
- `OSC 133 ; C` — command output start
- `OSC 133 ; D ; <exit_code>` — command finished

bash 5.x, zsh, and fish support these. `alacritty_terminal` can parse them.

## Decision

Implement the block model as a Core responsibility. Core maintains a structured command history alongside the raw cell grid.

Each command block contains:
- The command text
- The working directory
- The output (line range in scrollback)
- Exit code
- Timestamp and duration

This structured history is queryable by:
- MAGGI (for context: "what did the last command output?")
- Agents via THE WIRED (for structured interaction with the terminal)
- Navi (for visual features: collapse/expand blocks, block-level selection)
- The user via CLI (`lain history query --failed --last 5`)

### Why Core, not Navi

The block model is a property of terminal state — it's built on VTE parsing of shell integration sequences. It lives in the same layer as the cell grid and scrollback buffer, which are Core responsibilities. Navi consumes block information but doesn't produce it.

## Consequences

### Enables

- Agents can query structured command history without text scraping
- `lain history query` for structured introspection
- Block-level visual features (collapse, expand, select, copy)
- MAGGI can reason about command outcomes structurally
- Differentiation from every other terminal (none expose structured command history to agents)

### Costs

- Additional state tracking in Core alongside the cell grid
- Must handle shells that don't emit OSC 133 (graceful fallback to unstructured scrollback)
- Memory overhead for maintaining block metadata

### Risks

- Shells without OSC 133 get no block structure — the feature degrades gracefully but unevenly
- Mitigation: Document how to enable shell integration for each major shell. MAGGI can guide users through setup.

## Alternatives Considered

### No block model

Rely on raw scrollback. Agents scrape text. Simpler, but gives up a major differentiation point for agent workflows.

### Block model in Navi

Navi as the session layer could maintain blocks. Rejected because block detection depends on VTE parsing (Core's domain), and splitting this across quanta creates unnecessary coupling.

### Heuristic prompt detection (for shells without OSC 133)

Detect command boundaries by recognizing prompt patterns. Fragile, prompt-dependent, and prone to false positives. Worth exploring as a future fallback but not the primary mechanism.
