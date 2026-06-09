//! Shared seam types for the lain-shell workspace.

use std::fmt;

/// A source/sink of terminal bytes (local PTY, ssh, or an EVA cage relay).
/// `read` is blocking and is expected to run off the UI thread.
pub trait ByteStream: Send {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    fn resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

/// Keyboard modifier state, backend-neutral. `logo` = Super/Command/Windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub logo: bool,
}

/// A functional (non-text) key. `F(n)` is a function key (F1..F12 used now).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),
}

/// A key press, backend-neutral: either a produced character (post-shift, e.g.
/// '!' for Shift+1) or a functional key. The host translates its native key
/// events (winit, etc.) into this; `terminal-core` never sees a toolkit type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Named(NamedKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
    pub key: Key,
    pub mods: Modifiers,
}

/// Result of feeding a key press to a terminal: bytes to write to the PTY, or
/// unhandled (the host may bind it to an action -- a future `Action(..)` variant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOutcome {
    Bytes(Vec<u8>),
    Unhandled,
}

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

/// A flat, render-agnostic view of the visible grid. Chars only in Plan 1;
/// color/attrs arrive with the render plan.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSnapshot {
    pub cols: usize,
    pub lines: usize,
    pub cells: Vec<char>, // row-major, len == cols * lines
    pub cursor: Cursor,
}

impl GridSnapshot {
    /// The given visible line as a string with trailing blanks trimmed.
    pub fn row(&self, line: usize) -> String {
        let start = line * self.cols;
        let end = start + self.cols;
        self.cells[start..end].iter().collect::<String>().trim_end().to_string()
    }
}

/// What changed since the last feed. Plan 1 reports conservatively (`full`);
/// per-line precision arrives with the reactive render loop (Plan 2).
#[derive(Debug, Clone, Default)]
pub struct Damage {
    pub full: bool,
    pub lines: Vec<usize>,
}

impl Damage {
    pub fn none() -> Self {
        Damage { full: false, lines: Vec::new() }
    }
    pub fn full() -> Self {
        Damage { full: true, lines: Vec::new() }
    }
}

#[derive(Debug)]
pub enum TermError {
    Pty(std::io::Error),
    Spawn(String),
}

impl fmt::Display for TermError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermError::Pty(e) => write!(f, "pty error: {e}"),
            TermError::Spawn(s) => write!(f, "spawn error: {s}"),
        }
    }
}

impl std::error::Error for TermError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_row_trims_and_indexes() {
        let snap = GridSnapshot {
            cols: 4,
            lines: 2,
            cells: vec!['h', 'i', ' ', ' ', ' ', ' ', ' ', ' '],
            cursor: Cursor { line: 0, col: 2 },
        };
        assert_eq!(snap.row(0), "hi");
        assert_eq!(snap.row(1), "");
        assert_eq!(snap.cursor, Cursor { line: 0, col: 2 });
    }

    #[test]
    fn damage_full_is_not_empty() {
        let d = Damage::full();
        assert!(d.full);
    }

    #[test]
    fn key_input_constructs_and_compares() {
        let a = KeyInput {
            key: Key::Named(NamedKey::Up),
            mods: Modifiers { ctrl: true, ..Modifiers::default() },
        };
        let b = KeyInput {
            key: Key::Named(NamedKey::Up),
            mods: Modifiers { ctrl: true, ..Modifiers::default() },
        };
        assert_eq!(a, b);
        assert!(a.mods.ctrl && !a.mods.alt);
        assert_eq!(Key::Char('x'), Key::Char('x'));
        assert_eq!(NamedKey::F(5), NamedKey::F(5));
        assert_eq!(InputOutcome::Bytes(vec![0x1b]), InputOutcome::Bytes(vec![0x1b]));
        assert_ne!(InputOutcome::Bytes(vec![0x1b]), InputOutcome::Unhandled);
    }

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
}
