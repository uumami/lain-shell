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
