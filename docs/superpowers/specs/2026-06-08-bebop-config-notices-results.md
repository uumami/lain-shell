# BEBOP Plan 5 -- config + notices/status: RESULTS

> Status: DONE. Full workspace tests pass, `cargo clippy --all-targets` clean,
> and `bebop run --smoke` returns `SMOKE_OK` on `DISPLAY=:1`.

## What shipped

**Typed terminal notices/status in `lain-types`:**

- `Severity` (`Info`, `Warn`, `Error`)
- `NoticeCode` (`GpuFallback`, `ConfigRejected`, `ChildExited`, `DeviceLost`,
  `FontFallback`)
- `NoticeAction` (`RestartPane`, `ReloadConfig`, `Dismiss`)
- `Notice { severity, code, message, action }`
- `DegradedReason` and `TermStatus`

These are backend-neutral seam types. NAVI can render them later without parsing
strings from `terminal-core`.

**TOML config loader in `terminal-core`:**

- `TermConfig` with font, cursor, scrollback, shell, render backend preference,
  and vsync fields.
- `parse_toml` / `load_from_path` with validation and defaults.
- `ConfigReloader`, which keeps the last-good config after a bad reload and
  emits a typed `ConfigRejected` notice.
- Re-exported config API from `terminal_core`.

**Terminal notice/status channel:**

- `Terminal::push_notice`
- `Terminal::notices`, draining notices exactly once.
- `Terminal::status`
- `Terminal::set_status`

**`bebop run` config and degradation behavior:**

- New `--config <path>` option.
- Startup config load with default fallback on bad config.
- Lightweight polling watcher that wakes the winit event loop when the config
  file changes.
- Live font-size reload by rebuilding the active renderer and resizing terminal
  geometry.
- `--cpu` still overrides renderer selection.
- GPU startup no longer panics when adapter/device acquisition fails. BEBOP
  falls back to CPU, records `NoticeCode::GpuFallback`, sets
  `TermStatus::Degraded { GpuUnavailable }`, and logs the notice in the dev
  binary.

## Verification

Commands run fresh on this branch:

```bash
cargo test
cargo clippy --all-targets
cargo build -p terminal-core --bin bebop
cargo clippy --bin bebop
DISPLAY=:1 timeout 30 cargo run -p terminal-core --bin bebop -- run --smoke
```

Observed:

- `cargo test`: 34 runtime tests passed; doc-tests passed.
- `cargo clippy --all-targets`: clean.
- `bebop run --smoke`: `SMOKE_OK` with the Intel HD Graphics 630 Vulkan adapter.

## Deferrals

- NAVI toast/badge rendering for `Notice` and `TermStatus`.
- Full theme/palette/cursor rendering application.
- Structural hot recreation for shell/backend preference changes.
- Device-lost recovery during an already-running GPU session.
- OSC 133 marks and `bebop shell-integration` (Plan 6).
- Action bindings / shared keymap resolver (Plan 4b).

## Next

Plan 6 should add OSC 133 command marks and shell-integration snippets. Plan 7
should add the automated performance regression gate.
