# BEBOP Terminal-Core — Parse Core Implementation Plan (Plan 1 of N)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the headless parse core of BEBOP — spawn a real shell on a PTY, parse its output into a grid/scrollback with `alacritty_terminal`, expose `feed(bytes) -> Damage` + a grid snapshot, with a bounded reader that survives an adversarial flood.

**Architecture:** A cargo workspace with `lain-types` (the trait seam: `ByteStream`, snapshot/damage types) and `terminal-core` (codename BEBOP). `LocalPty` implements `ByteStream` over `portable-pty`; `ReaderPump` runs the blocking read off-thread behind a **bounded** `sync_channel` (backpressure → no unbounded heap growth under flood); `Terminal` wraps `alacritty_terminal` and turns drained bytes into grid state. No window, no GPU — that is Plan 2.

**Tech Stack:** Rust 1.94, `alacritty_terminal` 0.24, `portable-pty` 0.8. (Render deps — wgpu/glyphon/softbuffer — are deliberately absent here.)

**Source of truth:** `docs/superpowers/specs/2026-06-08-bebop-terminal-core-design.md` (esp. §1 scope, §3 concurrency, §3.1 flood resistance, §5 terminal model, §7 API seam). This plan implements build-order step 1 from §11.

---

## File structure

```
Cargo.toml                         # workspace
crates/
  lain-types/
    Cargo.toml                     # no deps
    src/lib.rs                     # ByteStream trait; GridSnapshot/GridCell/Cursor; Damage; TermError
  terminal-core/
    Cargo.toml                     # deps: lain-types(path), alacritty_terminal, portable-pty
    src/lib.rs                     # re-exports
    src/terminal.rs                # Terminal: alacritty wrap, feed()->Damage, resize, snapshot
    src/pty.rs                     # LocalPty: ByteStream over portable-pty; clone_reader()
    src/pump.rs                    # ReaderPump: bounded off-thread reader (backpressure)
```

Responsibilities, one per file: `lain-types` is pure data + traits (no logic). `terminal.rs` owns grid state. `pty.rs` owns the OS PTY. `pump.rs` owns the read-thread + backpressure. Each is testable alone.

---

## Task 1: Workspace + crate scaffold

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/lain-types/Cargo.toml`
- Create: `crates/lain-types/src/lib.rs`
- Create: `crates/terminal-core/Cargo.toml`
- Create: `crates/terminal-core/src/lib.rs`

- [ ] **Step 1: Write the workspace manifest**

`Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/lain-types", "crates/terminal-core"]

[workspace.package]
edition = "2021"
version = "0.0.0"
publish = false
```

- [ ] **Step 2: Write the `lain-types` manifest**

`crates/lain-types/Cargo.toml`:
```toml
[package]
name = "lain-types"
edition.workspace = true
version.workspace = true
publish.workspace = true
```

- [ ] **Step 3: Write the `terminal-core` manifest**

`crates/terminal-core/Cargo.toml`:
```toml
[package]
name = "terminal-core"
edition.workspace = true
version.workspace = true
publish.workspace = true

[dependencies]
lain-types = { path = "../lain-types" }
alacritty_terminal = "0.24"
portable-pty = "0.8"
```

- [ ] **Step 4: Write placeholder lib roots**

`crates/lain-types/src/lib.rs`:
```rust
//! Shared seam types for the lain-shell workspace.
```

`crates/terminal-core/src/lib.rs`:
```rust
//! terminal-core (codename BEBOP): single-terminal primitive.
```

- [ ] **Step 5: Verify the workspace builds**

Run: `cargo build`
Expected: `Finished` with no errors (downloads alacritty_terminal + portable-pty on first run).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/
git commit -m "scaffold: cargo workspace with lain-types + terminal-core"
```

---

## Task 2: `lain-types` — seam types

**Files:**
- Modify: `crates/lain-types/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/lain-types/src/lib.rs`:
```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lain-types`
Expected: FAIL — `GridSnapshot`, `Cursor`, `Damage` not found.

- [ ] **Step 3: Write the types**

