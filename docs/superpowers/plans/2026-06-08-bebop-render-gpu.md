# BEBOP Render — GPU + Reactive Loop Implementation Plan (Plan 3 of N)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a GPU (`wgpu` 22 + `glyphon` 0.6) render backend that shares the CPU backend's `cosmic-text` layout, prove the two backends agree with a headless parity test, fix the deferred `LocalPty` dual-reader hazard, and ship a `bebop run` dev binary that renders a real `$SHELL` through a reactive (damage-driven) `winit` loop.

**Architecture:** A `GpuRenderer` (wgpu+glyphon) renders a `GridSnapshot` into any RGBA target — a `winit` surface frame for the live window, or a headless offscreen texture it reads back into the same `PixelBuffer` the `CpuRenderer` produces. Both backends call two shared, pure helpers (`cell_metrics`, `grid_to_text`) so they lay out identical glyphs (the "shared text engine" seam). The `bebop run` binary owns the `winit` event loop with `ControlFlow::Wait`, woken by PTY bytes through an `EventLoopProxy`; it drives one passive `Terminal` and presents via the GPU surface or a `softbuffer` window for the CPU path.

**Tech Stack:** Rust, `wgpu` 22, `glyphon` 0.6 (re-exports the same `cosmic-text` 0.12 the crate already uses, so the layout type is shared — not duplicated), `winit` 0.30, `softbuffer` 0.4, `pollster` 0.3, the existing `lain-types` / `terminal-core` workspace.

**Source of truth:** `docs/superpowers/specs/2026-06-08-bebop-terminal-core-design.md` §3 (passive shared-device ownership; reactive damage-driven loop), §3.1 (bounded reader→parser backpressure), §4 (dual-backend seam drawn low at the glyph-blit layer; bundled font; `Shaping::Basic`; vsync default), §9 (backend-parity test). This is build-order step 3 from §11.

**Builds on Plan 1 + Plan 2** (committed on `new-seed`):
- Plan 1: `terminal-core` exports `Terminal` (`feed`/`snapshot`/`resize`/`take_pty_writes`), `LocalPty` (`spawn`/`take_reader` + `ByteStream`), `ReaderPump` (bounded `sync_channel`). `Cargo.toml` already pins `rustix = { version = "0.38", features = ["std"] }`.
- Plan 2: `crates/terminal-core/src/render.rs` exports the `Renderer` trait (`fn render(&mut self, grid: &GridSnapshot) -> PixelBuffer`), `PixelBuffer` (pub `width`/`height`/`data: Vec<u32>`, `0x00RRGGBB`, plus `at(x,y)`), and `CpuRenderer` (bundled DejaVu Sans Mono via `cosmic-text` 0.12, `Shaping::Basic`). 16 tests green; SC-6 headless < 15 MB holds (~11 MB).

**Scope decision (LOCKED for this plan): monochrome.** The `GridSnapshot` is chars-only today; the spike and Plan 2 are monochrome and that was accepted as proving the pipeline. Color/attrs would change `lain-types::GridSnapshot`, `Terminal::snapshot`, AND both renderers at once — a cross-cutting change that earns its own TDD cycle (risk-first / YAGNI). It is **deferred to a dedicated later plan**, not folded in here. Plan 3's job is the GPU seam + reactive loop + dev binary, proven headless.

---

## File structure

```
crates/terminal-core/Cargo.toml               # add wgpu/glyphon/winit/softbuffer/pollster + [[bin]] bebop
crates/terminal-core/src/render.rs             # add shared helpers cell_metrics/grid_to_text/cell_size; CpuRenderer uses them (refactor, behavior identical)
crates/terminal-core/src/render_gpu.rs         # NEW: GpuRenderer (wgpu+glyphon) + try_headless_gpu()
crates/terminal-core/src/pty.rs                # MODIFY: structural single-reader fix (take_reader vs ByteStream::read mutually exclusive by construction)
crates/terminal-core/src/pump.rs              # MODIFY: ReaderPump::start_with_waker (wake the reactive loop per chunk)
crates/terminal-core/src/lib.rs               # wire `mod render_gpu`; re-export GpuRenderer, try_headless_gpu, cell_size
crates/terminal-core/src/bin/bebop.rs         # NEW: `bebop run` reactive winit loop + dev binary
crates/terminal-core/tests/backend_parity.rs  # NEW: CPU vs GPU headless parity (skips with no adapter)
```

