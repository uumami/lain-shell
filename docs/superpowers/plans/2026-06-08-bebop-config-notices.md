# BEBOP Config + Notices Implementation Plan (Plan 5 of N)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add BEBOP's typed notice/status seam plus a human-readable TOML config loader with last-good reload behavior, then surface GPU fallback as a visible degradation path in `bebop run`.

**Architecture:** `lain-types` owns toolkit-neutral `Notice`, `TermStatus`, and related enums. `terminal-core::config` owns TOML parsing, defaults, validation, and a polling-friendly `ConfigReloader` that keeps the last-good config on bad reloads. `Terminal` stores drained notices and status; `bebop` loads config at startup, watches it when a config path is provided, applies live font-size changes, and falls back from GPU to CPU with a typed notice/status instead of panicking.

**Tech Stack:** Rust, `serde`, `toml`, existing `lain-types` / `terminal-core`, winit event-loop proxy for config-change wakeups.

**Source of truth:** `docs/superpowers/specs/2026-06-08-bebop-terminal-core-design.md` section 6 (TOML config + hot reload), section 7 (`Notice`, `TermStatus` seam), section 8 (visible degradation; bad config keeps last-good). This plan intentionally does not build NAVI toast/badge UI, action bindings, OSC 133 marks, or a full theme renderer.

---

## File structure

```
crates/lain-types/src/lib.rs             # add Severity/Notice/NoticeCode/NoticeAction/TermStatus
crates/terminal-core/Cargo.toml          # add serde + toml
crates/terminal-core/src/config.rs       # new config types, parser, reloader, tests
crates/terminal-core/src/terminal.rs     # notice/status storage and accessors
crates/terminal-core/src/lib.rs          # mod config; re-export config + notice/status types
crates/terminal-core/src/bin/bebop.rs    # --config, config watcher, GPU fallback notice/status
docs/superpowers/specs/2026-06-08-bebop-config-notices-results.md
```

---

## Task 1: Typed notice/status seam in `lain-types`

**Files:**
- Modify: `crates/lain-types/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add this test to the existing `mod tests`:

```rust
    #[test]
    fn notice_and_status_construct_and_compare() {
        let notice = Notice {
            severity: Severity::Warn,
            code: NoticeCode::GpuFallback,
            message: "running on CPU renderer".to_string(),
            action: Some(NoticeAction::Dismiss),
        };
        assert_eq!(notice.severity, Severity::Warn);
        assert_eq!(notice.code, NoticeCode::GpuFallback);
        assert_eq!(notice.action, Some(NoticeAction::Dismiss));
        assert_eq!(TermStatus::Running, TermStatus::Running);
        assert_eq!(
            TermStatus::Degraded { reason: DegradedReason::GpuUnavailable },
            TermStatus::Degraded { reason: DegradedReason::GpuUnavailable }
        );
        assert_ne!(TermStatus::Closed { code: Some(1) }, TermStatus::Running);
    }
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cargo test -p lain-types notice_and_status_construct_and_compare`

Expected: compile failure because the types do not exist.

- [ ] **Step 3: Add the types**

Add after `InputOutcome`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeCode {
    GpuFallback,
    ConfigRejected,
    ChildExited(i32),
    DeviceLost,
    FontFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeAction {
    RestartPane,
    ReloadConfig,
    Dismiss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub severity: Severity,
    pub code: NoticeCode,
    pub message: String,
    pub action: Option<NoticeAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradedReason {
    GpuUnavailable,
    DeviceLost,
    FontFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermStatus {
    Running,
    Closed { code: Option<i32> },
    Degraded { reason: DegradedReason },
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p lain-types notice_and_status_construct_and_compare`

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/lain-types/src/lib.rs
git commit -m "feat(lain-types): add typed terminal notices and status"
```

---

## Task 2: TOML config parser and last-good reloader

**Files:**
- Modify: `crates/terminal-core/Cargo.toml`
- Create: `crates/terminal-core/src/config.rs`
- Modify: `crates/terminal-core/src/lib.rs`

- [ ] **Step 1: Add dependencies**

Add to `crates/terminal-core/Cargo.toml`:

```toml
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

- [ ] **Step 2: Create the config module with tests and implementation**

Create `crates/terminal-core/src/config.rs` with:

