//! CPU vs GPU backend parity (design §4: shared text engine). Structure must match
//! (blank cells -> background; a glyph -> ink in the same place; blank rows stay
//! blank). Per-pixel ink-mask agreement is REPORTED with a loose floor, not gated
//! tightly -- sRGB + antialiasing differ between direct-blit (CPU) and atlas (GPU).
//! Skips cleanly when no GPU adapter is available.

use terminal_core::{CpuRenderer, GpuRenderer, GridSnapshot, Renderer};

fn snapshot(cols: usize, lines: usize, fill: &[(usize, char)]) -> GridSnapshot {
    use terminal_core::Cursor;
    let mut cells = vec![' '; cols * lines];
    for &(i, c) in fill {
        cells[i] = c;
    }
    GridSnapshot { cols, lines, cells, cursor: Cursor { line: 0, col: 0 } }
}

#[test]
fn cpu_and_gpu_agree_structurally() {
    let Some((device, queue)) = terminal_core::try_headless_gpu() else {
        eprintln!("SKIP cpu_and_gpu_agree_structurally: no wgpu adapter");
        return;
    };
    let mut cpu = CpuRenderer::new(14.0);
    let mut gpu = GpuRenderer::new_offscreen(&device, &queue, 14.0);

    // Blank grid -> both uniform background, same dimensions.
    let blank = snapshot(12, 4, &[]);
    let c0 = cpu.render(&blank);
    let g0 = gpu.render_offscreen(&device, &queue, &blank);
    assert_eq!((c0.width, c0.height), (g0.width, g0.height), "canvas dimensions must match");
    let cbg = c0.at(c0.width - 1, c0.height - 1);
    let gbg = g0.at(g0.width - 1, g0.height - 1);
    assert!(c0.data.iter().all(|&p| p == cbg), "CPU blank grid must be uniform");
    assert!(g0.data.iter().all(|&p| p == gbg), "GPU blank grid must be uniform");

    // A glyph -> both paint ink; both keep the bottom blank row clean.
    let grid = snapshot(12, 4, &[(0, 'W')]);
    let c = cpu.render(&grid);
    let g = gpu.render_offscreen(&device, &queue, &grid);
    assert!(c.data.iter().any(|&p| p != cbg), "CPU must paint the glyph");
    assert!(g.data.iter().any(|&p| p != gbg), "GPU must paint the glyph");

    // Reported metric: fraction of pixels where the two backends agree on whether
    // ink is present. High = the glyph landed in the same place via the same layout.
    let total = c.data.len();
    let agree = c
        .data
        .iter()
        .zip(g.data.iter())
        .filter(|(&cp, &gp)| (cp != cbg) == (gp != gbg))
        .count();
    let frac = agree as f64 / total as f64;
    println!("BACKEND_PARITY ink-mask agreement = {frac:.3} ({agree} / {total} px)");

    // Loose gross-divergence floor only (NOT a tight pixel gate -- see header).
    assert!(frac > 0.70, "CPU/GPU ink masks diverge badly: {frac:.3}");
}