`render_gpu.rs` is a sibling module to `render.rs` (no module-dir split this cycle — minimal churn keeps Plan 2's tests untouched). The two share only the small pure helpers in `render.rs`.

---

## Task 1: Add GPU/window dependencies + the `bebop` bin target

**Files:**
- Modify: `crates/terminal-core/Cargo.toml`

- [ ] **Step 1: Add the dependencies and the bin target**

Append to `[dependencies]` in `crates/terminal-core/Cargo.toml` (leave all existing lines — `lain-types`, `alacritty_terminal`, `portable-pty`, the `rustix` pin, `cosmic-text` — untouched). Then add the `[[bin]]` block at the end of the file:

```toml
# GPU backend: glyphon 0.6 pins wgpu 22 and re-exports cosmic-text 0.12 (the same
# version this crate already depends on), so the text-layout type is shared, not
# duplicated. winit 0.30 + wgpu 22 + softbuffer 0.4 all share raw-window-handle 0.6.
wgpu = "22"
glyphon = "0.6"
winit = "0.30"
softbuffer = "0.4"
pollster = "0.3"

[[bin]]
name = "bebop"
path = "src/bin/bebop.rs"
```

- [ ] **Step 2: Verify the graph builds and cosmic-text stays a single version**

Run:
```bash
cargo build -p terminal-core 2>&1 | tail -5 && echo "---cosmic-text versions---" && cargo tree -p terminal-core 2>/dev/null | grep -c 'cosmic-text v'
```
Expected: `cargo build` finishes (this pulls wgpu + winit + glyphon — several minutes the first time; that is compile time, not an error). The grep count of `cosmic-text v` lines should show the dependency resolving to **one** version (a count of `1` after dedup; if `cargo tree` prints the same `cosmic-text v0.12.x` under multiple parents that is fine **as long as the version string is identical**). If two *different* `cosmic-text` versions appear (e.g. 0.12 and 0.14), STOP and report BLOCKED with the `cargo tree | grep cosmic-text` output — glyphon's version diverged from the crate's and the layout type will not unify.

The `bebop` bin does not exist yet, so `cargo build` builds only the lib at this step (the bin file is created in Task 7). That is expected.

- [ ] **Step 3: Confirm existing tests still pass**

Run: `cargo test -p terminal-core 2>&1 | tail -15`
Expected: Plan 1 + Plan 2 tests still green (no behavior changed yet).

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/Cargo.toml Cargo.lock
git commit -m "build(terminal-core): add wgpu+glyphon+winit+softbuffer for GPU backend + bebop bin

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `LocalPty` structural single-reader fix (deferred from Plan 1)

The Plan 1 review flagged that `LocalPty::take_reader` mints a *second* reader fd over the same PTY master while `ByteStream::read` uses a separate `reader` field — using both at once splits the output stream nondeterministically. Plan 1 documented this as "mutually exclusive" in prose. This task makes it exclusive **by construction**: there is exactly one reader, held in an `Option`; `take_reader` moves it out, after which `ByteStream::read` cannot succeed.

**Files:**
- Modify: `crates/terminal-core/src/pty.rs`

- [ ] **Step 1: Write the failing test**

Add this test to the `mod tests` block in `crates/terminal-core/src/pty.rs` (keep the existing `shell_echoes_a_command` test):
```rust
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
```

- [ ] **Step 2: Run it, verify it FAILS**

Run: `cargo test -p terminal-core read_after_take_reader_errors 2>&1 | tail -15`
Expected: FAIL — today `take_reader` clones a second fd and the original `reader` field is still readable, so `read` returns `Ok(..)`, not an error. (It may also fail to compile after Step 3's struct change if run later — run it now against the current code to see the logic failure.)

- [ ] **Step 3: Make the reader single + owned**

In `crates/terminal-core/src/pty.rs`, change the struct field and the three sites that touch it. Replace the struct definition:
```rust
pub struct LocalPty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    reader: Box<dyn Read + Send>,
}
```
with:
```rust
pub struct LocalPty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    // The ONE reader fd. `take_reader` moves it out for a ReaderPump; once taken,
    // `ByteStream::read` errors. This makes the two read paths mutually exclusive
    // by construction (not just by documentation).
    reader: Option<Box<dyn Read + Send>>,
}
```

In `spawn`, wrap the reader in `Some(...)` — replace:
```rust
        Ok(LocalPty { master: pair.master, writer, reader })
```
with:
```rust
        Ok(LocalPty { master: pair.master, writer, reader: Some(reader) })
```

Replace the whole `take_reader` method and its doc comment with:
```rust
    /// Move the single blocking reader out, to drive a [`ReaderPump`] thread.
    ///
    /// MUTUALLY EXCLUSIVE with [`ByteStream::read`] BY CONSTRUCTION: this moves the
    /// one reader fd out of the `LocalPty`, so after calling it `ByteStream::read`
    /// returns an error. A consumer picks one path — the pump (this) OR
    /// `ByteStream::read` — never both. Panics if called twice.
    pub fn take_reader(&mut self) -> Box<dyn Read + Send> {
        self.reader.take().expect("LocalPty reader already taken (take_reader is single-use)")
    }
```

In the `impl ByteStream for LocalPty` block, replace the `read` method with:
```rust
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.reader.as_mut() {
            Some(r) => r.read(buf),
            None => Err(std::io::Error::other(
                "LocalPty reader was taken via take_reader; drive the ReaderPump instead",
            )),
        }
    }
```

- [ ] **Step 4: Run the test + the existing PTY test**

Run: `cargo test -p terminal-core --lib pty:: 2>&1 | tail -15`
Expected: both `shell_echoes_a_command` (uses `read`, reader present) and `read_after_take_reader_errors` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/src/pty.rs
git commit -m "fix(terminal-core): LocalPty single-reader is structural (take_reader moves it out)

Resolves the Plan 1 deferral: take_reader and ByteStream::read can no longer
both drain the PTY -- the one reader fd is moved out, after which read() errors.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Shared text-layout helpers (`cell_metrics`, `grid_to_text`, `cell_size`)

Both backends must lay out glyphs identically (the design §4 "shared text engine" promise) and the dev binary needs cell dimensions. Extract the two pure pieces currently inlined in `CpuRenderer` into shared helpers, and add a public `cell_size`. Behavior is unchanged — Plan 2's render tests must stay green.

**Files:**
- Modify: `crates/terminal-core/src/render.rs`

- [ ] **Step 1: Add the helpers**

In `crates/terminal-core/src/render.rs`, add these three functions immediately after the `Renderer` trait definition (above `pub struct CpuRenderer`):
```rust
/// Monospace cell metrics derived from the font size: the `cosmic-text` line
/// metrics plus integer cell width/height. Both render backends use this so they
/// lay out glyphs on the same grid (design §4 shared text engine).
pub(crate) fn cell_metrics(font_size: f32) -> (Metrics, u32, u32) {
    let line_height = (font_size * 1.2).ceil();
    (
        Metrics::new(font_size, line_height),
        (font_size * 0.6).ceil() as u32,
        line_height as u32,
    )
}

/// Integer (cell_w, cell_h) for the given font size — for sizing windows/grids.
pub fn cell_size(font_size: f32) -> (u32, u32) {
    let (_, w, h) = cell_metrics(font_size);
    (w, h)
}

/// Flatten the visible grid into newline-separated monospace rows. Shared by both
/// backends so the text fed to `cosmic-text` is byte-identical across them.
pub(crate) fn grid_to_text(grid: &GridSnapshot) -> String {
    let mut text = String::with_capacity(grid.cells.len() + grid.lines);
    for l in 0..grid.lines {
        for c in 0..grid.cols {
            text.push(grid.cells[l * grid.cols + c]);
        }
        text.push('\n');
    }
    text
}
```

- [ ] **Step 2: Make `CpuRenderer` use the helpers**

In `CpuRenderer::new`, replace the metrics/cell computation. Change:
```rust
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
```
to:
```rust
        let (metrics, cell_w, cell_h) = cell_metrics(font_size);
        CpuRenderer {
            font_system,
            swash: SwashCache::new(),
            metrics,
            cell_w,
            cell_h,
            fg: Color::rgb(220, 220, 220),
            bg: 0x0d0d0f,
        }
```

In `impl Renderer for CpuRenderer`'s `render`, replace the inline text-building loop:
```rust
        // Build the visible grid as monospace text (one row per line).
        let mut text = String::with_capacity(grid.cells.len() + grid.lines);
        for l in 0..grid.lines {
            for c in 0..grid.cols {
                text.push(grid.cells[l * grid.cols + c]);
            }
            text.push('\n');
        }
```
with:
```rust
        let text = grid_to_text(grid);
```

- [ ] **Step 3: Run the render tests (behavior unchanged)**

Run: `cargo test -p terminal-core 2>&1 | tail -15`
Expected: all Plan 2 render tests (`blank_grid_is_all_background`, `glyph_paints_and_blank_rows_stay_background`, `render_is_deterministic`) plus the `render_rss` test still PASS — this was a pure refactor.

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/src/render.rs
git commit -m "refactor(terminal-core): extract shared cell_metrics/grid_to_text/cell_size

The GPU backend (next task) reuses these so both backends lay out identical
glyphs. Pure refactor; CPU render behavior unchanged.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: `GpuRenderer` — wgpu + glyphon, with headless offscreen readback

A `GpuRenderer` that lays out the same `cosmic-text` buffer as the CPU path and renders it via `glyphon` into any RGBA target. It exposes `render_to_view` (into a caller-supplied texture view — used by the window in Task 7) and `render_offscreen` (renders to its own texture and reads it back into a `PixelBuffer` — used here and by the parity test, with no window). A `try_headless_gpu` helper returns `None` when no adapter exists so tests skip gracefully instead of failing on GPU-less machines.

**Files:**
- Create: `crates/terminal-core/src/render_gpu.rs`
- Modify: `crates/terminal-core/src/lib.rs`

- [ ] **Step 1: Create `render_gpu.rs`**

Create `crates/terminal-core/src/render_gpu.rs` with this content:
```rust
//! GPU renderer: GridSnapshot -> wgpu + glyphon. Shares the cosmic-text layout
//! (`cell_metrics`, `grid_to_text`) with the CPU backend so both lay out glyphs
//! identically -- the "shared text engine" seam (design §4). glyphon 0.6
//! re-exports cosmic-text 0.12 (the version this crate already uses), so there is
//! one FontSystem/Buffer/Metrics type across both backends.

use crate::render::{cell_metrics, grid_to_text, PixelBuffer};
use glyphon::fontdb;
use glyphon::{
    Attrs, Buffer, Cache, Color as GColor, Family, FontSystem, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use lain_types::GridSnapshot;

const BUNDLED_FONT: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");

/// Try to acquire a headless wgpu device+queue (no surface). Returns `None` if no
/// adapter is available (GPU-less CI, missing drivers) so callers can skip.
pub fn try_headless_gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: None,
    }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("bebop-headless"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
        },
        None,
    ))
    .ok()?;
    Some((device, queue))
}

pub struct GpuRenderer {
    font_system: FontSystem,
    swash: SwashCache,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    viewport: Viewport,
    buffer: Buffer,
    cell_w: u32,
    cell_h: u32,
    format: wgpu::TextureFormat,
}

impl GpuRenderer {
    /// Build a GPU renderer targeting `format` (must equal the format of every
    /// view passed to `render_to_view`). Bundled font only -> no system enumeration.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        font_size: f32,
    ) -> Self {
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
        let mut font_system = FontSystem::new_with_locale_and_db("en-US".into(), db);

        let (metrics, cell_w, cell_h) = cell_metrics(font_size);
        let swash = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let text_renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        let buffer = Buffer::new(&mut font_system, metrics);

        GpuRenderer {
            font_system,
            swash,
            atlas,
            text_renderer,
            viewport,
            buffer,
            cell_w,
            cell_h,
            format,
        }
    }

    /// Convenience: a headless renderer targeting `Rgba8UnormSrgb`, for offscreen
    /// readback (parity/golden tests). Callers need not name a wgpu format.
    pub fn new_offscreen(device: &wgpu::Device, queue: &wgpu::Queue, font_size: f32) -> Self {
        Self::new(device, queue, wgpu::TextureFormat::Rgba8UnormSrgb, font_size)
    }

    /// Pixel size of the canvas for `grid` (cols*cell_w x lines*cell_h, min 1x1).
    pub fn canvas_size(&self, grid: &GridSnapshot) -> (u32, u32) {
        (
            (grid.cols as u32 * self.cell_w).max(1),
            (grid.lines as u32 * self.cell_h).max(1),
        )
    }

    /// Lay out `grid` and render it into `view` (an RGBA target of `width`x`height`
    /// whose format == the one passed to `new`). Clears to the BEBOP background,
    /// issues its own encoder + submit. Does NOT present — the caller presents a
    /// surface frame (window) or copies the texture (offscreen).
    pub fn render_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid: &GridSnapshot,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let text = grid_to_text(grid);
        self.buffer
            .set_size(&mut self.font_system, Some(width as f32), Some(height as f32));
        self.buffer.set_text(
            &mut self.font_system,
            &text,
            Attrs::new().family(Family::Monospace),
            Shaping::Basic,
        );
        self.buffer.shape_until_scroll(&mut self.font_system, false);

        self.viewport.update(queue, Resolution { width, height });
        self.text_renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                [TextArea {
                    buffer: &self.buffer,
                    left: 0.0,
                    top: 0.0,
                    scale: 1.0,
                    bounds: TextBounds { left: 0, top: 0, right: width as i32, bottom: height as i32 },
                    default_color: GColor::rgb(220, 220, 220),
                    custom_glyphs: &[],
                }],
                &mut self.swash,
            )
            .expect("glyphon prepare");

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("bebop-gpu") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bebop-text"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Matches the CPU backend's 0x0d0d0f background intent.
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 13.0 / 255.0,
                            g: 13.0 / 255.0,
                            b: 15.0 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .expect("glyphon render");
        }
        queue.submit(std::iter::once(encoder.finish()));
        self.atlas.trim();
    }

    /// Headless: render `grid` to an offscreen texture and read it back as a
    /// `PixelBuffer` (0x00RRGGBB). No window. Used by parity/golden tests.
    pub fn render_offscreen(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        grid: &GridSnapshot,
    ) -> PixelBuffer {
        let (width, height) = self.canvas_size(grid);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bebop-offscreen"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.render_to_view(device, queue, grid, &view, width, height);

        // copy_texture_to_buffer requires bytes_per_row aligned to 256.
        let bpp = 4u32;
        let unpadded = width * bpp;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let out_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bebop-readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("bebop-readback") });
        enc.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &out_buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));

        let slice = out_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv().expect("map channel").expect("buffer map");
        let mapped = slice.get_mapped_range();

        let mut data = vec![0u32; (width * height) as usize];
        for y in 0..height {
            let row = &mapped[(y * padded) as usize..];
            for x in 0..width {
                let i = (x * bpp) as usize;
                let r = row[i] as u32;
                let g = row[i + 1] as u32;
                let b = row[i + 2] as u32;
                data[(y * width + x) as usize] = (r << 16) | (g << 8) | b;
            }
        }
        drop(mapped);
        out_buf.unmap();
        PixelBuffer { width, height, data }
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
    fn gpu_blank_is_uniform_and_glyph_paints() {
        let Some((device, queue)) = try_headless_gpu() else {
            eprintln!("SKIP gpu_blank_is_uniform_and_glyph_paints: no wgpu adapter");
            return;
        };
        let mut r = GpuRenderer::new_offscreen(&device, &queue, 14.0);

        let blank = r.render_offscreen(&device, &queue, &snapshot(10, 3, &[]));
        let bg = blank.at(blank.width - 1, blank.height - 1);
        assert!(blank.data.iter().all(|&p| p == bg), "blank grid must be uniform background");

        let g = r.render_offscreen(&device, &queue, &snapshot(10, 3, &[(0, 'X')]));
        let gbg = g.at(g.width - 1, g.height - 1);
        assert!(g.data.iter().any(|&p| p != gbg), "glyph must paint non-background pixels");
        // The entire bottom (blank) row band must stay background -> glyph stayed in its row.
        let (_m, _cw, ch) = cell_metrics(14.0);
        let band = (2 * ch * g.width) as usize;
        assert!(
            g.data[band..].iter().all(|&p| p == gbg),
            "blank bottom row must stay background"
        );
    }

    #[test]
    fn gpu_render_is_deterministic() {
        let Some((device, queue)) = try_headless_gpu() else {
            eprintln!("SKIP gpu_render_is_deterministic: no wgpu adapter");
            return;
        };
        let mut r = GpuRenderer::new_offscreen(&device, &queue, 14.0);
        let grid = snapshot(8, 2, &[(0, 'h'), (1, 'i')]);
        let a = r.render_offscreen(&device, &queue, &grid);
        let b = r.render_offscreen(&device, &queue, &grid);
        assert_eq!(a.data, b.data, "same grid must rasterize identically on the GPU");
    }
}
```

- [ ] **Step 2: Wire the module + re-exports**

In `crates/terminal-core/src/lib.rs`, add the module declaration and re-exports. Add `mod render_gpu;` in the `mod` block (after `mod render;`) and update the render re-export line plus add a GPU re-export. The render-related lines should read:
```rust
mod render;
mod render_gpu;
```
and:
```rust
pub use render::{cell_size, CpuRenderer, PixelBuffer, Renderer};
pub use render_gpu::{try_headless_gpu, GpuRenderer};
```

- [ ] **Step 3: Run the GPU tests**

Run: `cargo test -p terminal-core --lib render_gpu:: -- --nocapture 2>&1 | tail -25`
Expected: both tests PASS. On this machine (Intel HD630 + NVIDIA, Vulkan per the spike) an adapter exists, so they run for real. If they print `SKIP ...: no wgpu adapter` and pass, report DONE_WITH_CONCERNS noting the adapter was unavailable (the parity in Task 5 will also skip) — do NOT treat a skip as a real GPU validation.

- [ ] **Step 4: Run the whole suite**

Run: `cargo test -p terminal-core 2>&1 | tail -20`
Expected: all green (Plan 1 + Plan 2 + the 2 new GPU tests).

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/src/render_gpu.rs crates/terminal-core/src/lib.rs
git commit -m "feat(terminal-core): GpuRenderer (wgpu+glyphon) with headless offscreen readback

Shares cell_metrics/grid_to_text with CpuRenderer; renders a GridSnapshot into
any RGBA view, or offscreen into a read-back PixelBuffer. try_headless_gpu lets
GPU tests skip gracefully without an adapter.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Backend-parity test (CPU vs GPU, headless)

Guards the design §4 promise that both backends share one text engine: the same grid must produce the same *structure* (background where blank, ink where a glyph is) on both backends, and a high agreement on which pixels carry ink. Exact per-channel RGB equality is NOT asserted — sRGB encoding and atlas vs direct-blit antialiasing differ slightly, so the agreement fraction is **reported** (like Plan 2's RSS number) with only a loose gross-divergence floor. The test names no wgpu types, so the test crate needs no wgpu dev-dependency.

**Files:**
- Create: `crates/terminal-core/tests/backend_parity.rs`

- [ ] **Step 1: Write the test**

Create `crates/terminal-core/tests/backend_parity.rs`:
```rust
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
```

- [ ] **Step 2: Run it**

Run: `cargo test -p terminal-core --test backend_parity -- --nocapture 2>&1 | tail -20`
Expected: PASS, printing `BACKEND_PARITY ink-mask agreement = 0.9xx ...`. Record that number in the results note (Task 8). If it prints `SKIP ...: no wgpu adapter`, report that the parity check could not run here.
If `frac` is between 0.70 and ~0.90, the test still passes — report it as DONE_WITH_CONCERNS with the number so the seam's fidelity is on record (it may indicate a half-cell layout offset worth a follow-up), but do NOT tighten or loosen the floor to force a different verdict.

- [ ] **Step 3: Run the whole suite**

Run: `cargo test -p terminal-core 2>&1 | tail -20`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/tests/backend_parity.rs
git commit -m "test(terminal-core): CPU/GPU backend parity (structural + reported ink-mask)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: `ReaderPump::start_with_waker` (wake the reactive loop)

The reactive loop (`ControlFlow::Wait`) must be woken when PTY bytes arrive. The pump already owns the off-thread bounded reader; add a variant that calls a waker after each enqueued chunk (the binary passes a closure that posts an `EventLoopProxy` event). `start` stays as a thin no-op-waker wrapper so existing callers/tests are unchanged.

**Files:**
- Modify: `crates/terminal-core/src/pump.rs`

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `crates/terminal-core/src/pump.rs`:
```rust
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
```

- [ ] **Step 2: Run it, verify it FAILS to compile**

Run: `cargo test -p terminal-core --lib pump:: 2>&1 | tail -15`
Expected: FAIL — `start_with_waker` does not exist yet (`E0599 no function`).

- [ ] **Step 3: Implement the waker variant**

In `crates/terminal-core/src/pump.rs`, replace the entire `start` method with both methods:
```rust
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
```

- [ ] **Step 4: Run the pump tests**

Run: `cargo test -p terminal-core --lib pump:: 2>&1 | tail -15`
Expected: all PASS (`unbounded_source_is_held_by_backpressure`, `drain_yields_written_bytes`, `waker_fires_for_enqueued_chunks`).

- [ ] **Step 5: Commit**

```bash
git add crates/terminal-core/src/pump.rs
git commit -m "feat(terminal-core): ReaderPump::start_with_waker to wake the reactive loop

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: `bebop run` — reactive winit loop + dev binary

The dev vehicle (design §1 deliverable): one window running `$SHELL` through the BEBOP core, on a reactive `ControlFlow::Wait` loop woken by PTY bytes via `EventLoopProxy`. `bebop run` uses the GPU backend (wgpu+glyphon present to the surface); `bebop run --cpu` uses `CpuRenderer` presented through `softbuffer`. A `--smoke` flag drives a deterministic command and exits, so the loop is verifiable without a human. Input encoding is intentionally minimal (Plan 4 owns real key→PTY encoding).

**Files:**
- Create: `crates/terminal-core/src/bin/bebop.rs`

- [ ] **Step 1: Create the binary**

Create `crates/terminal-core/src/bin/bebop.rs`:
```rust
//! `bebop run` -- dev binary: one window running $SHELL through the BEBOP core.
//! Reactive (ControlFlow::Wait), woken by PTY bytes via EventLoopProxy. Exercises
//! the GPU (wgpu+glyphon -> surface) and CPU (softbuffer) present paths in the
//! real crate. Input encoding is spike-grade -- Plan 4 replaces it. Not the NAVI
//! host; just a test vehicle until NAVI exists.
//!
//! Usage: bebop run [--cpu] [--smoke]

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use lain_types::ByteStream;
use terminal_core::{cell_size, CpuRenderer, GpuRenderer, LocalPty, ReaderPump, Renderer, Terminal};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

const FONT_SIZE: f32 = 14.0;

#[derive(Debug, Clone, Copy)]
enum UserEvent {
    Pty,
    Timeout,
}

enum Backend {
    Gpu {
        surface: wgpu::Surface<'static>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        config: wgpu::SurfaceConfiguration,
        renderer: GpuRenderer,
    },
    Cpu {
        _context: softbuffer::Context<Arc<Window>>,
        surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
        renderer: CpuRenderer,
    },
}

struct App {
    use_cpu: bool,
    smoke: bool,
    proxy: EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    backend: Option<Backend>,
    term: Option<Terminal>,
    pty: Option<LocalPty>,
    pump: Option<ReaderPump>,
    cell_w: u32,
    cell_h: u32,
    done: bool,
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("bebop run")
                    .with_inner_size(LogicalSize::new(1000.0, 700.0)),
            )
            .expect("create window"),
        );
        let size = window.inner_size();
        let (cw, ch) = cell_size(FONT_SIZE);
        self.cell_w = cw.max(1);
        self.cell_h = ch.max(1);
        let cols = (size.width / self.cell_w).max(1) as usize;
        let lines = (size.height / self.cell_h).max(1) as usize;

        // PTY + core. Move the single reader into the pump; wake the loop per chunk.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut pty = LocalPty::spawn(&shell, cols as u16, lines as u16).expect("spawn shell");
        let reader = pty.take_reader();
        let wake_proxy = self.proxy.clone();
        let pump = ReaderPump::start_with_waker(reader, 64, 65536, move || {
            let _ = wake_proxy.send_event(UserEvent::Pty);
        });
        let term = Terminal::new(cols, lines);

        let backend = if self.use_cpu {
            let context = softbuffer::Context::new(window.clone()).expect("softbuffer context");
            let surface =
                softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");
            Backend::Cpu { _context: context, surface, renderer: CpuRenderer::new(FONT_SIZE) }
        } else {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
            let surface = instance.create_surface(window.clone()).expect("surface");
            let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            }))
            .expect("no wgpu adapter (try `bebop run --cpu`)");
            eprintln!("adapter: {:?}", adapter.get_info());
            let (device, queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("bebop"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            ))
            .expect("request device");
            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::Fifo, // vsync default (design §4)
                desired_maximum_frame_latency: 2,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
            };
            surface.configure(&device, &config);
            let renderer = GpuRenderer::new(&device, &queue, format, FONT_SIZE);
            Backend::Gpu { surface, device, queue, config, renderer }
        };

        el.set_control_flow(ControlFlow::Wait);
        self.backend = Some(backend);
        self.term = Some(term);
        self.pty = Some(pty);
        self.pump = Some(pump);
        self.window = Some(window.clone());

        if self.smoke {
            // Deterministic command; success when its output text reaches the grid.
            let _ = self.pty.as_mut().unwrap().write(b"printf 'BEBOPSMOKE\\n'\r");
        }
        window.request_redraw();
    }

    fn user_event(&mut self, el: &ActiveEventLoop, ev: UserEvent) {
        match ev {
            UserEvent::Timeout => {
                if !self.done {
                    println!("SMOKE_TIMEOUT");
                }
                el.exit();
            }
            UserEvent::Pty => {
                let bytes = self.pump.as_ref().map(|p| p.drain()).unwrap_or_default();
                if bytes.is_empty() {
                    return;
                }
                if let Some(t) = self.term.as_mut() {
                    t.feed(&bytes);
                    let replies = t.take_pty_writes();
                    if !replies.is_empty() {
                        let _ = self.pty.as_mut().unwrap().write(&replies);
                    }
                }
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
                self.check_smoke(el);
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(new) => {
                self.resize(new.width, new.height);
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let bytes: Vec<u8> = match event.logical_key {
                    Key::Named(NamedKey::Enter) => vec![b'\r'],
                    Key::Named(NamedKey::Backspace) => vec![0x7f],
                    Key::Named(NamedKey::Tab) => vec![b'\t'],
                    Key::Named(NamedKey::Space) => vec![b' '],
                    Key::Character(s) => s.as_bytes().to_vec(),
                    _ => Vec::new(),
                };
                if !bytes.is_empty() {
                    if let Some(p) = self.pty.as_mut() {
                        let _ = p.write(&bytes);
                    }
                }
            }
            _ => {}
        }
    }
}