```rust
use lain_types::{Notice, NoticeAction, NoticeCode, Severity};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub struct TermConfig {
    pub font: FontConfig,
    pub cursor: CursorConfig,
    pub scrollback_lines: usize,
    pub shell: Option<String>,
    pub render_backend: RenderBackendPreference,
    pub vsync: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub fallback: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CursorConfig {
    pub style: CursorStyle,
    pub blink: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Underline,
    Beam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackendPreference {
    Auto,
    Gpu,
    Cpu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Read(String),
    Parse(String),
    Validate(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigReload {
    Unchanged,
    Applied(TermConfig),
    Rejected(Notice),
}

pub struct ConfigReloader {
    path: PathBuf,
    last_seen: Option<SystemTime>,
    current: TermConfig,
}

impl Default for TermConfig {
    fn default() -> Self {
        TermConfig {
            font: FontConfig {
                family: "DejaVu Sans Mono".to_string(),
                size: 14.0,
                fallback: Vec::new(),
            },
            cursor: CursorConfig { style: CursorStyle::Block, blink: true },
            scrollback_lines: 10_000,
            shell: None,
            render_backend: RenderBackendPreference::Auto,
            vsync: true,
        }
    }
}

pub fn parse_toml(input: &str) -> Result<TermConfig, ConfigError> {
    RawTermConfig::from_toml(input)?.finish()
}

impl ConfigReloader {
    pub fn new(path: impl Into<PathBuf>, current: TermConfig) -> Self {
        ConfigReloader { path: path.into(), last_seen: None, current }
    }

    pub fn current(&self) -> &TermConfig {
        &self.current
    }

    pub fn reload_now(&mut self) -> ConfigReload {
        match load_from_path(&self.path) {
            Ok(cfg) => {
                self.current = cfg.clone();
                self.last_seen = modified_at(&self.path).ok().flatten();
                ConfigReload::Applied(cfg)
            }
            Err(err) => ConfigReload::Rejected(config_rejected_notice(err)),
        }
    }

    pub fn reload_if_changed(&mut self) -> ConfigReload {
        let modified = match modified_at(&self.path) {
            Ok(m) => m,
            Err(err) => return ConfigReload::Rejected(config_rejected_notice(err)),
        };
        if modified.is_some() && modified == self.last_seen {
            return ConfigReload::Unchanged;
        }
        self.reload_now()
    }
}

pub fn load_from_path(path: &Path) -> Result<TermConfig, ConfigError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::Read(format!("{}: {e}", path.display())))?;
    parse_toml(&text)
}

pub fn config_rejected_notice(err: ConfigError) -> Notice {
    Notice {
        severity: Severity::Warn,
        code: NoticeCode::ConfigRejected,
        message: format!("config rejected; keeping last-good settings: {err}"),
        action: Some(NoticeAction::ReloadConfig),
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Read(s) => write!(f, "read error: {s}"),
            ConfigError::Parse(s) => write!(f, "parse error: {s}"),
            ConfigError::Validate(s) => write!(f, "validation error: {s}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn modified_at(path: &Path) -> Result<Option<SystemTime>, ConfigError> {
    std::fs::metadata(path)
        .map_err(|e| ConfigError::Read(format!("{}: {e}", path.display())))
        .and_then(|m| m.modified().map(Some).map_err(|e| ConfigError::Read(e.to_string())))
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTermConfig {
    font: Option<RawFontConfig>,
    cursor: Option<RawCursorConfig>,
    scrollback_lines: Option<usize>,
    shell: Option<String>,
    render_backend: Option<String>,
    vsync: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFontConfig {
    family: Option<String>,
    size: Option<f32>,
    fallback: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCursorConfig {
    style: Option<String>,
    blink: Option<bool>,
}

impl RawTermConfig {
    fn from_toml(input: &str) -> Result<Self, ConfigError> {
        toml::from_str(input).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    fn finish(self) -> Result<TermConfig, ConfigError> {
        let mut cfg = TermConfig::default();
        if let Some(font) = self.font {
            if let Some(family) = font.family {
                if family.trim().is_empty() {
                    return Err(ConfigError::Validate("font.family must not be empty".to_string()));
                }
                cfg.font.family = family;
            }
            if let Some(size) = font.size {
                if !size.is_finite() || size < 6.0 || size > 96.0 {
                    return Err(ConfigError::Validate("font.size must be between 6 and 96".to_string()));
                }
                cfg.font.size = size;
            }
            if let Some(fallback) = font.fallback {
                cfg.font.fallback = fallback;
            }
        }
        if let Some(cursor) = self.cursor {
            if let Some(style) = cursor.style {
                cfg.cursor.style = match style.as_str() {
                    "block" => CursorStyle::Block,
                    "underline" => CursorStyle::Underline,
                    "beam" => CursorStyle::Beam,
                    _ => {
                        return Err(ConfigError::Validate(
                            "cursor.style must be block, underline, or beam".to_string(),
                        ))
                    }
                };
            }
            if let Some(blink) = cursor.blink {
                cfg.cursor.blink = blink;
            }
        }
        if let Some(lines) = self.scrollback_lines {
            if lines > 1_000_000 {
                return Err(ConfigError::Validate(
                    "scrollback_lines must be <= 1000000".to_string(),
                ));
            }
            cfg.scrollback_lines = lines;
        }
        if let Some(shell) = self.shell {
            if shell.trim().is_empty() {
                return Err(ConfigError::Validate("shell must not be empty".to_string()));
            }
            cfg.shell = Some(shell);
        }
        if let Some(backend) = self.render_backend {
            cfg.render_backend = match backend.as_str() {
                "auto" => RenderBackendPreference::Auto,
                "gpu" => RenderBackendPreference::Gpu,
                "cpu" => RenderBackendPreference::Cpu,
                _ => {
                    return Err(ConfigError::Validate(
                        "render_backend must be auto, gpu, or cpu".to_string(),
                    ))
                }
            };
        }
        if let Some(vsync) = self.vsync {
            cfg.vsync = vsync;
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_full_toml_config() {
        let cfg = parse_toml(
            r#"
shell = "/bin/zsh"
render_backend = "cpu"
vsync = false
scrollback_lines = 5000

[font]
family = "Commit Mono"
size = 16.5
fallback = ["Symbols Nerd Font"]

[cursor]
style = "beam"
blink = false
"#,
        )
        .unwrap();
        assert_eq!(cfg.shell.as_deref(), Some("/bin/zsh"));
        assert_eq!(cfg.render_backend, RenderBackendPreference::Cpu);
        assert!(!cfg.vsync);
        assert_eq!(cfg.scrollback_lines, 5000);
        assert_eq!(cfg.font.family, "Commit Mono");
        assert_eq!(cfg.font.size, 16.5);
        assert_eq!(cfg.font.fallback, vec!["Symbols Nerd Font"]);
        assert_eq!(cfg.cursor.style, CursorStyle::Beam);
        assert!(!cfg.cursor.blink);
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(matches!(
            parse_toml("render_backend = \"quantum\""),
            Err(ConfigError::Validate(_))
        ));
        assert!(matches!(
            parse_toml("[font]\nsize = 2.0"),
            Err(ConfigError::Validate(_))
        ));
    }

    #[test]
    fn reloader_keeps_last_good_after_bad_reload() {
        let path = temp_path("bebop-config-bad");
        fs::write(&path, "shell = \"/bin/sh\"\n").unwrap();
        let mut reloader = ConfigReloader::new(&path, TermConfig::default());
        let applied = reloader.reload_now();
        assert!(matches!(applied, ConfigReload::Applied(_)));
        assert_eq!(reloader.current().shell.as_deref(), Some("/bin/sh"));

        fs::write(&path, "render_backend = \"wrong\"\n").unwrap();
        let rejected = reloader.reload_now();
        assert!(matches!(rejected, ConfigReload::Rejected(_)));
        assert_eq!(reloader.current().shell.as_deref(), Some("/bin/sh"));
        let _ = fs::remove_file(path);
    }

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("{label}-{nanos}.toml"))
    }
}
```

