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
}