impl App {
    fn check_smoke(&mut self, el: &ActiveEventLoop) {
        if !self.smoke || self.done {
            return;
        }
        let snap = match self.term.as_ref() {
            Some(t) => t.snapshot(),
            None => return,
        };
        if (0..snap.lines).any(|l| snap.row(l).contains("BEBOPSMOKE")) {
            println!("SMOKE_OK");
            self.done = true;
            el.exit();
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let cols = (w / self.cell_w).max(1) as usize;
        let lines = (h / self.cell_h).max(1) as usize;
        if let Some(t) = self.term.as_mut() {
            t.resize(cols, lines);
        }
        if let Some(p) = self.pty.as_mut() {
            let _ = p.resize(cols as u16, lines as u16);
        }
        match self.backend.as_mut() {
            Some(Backend::Gpu { surface, device, config, .. }) => {
                config.width = w;
                config.height = h;
                surface.configure(device, config);
            }
            Some(Backend::Cpu { surface, .. }) => {
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = surface.resize(nw, nh);
                }
            }
            None => {}
        }
    }

    fn draw(&mut self) {
        let snap = match self.term.as_ref() {
            Some(t) => t.snapshot(),
            None => return,
        };
        match self.backend.as_mut() {
            Some(Backend::Gpu { surface, device, queue, config, renderer }) => {
                let frame = match surface.get_current_texture() {
                    Ok(f) => f,
                    Err(_) => {
                        surface.configure(device, config);
                        match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => return,
                        }
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                renderer.render_to_view(device, queue, &snap, &view, config.width, config.height);
                frame.present();
            }
            Some(Backend::Cpu { surface, renderer, .. }) => {
                let pb = renderer.render(&snap);
                let (w, h) = (pb.width.max(1), pb.height.max(1));
                if let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) {
                    let _ = surface.resize(nw, nh);
                }
                if let Ok(mut buf) = surface.buffer_mut() {
                    let n = buf.len().min(pb.data.len());
                    buf[..n].copy_from_slice(&pb.data[..n]);
                    let _ = buf.present();
                }
            }
            None => {}
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) != Some("run") {
        eprintln!("usage: bebop run [--cpu] [--smoke]");
        return;
    }
    let use_cpu = args.iter().any(|a| a == "--cpu");
    let smoke = args.iter().any(|a| a == "--smoke");

    let el = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(e) => e,
        Err(e) => {
            // No display (headless CI) -> not a failure for a windowed dev binary.
            eprintln!("SMOKE_SKIP: cannot build event loop ({e})");
            return;
        }
    };
    let proxy = el.create_proxy();
    if smoke {
        let p = proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(12));
            let _ = p.send_event(UserEvent::Timeout);
        });
    }
    let mut app = App {
        use_cpu,
        smoke,
        proxy,
        window: None,
        backend: None,
        term: None,
        pty: None,
        pump: None,
        cell_w: 1,
        cell_h: 1,
        done: false,
    };
    el.run_app(&mut app).expect("run app");
}
```

- [ ] **Step 2: Build the binary**

Run: `cargo build -p terminal-core --bin bebop 2>&1 | tail -20`
Expected: builds clean. If `softbuffer::Surface::new` / `Context::new` rejects `Arc<Window>`, that is a raw-window-handle wiring issue — raw-window-handle 0.6 provides blanket `HasWindowHandle`/`HasDisplayHandle` impls for `Arc<T>`, and winit 0.30 + wgpu 22 + softbuffer 0.4 all share rwh 0.6; confirm a single `raw-window-handle v0.6` via `cargo tree -p terminal-core | grep raw-window-handle` before changing the handle type. Report BLOCKED with the exact error rather than improvising a different handle wrapper.

- [ ] **Step 3: GPU smoke (best-effort, needs a display)**

Run: `DISPLAY=:1 timeout 30 cargo run -p terminal-core --bin bebop -- run --smoke 2>&1 | tail -15`
Expected: prints an `adapter: ...` line and then `SMOKE_OK` (the loop opened a window, spawned the shell, fed PTY bytes, rendered frames, and saw `BEBOPSMOKE` in the grid). Acceptable alternative outcomes:
- `SMOKE_SKIP: ...` (no display reachable) — the windowed path could not be exercised here; note it. NOT a failure (the GPU render path itself is already gated by Tasks 4/5 offscreen tests, which need only an adapter, not a window).
- A `SMOKE_TIMEOUT` or a panic IS a failure — report it.

- [ ] **Step 4: CPU smoke (best-effort, needs a display)**

Run: `DISPLAY=:1 timeout 30 cargo run -p terminal-core --bin bebop -- run --cpu --smoke 2>&1 | tail -15`
Expected: `SMOKE_OK` (CPU/softbuffer present path), or `SMOKE_SKIP` with no display. A timeout/panic is a failure.

- [ ] **Step 5: Confirm clippy + the full suite are clean**

Run: `cargo clippy --all-targets 2>&1 | tail -20 && cargo test 2>&1 | tail -20`
Expected: no clippy errors (warnings acceptable but prefer clean); all tests green across the workspace.

- [ ] **Step 6: Commit**

```bash
git add crates/terminal-core/src/bin/bebop.rs
git commit -m "feat(terminal-core): bebop run dev binary -- reactive winit loop + GPU/CPU present

