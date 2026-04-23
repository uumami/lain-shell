## 1. text_shaping Module

- [x] 1.1 Create `crates/lain-core/src/renderer/text_shaping.rs` with `pub(crate) fn needs_advanced_shaping(cells: &[CellInfo]) -> bool` — returns `true` for rows containing combining marks (U+0300–U+036F), Arabic/Hebrew (U+0590–U+06FF), Syriac/Thaana (U+0700–U+07BF), Indic scripts (U+0900–U+0DFF), Thai/Lao/Tibetan (U+0E00–U+0FFF), or emoji/ZWJ range (U+1F000+); `false` for everything else
- [x] 1.2 Add `pub(crate) fn build_row_buffer(font_system: &mut FontSystem, cells: &[CellInfo], cols: u16, font_size: f32, line_height: f32, cell_width: f32) -> Buffer` to `text_shaping.rs` — calls `needs_advanced_shaping(cells)` to select `Shaping::Basic` or `Shaping::Advanced`, builds and returns a shaped `cosmic_text::Buffer`
- [x] 1.3 Add `pub(crate) mod text_shaping;` to `crates/lain-core/src/renderer/mod.rs`

## 2. GPU Backend — Delegate to Shared Shaping

- [x] 2.1 Replace the body of `GlyphonRenderer::build_row_buffer()` in `glyphon_backend.rs` with a 1-line delegation: `text_shaping::build_row_buffer(font_system, cells, cols, self.font_size, self.line_height, self.cell_width)` — the method signature stays unchanged so `GpuRenderer` requires no modification
- [x] 2.2 Remove the now-unused imports from `glyphon_backend.rs` line 1: `Attrs`, `Family`, `Metrics`, `Shaping`, `Weight` — these move to `text_shaping.rs`; remaining imports are `Buffer`, `Color`, `FontSystem`

## 3. CPU Backend — Use Shared Shaping

- [x] 3.1 Remove the `build_row_buffer()` free function from `cpu.rs`
- [x] 3.2 Add `use super::text_shaping;` to the imports in `cpu.rs`, then update `CpuRenderer::render_frame()` to call `text_shaping::build_row_buffer(font_system, row_cells, grid.cols, self.font_size, self.line_height, self.cell_width)` in place of the removed local function

## 4. GPU Surface — Present Mode and Lifecycle

- [x] 4.1 Change `present_mode: wgpu::PresentMode::Fifo` to `present_mode: wgpu::PresentMode::AutoNoVsync` in `GpuRenderer::new()` in `gpu.rs`
- [x] 4.2 Update `render_frame()` surface texture match in `gpu.rs` — handle `Suboptimal` identically to `Outdated`/`Lost`: call `surface.configure()` then retry `get_current_texture()`, rather than rendering the suboptimal frame and configuring afterward

## 5. Verification

- [x] 5.1 Verify `cargo check` passes with no warnings
- [x] 5.2 Verify `cargo build` succeeds for both GPU and CPU paths
- [ ] 5.3 Test GPU backend (`LAIN_RENDERER=gpu`): run `htop`, verify smooth refresh with no visible lag; run `ls`, `grep`, compiler output — all render correctly
- [ ] 5.4 Test CPU backend (`LAIN_RENDERER=cpu`): same — `htop`, `ls`, basic shell usage renders correctly
- [ ] 5.5 Test complex script row: paste Arabic or Devanagari text into the terminal, verify it renders (uses Advanced shaping path, no panic or tofu for common test strings)
- [ ] 5.6 Test box-drawing TUI: run `htop` or `ncdu`, verify box-drawing borders render correctly via Basic shaping path
- [ ] 5.7 Test resize smoothness (GPU): resize window interactively, verify no flash/artifact difference vs before
- [ ] 5.8 Test Ctrl+C responsiveness: run a long-running command, Ctrl+C — verify prompt returns visibly faster than before (AutoNoVsync effect)
