//! LocalPty: a real OS PTY running $SHELL, behind ByteStream.

use lain_types::{ByteStream, TermError};
use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::io::Read;

pub struct LocalPty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    // The ONE reader fd. `take_reader` moves it out for a ReaderPump; once taken,
    // `ByteStream::read` errors. This makes the two read paths mutually exclusive
    // by construction (not just by documentation).
    reader: Option<Box<dyn Read + Send>>,
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
        Ok(LocalPty { master: pair.master, writer, reader: Some(reader) })
    }

    /// Move the single blocking reader out, to drive a [`ReaderPump`] thread.
    ///
    /// MUTUALLY EXCLUSIVE with [`ByteStream::read`] BY CONSTRUCTION: this moves the
    /// one reader fd out of the `LocalPty`, so after calling it `ByteStream::read`
    /// returns an error. A consumer picks one path — the pump (this) OR
    /// `ByteStream::read` — never both. Panics if called twice.
    pub fn take_reader(&mut self) -> Box<dyn Read + Send> {
        self.reader.take().expect("LocalPty reader already taken (take_reader is single-use)")
    }
}

impl ByteStream for LocalPty {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.reader.as_mut() {
            Some(r) => r.read(buf),
            None => Err(std::io::Error::other(
                "LocalPty reader was taken via take_reader; drive the ReaderPump instead",
            )),
        }
    }
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }
    fn resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Terminal;
    use std::time::{Duration, Instant};

    #[test]
    fn read_after_take_reader_errors() {
        // take_reader moves the single reader out; ByteStream::read must then fail,
        // so the two paths cannot both drain the PTY (structural exclusivity).
        let mut pty = LocalPty::spawn("/bin/sh", 80, 24).expect("spawn sh");
        let _reader = pty.take_reader();
        let mut buf = [0u8; 16];
        assert!(
            pty.read(&mut buf).is_err(),
            "ByteStream::read must error after take_reader (single reader was moved out)"
        );
    }

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