ControlFlow::Wait woken by PTY bytes via EventLoopProxy; renders a real \$SHELL
through Terminal -> GpuRenderer (wgpu+glyphon, vsync) or CpuRenderer (softbuffer).
--smoke drives a deterministic command for non-interactive verification.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 8: Results note + push

A short note recording what Plan 3 delivered and the two measured numbers (backend-parity agreement; whether the GPU smoke ran), in the same spirit as the spike's results note.

**Files:**
- Create: `docs/superpowers/specs/2026-06-08-bebop-render-gpu-results.md`

- [ ] **Step 1: Write the note**

Create `docs/superpowers/specs/2026-06-08-bebop-render-gpu-results.md` capturing:
- What shipped: `GpuRenderer` (wgpu+glyphon) behind the shared cosmic-text layout; headless offscreen readback; backend-parity test; `LocalPty` structural single-reader fix; `ReaderPump::start_with_waker`; `bebop run` reactive dev binary (GPU + CPU/softbuffer present).
- The **backend-parity ink-mask agreement** number printed by `cargo test --test backend_parity -- --nocapture` (or "skipped — no adapter").
- The **`bebop run --smoke` outcome** on this machine (`SMOKE_OK` / `SMOKE_SKIP` + adapter name from the `adapter:` line).
- Test count delta and `cargo clippy --all-targets` status.
- Explicit deferrals carried forward: color/attrs (own plan), CPU-first-frame cold-start swap, prewarm/daemon (NAVI), `bebop shell-integration` (Plan 6). Note the dev binary's input encoding is spike-grade (Plan 4 owns it) and the CPU softbuffer path resizes to the rasterized buffer (window remainder undefined) — fine for a dev vehicle.

