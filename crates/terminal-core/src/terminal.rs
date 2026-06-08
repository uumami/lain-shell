//! Terminal: alacritty_terminal grid + VTE, behind feed()/snapshot().

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use lain_types::{Cursor, Damage, GridSnapshot};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
struct Dims {
    cols: usize,
    lines: usize,
}

impl Dimensions for Dims {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// alacritty emits replies (cursor reports, DA) via the event listener. We use a
/// LOCAL newtype (orphan rule: cannot impl the foreign `EventListener` for
/// `Arc<...>` directly) holding a shared buffer the caller drains and writes back.
#[derive(Clone)]
struct EventProxy(Arc<Mutex<Vec<u8>>>);

impl EventListener for EventProxy {
    fn send_event(&self, ev: Event) {
        if let Event::PtyWrite(s) = ev {
            self.0.lock().unwrap().extend_from_slice(s.as_bytes());
        }
    }
}

pub struct Terminal {
    term: Term<EventProxy>,
    parser: Processor,
    writes: Arc<Mutex<Vec<u8>>>,
    dims: Dims,
}

impl Terminal {
    pub fn new(cols: usize, lines: usize) -> Self {
        let dims = Dims { cols: cols.max(1), lines: lines.max(1) };
        let writes = Arc::new(Mutex::new(Vec::new()));
        let term = Term::new(Config::default(), &dims, EventProxy(writes.clone()));
        Terminal { term, parser: Processor::new(), writes, dims }
    }

    /// Advance the parser with PTY bytes. Returns conservative damage.
    pub fn feed(&mut self, bytes: &[u8]) -> Damage {
        for &b in bytes {
            self.parser.advance(&mut self.term, b);
        }
        if bytes.is_empty() {
            Damage::none()
        } else {
            Damage::full()
        }
    }

    /// Bytes alacritty wants written back to the PTY (replies). Drains them.
    pub fn take_pty_writes(&mut self) -> Vec<u8> {
        std::mem::take(&mut *self.writes.lock().unwrap())
    }

    pub fn snapshot(&self) -> GridSnapshot {
        let grid = self.term.grid();
        let mut cells = Vec::with_capacity(self.dims.cols * self.dims.lines);
        for l in 0..self.dims.lines {
            for c in 0..self.dims.cols {
                let ch = grid[Line(l as i32)][Column(c)].c;
                cells.push(if ch == '\u{0}' { ' ' } else { ch });
            }
        }
        let point = self.term.grid().cursor.point;
        GridSnapshot {
            cols: self.dims.cols,
            lines: self.dims.lines,
            cells,
            cursor: Cursor { line: point.line.0.max(0) as usize, col: point.column.0 },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_lands_in_row_zero() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"hello");
        let s = t.snapshot();
        assert_eq!(s.row(0), "hello");
        assert_eq!(s.cursor, Cursor { line: 0, col: 5 });
    }

    #[test]
    fn sgr_color_does_not_appear_as_text() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"\x1b[31mred\x1b[0m");
        assert_eq!(t.snapshot().row(0), "red");
    }

    #[test]
    fn carriage_return_and_newline_move_down() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"one\r\ntwo");
        let s = t.snapshot();
        assert_eq!(s.row(0), "one");
        assert_eq!(s.row(1), "two");
    }

    #[test]
    fn empty_feed_reports_no_damage() {
        let mut t = Terminal::new(10, 3);
        assert!(!t.feed(b"").full);
        assert!(t.feed(b"x").full);
    }
}