Replace `crates/lain-types/src/lib.rs` body (keep the `//!` line, put this above the `#[cfg(test)]` block):
```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lain-types`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/lain-types/src/lib.rs
git commit -m "feat(lain-types): ByteStream trait + GridSnapshot/Damage/TermError"
```

---

## Task 3: `Terminal` — alacritty wrap, feed + snapshot

**Files:**
- Create: `crates/terminal-core/src/terminal.rs`
- Modify: `crates/terminal-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/terminal-core/src/terminal.rs`:
```rust
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
```

- [ ] **Step 2: Wire the module**

Replace `crates/terminal-core/src/lib.rs`:
```rust
//! terminal-core (codename BEBOP): single-terminal primitive.

mod terminal;

pub use terminal::Terminal;
```

- [ ] **Step 3: Run tests to verify they fail then pass**

Run: `cargo test -p terminal-core terminal::`
Expected: compiles, 4 tests PASS. (If `cursor.point` access differs in your patch of alacritty_terminal 0.24, the grid cursor is `term.grid().cursor.point` — a `Point { line: Line(i32), column: Column(usize) }`.)

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/src/lib.rs crates/terminal-core/src/terminal.rs
git commit -m "feat(terminal-core): Terminal wrap over alacritty (feed/snapshot)"
```

---

## Task 4: `Terminal::resize` + line wrap

**Files:**
- Modify: `crates/terminal-core/src/terminal.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/terminal-core/src/terminal.rs`:
```rust
    #[test]
    fn text_wraps_at_column_width() {
        let mut t = Terminal::new(4, 3); // 4 columns
        t.feed(b"abcdef"); // 6 chars -> wraps after 4
        let s = t.snapshot();
        assert_eq!(s.row(0), "abcd");
        assert_eq!(s.row(1), "ef");
    }

    #[test]
    fn resize_changes_width() {
        let mut t = Terminal::new(4, 3);
        t.resize(8, 3);
        t.feed(b"abcdef");
        let s = t.snapshot();
        assert_eq!(s.cols, 8);
        assert_eq!(s.row(0), "abcdef"); // now fits on one line
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p terminal-core resize_changes_width`
Expected: FAIL — `resize` not found.

- [ ] **Step 3: Implement resize**

Add this method inside `impl Terminal` (after `feed`):
```rust
    pub fn resize(&mut self, cols: usize, lines: usize) {
        self.dims = Dims { cols: cols.max(1), lines: lines.max(1) };
        self.term.resize(self.dims);
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p terminal-core terminal::`
Expected: all PASS (6 tests). `Term::resize<D: Dimensions>(&mut self, dims: D)` takes the dimensions by value.

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/src/terminal.rs
git commit -m "feat(terminal-core): Terminal::resize with reflow"
```

---

## Task 5: `LocalPty` — ByteStream over portable-pty

**Files:**
- Create: `crates/terminal-core/src/pty.rs`
- Modify: `crates/terminal-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/terminal-core/src/pty.rs`:
```rust
//! LocalPty: a real OS PTY running $SHELL, behind ByteStream.

use lain_types::{ByteStream, TermError};
use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::io::Read;

pub struct LocalPty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    reader: Box<dyn Read + Send>,
}

impl LocalPty {
    /// Spawn `shell` on a fresh PTY of the given size.
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> Result<Self, TermError> {
        let sys = NativePtySystem::default();
        let pair = sys
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| TermError::Spawn(e.to_string()))?;
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", "xterm-256color");
        pair.slave.spawn_command(cmd).map_err(|e| TermError::Spawn(e.to_string()))?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(|e| TermError::Spawn(e.to_string()))?;
        let writer = pair.master.take_writer().map_err(|e| TermError::Spawn(e.to_string()))?;
        Ok(LocalPty { master: pair.master, writer, reader })
    }

    /// The blocking reader, to be moved into a ReaderPump thread.
    pub fn take_reader(&mut self) -> Box<dyn Read + Send> {
        self.master.try_clone_reader().expect("clone reader")
    }
}