- [ ] **Step 2: Commit + push**

```bash
git add docs/superpowers/specs/2026-06-08-bebop-render-gpu-results.md
git commit -m "docs(bebop): Plan 3 results note (GPU backend + reactive loop)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
git push -u origin new-seed
```

---

## What Plan 3 delivers (and what is next)

After Task 8: a GPU render backend (`wgpu` + `glyphon`) sharing the CPU backend's `cosmic-text` layout behind matching seams, proven equivalent by a headless parity test; the `LocalPty` dual-reader hazard closed structurally; a reactive damage-driven `winit` loop that renders a real `$SHELL`, presentable on GPU or CPU; all headless-testable except the windowed smoke (which degrades to SKIP without a display).

**Subsequent plans (spec §11):**
- **Plan 4 — Input:** real key→PTY encoding (escape sequences, modifiers, app-cursor/keypad, kitty keyboard protocol) + terminal actions + the shared keymap resolver seam (replaces this binary's spike-grade input).
- **(Deferred) Color/attrs:** extend `GridSnapshot` + `Terminal::snapshot` to carry fg/bg/attrs and teach both renderers — its own TDD cycle.
- **Plan 5 — Config (TOML + hot-reload) + typed `Notice`/`TermStatus` + degradation;** **Plan 6 — OSC 133 marks + `bebop shell-integration`;** **Plan 7 — Performance regression gate** wired to the §2 budgets.
```
