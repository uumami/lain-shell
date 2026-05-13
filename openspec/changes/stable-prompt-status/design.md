## Context

lain-shell launches the user's shell with an app-specific `ZDOTDIR` generated from `crates/lain-core/src/pty.rs`. That generated zsh configuration currently sources the user's normal `~/.zshrc`, then overrides `PROMPT`.

The prompt uses zsh conditional prompt expansion to color `lambda_` green on success and red on failure. This correctly reflects exit status, but it makes the `lambda_` identity marker appear unstable after commands such as `htop` exit non-zero or are interrupted.

## Goals / Non-Goals

**Goals:**

- Keep `lambda_` visually stable across successful and failed commands.
- Preserve a compact non-zero exit status indicator.
- Keep the existing prompt structure: directory, git branch, colon, prompt marker.
- Keep the behavior local to lain-shell's generated zsh configuration.

**Non-Goals:**

- Do not change the user's normal `~/.zshrc`.
- Do not change renderer color handling.
- Do not introduce prompt configuration files or user-facing settings in this change.
- Do not replace the current zsh prompt approach with another prompt framework.

## Decisions

Use a separate failure marker instead of recoloring `lambda_`.

The `lambda_` marker is the shell identity, so it should remain the same color regardless of the last command's exit status. A small status marker before the colon keeps failure feedback visible without making the identity glyph change.

Keep zsh prompt expansion.

The existing generated rc file already depends on zsh and uses zsh prompt escapes. The smallest coherent change is to adjust the prompt string and add a tiny prompt-status helper or conditional expansion.

Use `!` for non-zero status.

The marker is ASCII, compact, and readable in the existing prompt. It avoids requiring additional glyph coverage and keeps the failure state separate from the lambda marker.

## Risks / Trade-offs

- The failure marker may be too subtle for some users -> keep it adjacent to the colon so it appears near the command entry point.
- A literal `!` can visually resemble shell history expansion -> it is prompt text only, not typed input.
- zsh prompt escaping is easy to get wrong -> verify with both success (`true`) and failure (`false`) commands.
