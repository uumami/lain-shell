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
    ///
    /// MUTUALLY EXCLUSIVE with `ByteStream::read`: this hands out an independent
    /// reader fd over the same PTY master, so using both at once would split the
    /// output stream nondeterministically. A consumer picks one path — the pump
    /// (this) OR `ByteStream::read` — never both. Plan 2 makes this exclusivity
    /// structural when it wires the read side into the event loop.
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
            .map_err(|e| std::io::Error::other(e.to_string()))
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
