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
    pub fn start(reader: Box<dyn Read + Send>, bound: usize, chunk: usize) -> Self {
        Self::start_with_waker(reader, bound, chunk, || {})
    }

    /// Like [`start`], but calls `waker` after each chunk is enqueued — used to
    /// wake a reactive event loop (`winit` `EventLoopProxy`) so it drains+renders.
    pub fn start_with_waker(
        mut reader: Box<dyn Read + Send>,
        bound: usize,
        chunk: usize,
        waker: impl Fn() + Send + 'static,
    ) -> Self {
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
                        waker();
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

    #[test]
    fn waker_fires_for_enqueued_chunks() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let reader = Box::new(std::io::Cursor::new(b"abcdef".to_vec()));
        let pump = ReaderPump::start_with_waker(reader, 8, 3, move || {
            c.fetch_add(1, Ordering::Relaxed);
        });
        std::thread::sleep(Duration::from_millis(50));
        let _ = pump.drain();
        assert!(calls.load(Ordering::Relaxed) >= 1, "waker should fire at least once");
    }
}