- [ ] **Step 3: Wire the module and re-exports**

In `crates/terminal-core/src/lib.rs`, add `mod config;` and re-export:

```rust
pub use config::{
    config_rejected_notice, load_from_path, parse_toml, ConfigError, ConfigReload,
    ConfigReloader, CursorConfig, CursorStyle, FontConfig, RenderBackendPreference, TermConfig,
};
pub use lain_types::{
    ByteStream, Cursor, Damage, DegradedReason, GridSnapshot, InputOutcome, Key, KeyInput,
    Modifiers, NamedKey, Notice, NoticeAction, NoticeCode, Severity, TermStatus,
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p terminal-core config::`

Expected: config tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/Cargo.toml crates/terminal-core/src/config.rs crates/terminal-core/src/lib.rs Cargo.lock
git commit -m "feat(terminal-core): add TOML terminal config loader"
```

---

## Task 3: Terminal notice/status accessors

**Files:**
- Modify: `crates/terminal-core/src/terminal.rs`

- [ ] **Step 1: Write the failing tests**

Add to `crates/terminal-core/src/terminal.rs` tests:

```rust
    #[test]
    fn notices_drain_and_status_is_reported() {
        let mut t = Terminal::new(20, 5);
        assert_eq!(t.status(), lain_types::TermStatus::Running);
        t.push_notice(lain_types::Notice {
            severity: lain_types::Severity::Warn,
            code: lain_types::NoticeCode::ConfigRejected,
            message: "bad config".to_string(),
            action: Some(lain_types::NoticeAction::ReloadConfig),
        });
        let notices: Vec<_> = t.notices().collect();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].code, lain_types::NoticeCode::ConfigRejected);
        assert_eq!(t.notices().count(), 0);

        t.set_status(lain_types::TermStatus::Degraded {
            reason: lain_types::DegradedReason::GpuUnavailable,
        });
        assert_eq!(
            t.status(),
            lain_types::TermStatus::Degraded {
                reason: lain_types::DegradedReason::GpuUnavailable,
            }
        );
    }
