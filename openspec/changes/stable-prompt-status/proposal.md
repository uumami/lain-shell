## Why

The `lambda_` prompt marker currently changes color based on the previous command exit status. That makes normal failures or interrupted full-screen programs such as `htop` look like the shell identity changed state, which is visually confusing.

## What Changes

- Keep the `lambda_` prompt marker on a stable brand color regardless of the previous command exit status.
- Move non-zero exit status feedback into a separate compact marker near the prompt metadata.
- Preserve the existing directory and git branch prompt layout.
- Keep the prompt local to lain-shell's generated zsh configuration; do not modify the user's normal `~/.zshrc`.

## Capabilities

### New Capabilities

- `terminal-prompt`: Defines the app-specific shell prompt presentation used inside lain-shell.

### Modified Capabilities

- None.

## Impact

- `crates/lain-core/src/pty.rs`: generated `ZDOTDIR` zsh configuration and prompt string.
- No dependency changes.
- No terminal renderer changes expected.