impl ByteStream for LocalPty {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }
    fn resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Terminal;
    use std::time::{Duration, Instant};

    #[test]
    fn shell_echoes_a_command() {
        let mut pty = LocalPty::spawn("/bin/sh", 80, 24).expect("spawn sh");
        let mut term = Terminal::new(80, 24);
        pty.write(b"printf BEBOPMARK\n").unwrap();

        let mut buf = [0u8; 4096];
        let start = Instant::now();
        loop {
            let n = pty.read(&mut buf).unwrap();
            term.feed(&buf[..n]);
            let snap = term.snapshot();
            if (0..snap.lines).any(|l| snap.row(l).contains("BEBOPMARK")) {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "no echo within 5s");
        }
    }
}
```

- [ ] **Step 2: Wire the module**

Update `crates/terminal-core/src/lib.rs`:
```rust
//! terminal-core (codename BEBOP): single-terminal primitive.

mod pty;
mod terminal;

pub use pty::LocalPty;
pub use terminal::Terminal;
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p terminal-core shell_echoes_a_command -- --nocapture`
Expected: PASS within a few hundred ms (uses `/bin/sh`, not the user's slow zsh — see spike results).

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/src/lib.rs crates/terminal-core/src/pty.rs
git commit -m "feat(terminal-core): LocalPty ByteStream over portable-pty"
```

---

## Task 6: `ReaderPump` — bounded off-thread reader (backpressure)

**Files:**
- Create: `crates/terminal-core/src/pump.rs`
- Modify: `crates/terminal-core/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/terminal-core/src/pump.rs`:
```rust
//! ReaderPump: runs a blocking reader off-thread behind a BOUNDED channel.
//! Full channel -> reader blocks -> PTY fills -> writer blocks -> OS throttles
//! the source. This is the backpressure of design spec S3.1.

use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct ReaderPump {
    rx: Receiver<Vec<u8>>,
    produced: Arc<AtomicUsize>,
    _handle: JoinHandle<()>,
}

impl ReaderPump {
    /// `bound` = max queued chunks; `chunk` = read buffer size. Memory ceiling
    /// is ~`bound * chunk`.
    pub fn start(mut reader: Box<dyn Read + Send>, bound: usize, chunk: usize) -> Self {
        let (tx, rx) = sync_channel::<Vec<u8>>(bound);
        let produced = Arc::new(AtomicUsize::new(0));
        let produced_t = produced.clone();
        let handle = std::thread::spawn(move || {
            let mut buf = vec![0u8; chunk];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break; // consumer dropped
                        }
                        produced_t.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
        ReaderPump { rx, produced, _handle: handle }
    }

    /// Pull all currently-available bytes (non-blocking), concatenated.
    pub fn drain(&self) -> Vec<u8> {
        let mut out = Vec::new();
        while let Ok(chunk) = self.rx.try_recv() {
            out.extend_from_slice(&chunk);
        }
        out
    }

    /// Chunks the reader thread has successfully enqueued (test instrumentation).
    pub fn chunks_produced(&self) -> usize {
        self.produced.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn unbounded_source_is_held_by_backpressure() {
        // Infinite source; never drain. Producer must stall near the bound.
        let reader = Box::new(std::io::repeat(b'x'));
        let pump = ReaderPump::start(reader, 4, 1024);

        std::thread::sleep(Duration::from_millis(100));
        let stalled = pump.chunks_produced();
        // capacity 4 + at most one in-flight send
        assert!(stalled <= 5, "producer should stall near bound, got {stalled}");

        // Draining lets it resume.
        let bytes = pump.drain();
        assert!(!bytes.is_empty());
        std::thread::sleep(Duration::from_millis(50));
        assert!(pump.chunks_produced() > stalled, "producer should resume after drain");
    }

    #[test]
    fn drain_yields_written_bytes() {
        let reader = Box::new(std::io::Cursor::new(b"abcdef".to_vec()));
        let pump = ReaderPump::start(reader, 8, 3);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(pump.drain(), b"abcdef");
    }
}
```

- [ ] **Step 2: Wire the module**

