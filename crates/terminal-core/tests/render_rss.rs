//! Headless CPU render must not leak across frames, and should be light
//! (design spec SC-6: target < 15 MB headless). The < 15 MB target is REPORTED
//! against, not hard-gated, because cosmic-text's baseline may sit near it — the
//! point is to measure it honestly (as the spike did), not to fudge a pass.

use terminal_core::{CpuRenderer, Cursor, GridSnapshot, Renderer};

fn vmrss_kb() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
        }
    }
    0
}

fn filled_grid(cols: usize, lines: usize) -> GridSnapshot {
    // A realistic mix: printable ASCII tiled across the grid.
    let cells: Vec<char> = (0..cols * lines)
        .map(|i| char::from(33u8 + (i % 94) as u8))
        .collect();
    GridSnapshot { cols, lines, cells, cursor: Cursor { line: 0, col: 0 } }
}

#[test]
fn cpu_render_is_memory_stable_and_light() {
    let mut r = CpuRenderer::new(14.0);
    let grid = filled_grid(80, 24);

    // Warm the swash glyph cache.
    for _ in 0..50 {
        let _ = r.render(&grid);
    }
    let base = vmrss_kb();
    for _ in 0..1000 {
        let _ = r.render(&grid);
    }
    let after = vmrss_kb();

    println!("CPU headless render RSS: base={base} KB, after_1000={after} KB (SC-6 target < 15360 KB)");

    // Hard gate 1: no per-frame leak (1000 renders must not grow RSS meaningfully).
    assert!(after <= base + 5_000, "render leaks memory: {base} -> {after} KB");
    // Hard gate 2: gross sanity ceiling (catch a real blowup; not the 15 MB target).
    assert!(after < 40_000, "headless RSS unexpectedly high: {after} KB");
}