```

- [ ] **Step 2: Run, verify failure**

Run: `cargo test -p terminal-core --lib terminal::tests::notices_drain_and_status_is_reported`

Expected: compile failure because methods/fields do not exist.

- [ ] **Step 3: Implement storage and accessors**

Add fields to `Terminal`:

```rust
    notices: Vec<lain_types::Notice>,
    status: lain_types::TermStatus,
```

Initialize in `Terminal::new`:

```rust
        Terminal {
            term,
            parser: Processor::new(),
            writes,
            dims,
            notices: Vec::new(),
            status: lain_types::TermStatus::Running,
        }
```

Add methods inside `impl Terminal`:

```rust
    pub fn push_notice(&mut self, notice: lain_types::Notice) {
        self.notices.push(notice);
    }

    pub fn notices(&mut self) -> impl Iterator<Item = lain_types::Notice> {
        std::mem::take(&mut self.notices).into_iter()
    }

    pub fn status(&self) -> lain_types::TermStatus {
        self.status.clone()
    }

    pub fn set_status(&mut self, status: lain_types::TermStatus) {
        self.status = status;
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p terminal-core --lib terminal::`

Expected: terminal tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/src/terminal.rs
git commit -m "feat(terminal-core): expose terminal notices and status"
```

---

## Task 4: Wire config reload and GPU fallback into `bebop`

**Files:**
- Modify: `crates/terminal-core/src/bin/bebop.rs`

- [ ] **Step 1: Update imports and user events**

Add `PathBuf`, config types, notices/status types, and a `Config` user event. Parse `--config <path>` and store `TermConfig`, optional `ConfigReloader`, and `font_size` on `App`.

- [ ] **Step 2: Add startup config loading**

If `--config <path>` is present, load it with `load_from_path`. On success use it and create `ConfigReloader`. On failure keep `TermConfig::default()` and store a `ConfigRejected` notice to push into the terminal after construction.

- [ ] **Step 3: Add config watcher**

When a config path is present, spawn a thread that polls metadata modified time every 500 ms and sends `UserEvent::ConfigChanged`. The event handler calls `ConfigReloader::reload_if_changed`.

- [ ] **Step 4: Apply config live**

On `ConfigReload::Applied`, if font size changed, update `cell_w`/`cell_h`, rebuild the active renderer with the new size, and resize terminal/PTY to the current window. On `ConfigReload::Rejected`, push the notice into the terminal. Backend preference changes are startup-only for this cycle.

- [ ] **Step 5: Replace GPU startup panics with CPU fallback**

If the selected backend is `Auto` or `Gpu` and GPU setup fails, initialize the CPU backend, push `NoticeCode::GpuFallback`, and set `TermStatus::Degraded { reason: DegradedReason::GpuUnavailable }`. Keep `--cpu` as an explicit CPU override.

- [ ] **Step 6: Build and smoke**

Run:

```bash
cargo build -p terminal-core --bin bebop
cargo clippy --bin bebop
DISPLAY=:1 timeout 30 cargo run -p terminal-core --bin bebop -- run --smoke
```

Expected: build and clippy pass; smoke prints `SMOKE_OK` or `SMOKE_SKIP` on a headless environment.

- [ ] **Step 7: Commit**

```bash
git add crates/terminal-core/src/bin/bebop.rs
git commit -m "feat(terminal-core): bebop loads config and surfaces GPU fallback"
```

---

## Task 5: Results note and full verification

**Files:**
- Create: `docs/superpowers/specs/2026-06-08-bebop-config-notices-results.md`

- [ ] **Step 1: Write the note**

Capture what shipped, verification output, and deferrals:

- Typed notices/status and drainable `Terminal::notices`.
- TOML config parser and last-good reloader.
- `bebop run --config <path>` and config-change wakeups.
- GPU fallback to CPU with notice/status.
- Deferred: NAVI toast/badge UI, complete theme application, structural hot recreation for backend/shell changes, OSC 133 marks, action bindings.

- [ ] **Step 2: Run full suite**

Run:

```bash
cargo test
cargo clippy --all-targets
```

Expected: all tests pass and clippy clean.

- [ ] **Step 3: Commit and push**

```bash
git add docs/superpowers/specs/2026-06-08-bebop-config-notices-results.md
git commit -m "docs(bebop): Plan 5 config and notices results"
git push origin new-seed
```

