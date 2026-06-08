# BEBOP Render — CPU/Headless Rasterizer Implementation Plan (Plan 2 of N)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render a BEBOP `GridSnapshot` into an in-memory pixel buffer on the CPU (no window, no GPU), using a bundled font, behind a `Renderer` trait the GPU backend will later share.

**Architecture:** A `Renderer` trait + `PixelBuffer` output type, and a `CpuRenderer` that builds the visible grid as monospace text, shapes/lays it out with `cosmic-text` (`Basic` shaping), and rasterizes glyphs with `cosmic-text`'s `Buffer::draw` (swash under the hood) by alpha-blending each glyph pixel into the buffer. Font is a single bundled TTF loaded directly into a `fontdb::Database` (skips system enumeration). Headless and deterministic, so it tests via pixel-property + determinism assertions and a memory-stability check — no window, no committed reference images.

**Tech Stack:** Rust 1.94, `cosmic-text` 0.12 (same version glyphon 0.6 uses, so Plan 3's GPU backend unifies), the existing `lain-types` / `terminal-core` workspace.

**Source of truth:** `docs/superpowers/specs/2026-06-08-bebop-terminal-core-design.md` §4 (render layer: dual-backend seam, shared cosmic-text+swash text engine, bundled font, `Basic` shaping) and §2 (SC-6 headless < 15 MB). This is build-order step 2 from §11. The `softbuffer` present-to-window step is Plan 3 (this plan is pure in-memory rasterization — that is what makes it headless and testable).

**Builds on Plan 1** (committed on `new-seed`): `terminal-core` exports `Terminal`, `LocalPty`, `ReaderPump`, and the `lain-types` seam (`GridSnapshot`, `Cursor`, `Damage`, `ByteStream`). The workspace already pins `rustix = { version = "0.38", features = ["std"] }` in `terminal-core` (a feature-unification fix from Plan 1).

---

## File structure

```
crates/terminal-core/Cargo.toml          # add cosmic-text dep
crates/terminal-core/assets/
  DejaVuSansMono.ttf                       # vendored bundled font (provisional choice, permissive license)
  FONT-LICENSE.txt                         # the font's license text
crates/terminal-core/src/render.rs         # PixelBuffer + Renderer trait + CpuRenderer (one file, one responsibility)
crates/terminal-core/src/lib.rs            # wire `mod render` + re-exports
crates/terminal-core/tests/render_rss.rs   # headless memory-stability + RSS report
```

`render.rs` owns everything render-CPU: the output type, the trait seam, and the cosmic-text rasterizer. It is self-contained and testable without a window. (Plan 3 will add `render/gpu.rs` behind the same `Renderer` trait and may split `render.rs` into a module dir then — not now.)

---

## Task 1: Add cosmic-text + vendor the bundled font

**Files:**
- Modify: `crates/terminal-core/Cargo.toml`
- Create: `crates/terminal-core/assets/DejaVuSansMono.ttf`
- Create: `crates/terminal-core/assets/FONT-LICENSE.txt`

- [ ] **Step 1: Add the dependency**

Add `cosmic-text` to `crates/terminal-core/Cargo.toml` under `[dependencies]` (leave the existing lines, including the `rustix` pin, untouched):
```toml
cosmic-text = "0.12"
```

- [ ] **Step 2: Vendor the bundled font**

The provisional bundled font is DejaVu Sans Mono (permissive Bitstream Vera / Arev license, redistributable). It exists on this machine. Copy it and its license note in:
```bash
mkdir -p crates/terminal-core/assets
cp /usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf crates/terminal-core/assets/DejaVuSansMono.ttf
cat > crates/terminal-core/assets/FONT-LICENSE.txt <<'EOF'
DejaVu Sans Mono — provisional bundled monospace font for BEBOP.
License: Bitstream Vera Fonts Copyright + Arev Fonts Copyright (permissive,
redistributable; see https://dejavu-fonts.github.io/License.html).
Provisional per BEBOP design open item: bundled font choice (license + glyph
coverage) is finalized at implementation; swap here if a different font is chosen.
EOF
```
If `/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf` does not exist on the build machine, STOP and report BLOCKED (do not substitute a random font silently).

- [ ] **Step 3: Verify it builds and the font is non-empty**

Run: `test -s crates/terminal-core/assets/DejaVuSansMono.ttf && echo FONT_OK && cargo build`
Expected: prints `FONT_OK`, then `cargo build` finishes (downloads/compiles cosmic-text + its deps — a few minutes; that is compile time, not an error). If the build fails with a transitive feature-unification error like Plan 1's rustix one (e.g. an `E0277 From<Errno>` deep in a dep), report BLOCKED with the exact error — do not improvise version bumps.

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/Cargo.toml crates/terminal-core/assets/ Cargo.lock
git commit -m "build(terminal-core): add cosmic-text + vendor DejaVu Sans Mono

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `PixelBuffer` + `Renderer` trait + blank `CpuRenderer`

This task establishes the seam and a `CpuRenderer` whose `render` first only fills the background (TDD: minimal behavior). Task 3 adds glyph rasterization.

**Files:**
- Create: `crates/terminal-core/src/render.rs`
- Modify: `crates/terminal-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/terminal-core/src/render.rs` with this content (impl + the blank test):
```rust
//! CPU/headless rasterizer: GridSnapshot -> in-memory PixelBuffer.
//! Shared text path (cosmic-text + swash) that the GPU backend (Plan 3) reuses.

use cosmic_text::{fontdb, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, SwashCache};
use lain_types::GridSnapshot;

const BUNDLED_FONT: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");

/// Packed 0x00RRGGBB, opaque. Row-major, len == width * height.
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>,
}

impl PixelBuffer {
    fn filled(width: u32, height: u32, color: u32) -> Self {
        PixelBuffer { width, height, data: vec![color; (width * height) as usize] }
    }
    /// Pixel at (x, y); panics if out of range (test helper / internal use).
    pub fn at(&self, x: u32, y: u32) -> u32 {
        self.data[(y * self.width + x) as usize]
    }
}

pub trait Renderer {
    fn render(&mut self, grid: &GridSnapshot) -> PixelBuffer;
}

pub struct CpuRenderer {
    font_system: FontSystem,
    swash: SwashCache,
    metrics: Metrics,
    cell_w: u32,
    cell_h: u32,
    fg: Color,
    bg: u32,
}

impl CpuRenderer {
    pub fn new(font_size: f32) -> Self {
        // Bundled font only -> skip system enumeration (~18 MB PSS + ~110 ms saved).
        let mut db = fontdb::Database::new();
        db.load_font_data(BUNDLED_FONT.to_vec());
        let fam = db
            .faces()
            .next()
            .map(|f| f.families[0].0.clone())
            .expect("bundled font has a family");
        db.set_monospace_family(fam.clone());
        db.set_sans_serif_family(fam.clone());
        db.set_serif_family(fam);
        let font_system = FontSystem::new_with_locale_and_db("en-US".into(), db);

        let line_height = (font_size * 1.2).ceil();
        CpuRenderer {
            font_system,
            swash: SwashCache::new(),
            metrics: Metrics::new(font_size, line_height),
            cell_w: (font_size * 0.6).ceil() as u32,
            cell_h: line_height as u32,
            fg: Color::rgb(220, 220, 220),
            bg: 0x0d0d0f,
        }
    }

    fn canvas_size(&self, grid: &GridSnapshot) -> (u32, u32) {
        (
            (grid.cols as u32 * self.cell_w).max(1),
            (grid.lines as u32 * self.cell_h).max(1),
        )
    }
}

impl Renderer for CpuRenderer {
    fn render(&mut self, grid: &GridSnapshot) -> PixelBuffer {
        let (width, height) = self.canvas_size(grid);
        PixelBuffer::filled(width, height, self.bg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lain_types::Cursor;

    fn snapshot(cols: usize, lines: usize, fill: &[(usize, char)]) -> GridSnapshot {
        let mut cells = vec![' '; cols * lines];
        for &(i, c) in fill {
            cells[i] = c;
        }
        GridSnapshot { cols, lines, cells, cursor: Cursor { line: 0, col: 0 } }
    }

    #[test]
    fn blank_grid_is_all_background() {
        let mut r = CpuRenderer::new(14.0);
        let pb = r.render(&snapshot(10, 3, &[]));
        assert_eq!(pb.width, 10 * r.cell_w);
        assert_eq!(pb.height, 3 * r.cell_h);
        assert!(pb.data.iter().all(|&p| p == 0x0d0d0f), "blank grid must be all bg");
    }
}
```

- [ ] **Step 2: Wire the module**

Set `crates/terminal-core/src/lib.rs` to (add the `render` module + re-exports; keep the existing lines):
```rust
//! terminal-core (codename BEBOP): single-terminal primitive.

mod pty;
mod pump;
mod render;
mod terminal;

pub use lain_types::{ByteStream, Cursor, Damage, GridSnapshot};
pub use pty::LocalPty;
pub use pump::ReaderPump;
pub use render::{CpuRenderer, PixelBuffer, Renderer};
pub use terminal::Terminal;
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p terminal-core render::`
Expected: PASS (1 test). Also run `cargo test -p terminal-core` to confirm Plan 1's 9 tests still pass.

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/src/render.rs crates/terminal-core/src/lib.rs
git commit -m "feat(terminal-core): Renderer trait + PixelBuffer + blank CpuRenderer

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Glyph rasterization (cosmic-text Buffer::draw)

**Files:**
- Modify: `crates/terminal-core/src/render.rs`

- [ ] **Step 1: Write the failing tests**

Add these tests to the `mod tests` block in `crates/terminal-core/src/render.rs`:
```rust
    #[test]
    fn glyph_paints_pixels_in_its_cell_only() {
        let mut r = CpuRenderer::new(14.0);
        // 'X' in the top-left cell; everything else blank.
        let grid = snapshot(10, 3, &[(0, 'X')]);
        let pb = r.render(&grid);
        // Some non-bg pixels exist (the glyph painted).
        assert!(pb.data.iter().any(|&p| p != 0x0d0d0f), "glyph should paint pixels");
        // The bottom-right pixel (far from the glyph) stays background.
        assert_eq!(pb.at(pb.width - 1, pb.height - 1), 0x0d0d0f);
    }

    #[test]
    fn render_is_deterministic() {
        let mut r = CpuRenderer::new(14.0);
        let grid = snapshot(8, 2, &[(0, 'h'), (1, 'i')]);
        let a = r.render(&grid);
        let b = r.render(&grid);
        assert_eq!(a.data, b.data, "same grid must rasterize identically");
    }
```

- [ ] **Step 2: Run, verify the glyph test FAILS**

Run: `cargo test -p terminal-core glyph_paints_pixels_in_its_cell_only`
Expected: FAIL — the blank renderer paints nothing, so the `any(p != bg)` assertion fails.

- [ ] **Step 3: Implement rasterization**

Replace the `impl Renderer for CpuRenderer` block in `render.rs` with:
```rust
impl Renderer for CpuRenderer {
    fn render(&mut self, grid: &GridSnapshot) -> PixelBuffer {
        let (width, height) = self.canvas_size(grid);
        let mut pb = PixelBuffer::filled(width, height, self.bg);

        // Build the visible grid as monospace text (one row per line).
        let mut text = String::with_capacity(grid.cells.len() + grid.lines);
        for l in 0..grid.lines {
            for c in 0..grid.cols {
                text.push(grid.cells[l * grid.cols + c]);
            }
            text.push('\n');
        }

        let mut buffer = Buffer::new(&mut self.font_system, self.metrics);
        buffer.set_size(&mut self.font_system, Some(width as f32), Some(height as f32));
        buffer.set_text(
            &mut self.font_system,
            &text,
            Attrs::new().family(Family::Monospace),
            Shaping::Basic,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let fg = self.fg;
        let (w, h) = (width as i32, height as i32);
        // draw() rasterizes each glyph via swash and calls us per painted pixel.
        // `color` carries the glyph's coverage in its alpha channel.
        buffer.draw(&mut self.font_system, &mut self.swash, fg, |x, y, _gw, _gh, color| {
            if x < 0 || y < 0 || x >= w || y >= h {
                return;
            }
            let a = color.a() as u32;
            if a == 0 {
                return;
            }
            let idx = (y as u32 * width + x as u32) as usize;
            let dst = pb.data[idx];
            let blend = |s: u32, d: u32| (s * a + d * (255 - a)) / 255;
            let r = blend(color.r() as u32, (dst >> 16) & 0xff);
            let g = blend(color.g() as u32, (dst >> 8) & 0xff);
            let b = blend(color.b() as u32, dst & 0xff);
            pb.data[idx] = (r << 16) | (g << 8) | b;
        });

        pb
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p terminal-core render::`
Expected: all PASS (3 tests: blank, glyph, determinism). Also `cargo test -p terminal-core` → 12 pass (9 Plan-1 + 3 render).

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/src/render.rs
git commit -m "feat(terminal-core): CpuRenderer rasterizes glyphs via cosmic-text draw

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Headless memory-stability + RSS report

**Files:**
- Create: `crates/terminal-core/tests/render_rss.rs`

- [ ] **Step 1: Write the test**

Create `crates/terminal-core/tests/render_rss.rs`:
```rust
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
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p terminal-core --test render_rss -- --nocapture`
Expected: PASS. Note the printed `after_1000` value and compare it to the SC-6 < 15 MB (15360 KB) target.

- [ ] **Step 3: Report the RSS finding**

This is a measurement, not just a gate. After the test passes, record the observed steady-state `after_1000` value:
- If `after_1000 < 15360` KB: SC-6 headless target HOLDS — note it.
- If `after_1000 >= 15360` KB: the test still passes (it only gates leak + 40 MB ceiling), but the SC-6 < 15 MB target is MISSED by cosmic-text's baseline — report this as DONE_WITH_CONCERNS with the number, so it can be recorded the way the spike recorded its RSS findings (revise the target or investigate trimming).

- [ ] **Step 4: Run the whole suite**

Run: `cargo test`
Expected: all pass (lain-types 2 + terminal-core unit 12 + flood 1 + render_rss 1).

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/tests/render_rss.rs
git commit -m "test(terminal-core): headless render memory-stable + RSS report

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## What Plan 2 delivers (and what is next)

After Task 4: a deterministic, headless CPU rasterizer that turns a `GridSnapshot` into a pixel buffer using the bundled font and the shared cosmic-text text path, behind a `Renderer` trait, with measured headless memory behavior. No window, no GPU.

**Subsequent plans (spec §11):**
- **Plan 3 — GPU render + reactive loop:** `wgpu` + `glyphon` behind the same `Renderer` trait (sharing the cosmic-text layout already built here), backend-parity tests (CPU vs GPU pixel diff), the `winit` reactive damage-driven loop, passive `RenderTarget`, `softbuffer` present-to-window for the CPU path, the `bebop run` dev binary, and the LocalPty dual-reader exclusivity fix deferred from Plan 1.
- **Plan 4 — Input;** **Plan 5 — Config + notices;** **Plan 6 — OSC 133 marks;** **Plan 7 — Performance regression gate.**