Update `crates/terminal-core/src/lib.rs`:
```rust
//! terminal-core (codename BEBOP): single-terminal primitive.

mod pty;
mod pump;
mod terminal;

pub use lain_types::{ByteStream, Cursor, Damage, GridSnapshot};
pub use pty::LocalPty;
pub use pump::ReaderPump;
pub use terminal::Terminal;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p terminal-core pump::`
Expected: both PASS. `unbounded_source_is_held_by_backpressure` proves the channel bound stalls a runaway producer.

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/src/lib.rs crates/terminal-core/src/pump.rs
git commit -m "feat(terminal-core): ReaderPump bounded off-thread reader (backpressure)"
```

---

## Task 7: End-to-end flood test — bounded memory under `/dev/urandom`

**Files:**
- Create: `crates/terminal-core/tests/flood.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/terminal-core/tests/flood.rs`:
```rust
//! Integration: a sustained adversarial flood must NOT balloon memory.
//! Mirrors the spike's floodtest (design spec S3.1).

use std::time::{Duration, Instant};
use terminal_core::{ByteStream, LocalPty, ReaderPump, Terminal};

fn vmrss_kb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }
    0
}

#[test]
fn flood_keeps_memory_bounded() {
    let mut pty = LocalPty::spawn("/bin/sh", 142, 47).expect("spawn sh");
    let mut term = Terminal::new(142, 47);
    let pump = ReaderPump::start(pty.take_reader(), 64, 65536); // ~4 MiB ceiling

    // settle the prompt
    let s = Instant::now();
    while s.elapsed() < Duration::from_millis(300) {
        term.feed(&pump.drain());
        std::thread::sleep(Duration::from_millis(5));
    }

    pty.write(b"cat /dev/urandom\n").unwrap();
    let t = Instant::now();
    let (mut bytes, mut peak) = (0usize, 0u64);
    while t.elapsed() < Duration::from_secs(2) {
        let chunk = pump.drain();
        bytes += chunk.len();
        term.feed(&chunk);
        peak = peak.max(vmrss_kb());
    }

    assert!(bytes > 1_000_000, "expected real flow, got {bytes} bytes");
    // Generous ceiling: bounded channel + grid/scrollback, NOT GBs.
    assert!(peak < 120_000, "RSS ballooned to {peak} KB — backpressure failed");
}
```

- [ ] **Step 2: Run to verify it passes**

Run: `cargo test -p terminal-core --test flood -- --nocapture`
Expected: PASS. Processes many MB of `/dev/urandom`; peak RSS stays well under 120 MB (spike measured ~18 MB). With an unbounded channel this assertion would fail (RSS → GBs).

- [ ] **Step 3: Run the whole suite**

Run: `cargo test`
Expected: all tests across both crates PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/tests/flood.rs
git commit -m "test(terminal-core): end-to-end flood stays memory-bounded"
```

---

## What Plan 1 delivers (and what is next)

After Task 7: a headless terminal engine that runs a real shell, parses its output into a grid (VTE-conformant), survives an adversarial flood with bounded memory, and exposes `feed()` + `snapshot()` through the `lain-types` seam. No window, no GPU yet.

**Subsequent plans (each its own spec §11 step, risk-ordered):**
- **Plan 2 — CPU render + shared text engine:** `Renderer` trait, softbuffer + cosmic-text + swash, bundled font, golden-frame tests, hit < 15 MB headless render.
- **Plan 3 — GPU render + reactive loop:** wgpu + glyphon behind the same trait, backend-parity tests, `winit` reactive damage-driven loop, passive `RenderTarget`, the `bebop run` dev binary; precise per-line `Damage`.
- **Plan 4 — Input:** key→PTY encoding + terminal actions + the shared keymap resolver seam.
- **Plan 5 — Config + notices:** TOML + hot-reload, the typed `Notice`/`TermStatus` channel + degradation behaviors.
- **Plan 6 — OSC 133 marks:** mark capture, `command_blocks` (bounded by scrollback), `bebop shell-integration` snippets.
- **Plan 7 — Performance regression gate:** the spike's instrumentation as an automated bench vs the §2 budgets.
