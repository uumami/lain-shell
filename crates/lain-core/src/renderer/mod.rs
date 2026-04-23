pub mod cell_grid;
pub mod cpu;
pub(crate) mod glyphon_backend;
pub mod gpu;
pub(crate) mod rect;
pub(crate) mod text_shaping;

use cosmic_text::FontSystem;

pub use cell_grid::CellGrid;
pub use cpu::CpuRenderer;
pub use gpu::GpuRenderer;
pub use text_shaping::measure_cell_width;

pub enum Renderer {
    Gpu(GpuRenderer),
    Cpu(CpuRenderer),
}

impl Renderer {
    pub fn cell_metrics(&self) -> (f32, f32) {
        match self {
            Renderer::Gpu(r) => r.cell_metrics(),
            Renderer::Cpu(r) => r.cell_metrics(),
        }
    }

    /// Returns true if the caller should request another redraw (surface reconfigured).
    pub fn render_frame(&mut self, grid: &CellGrid, font_system: &mut FontSystem) -> bool {
        match self {
            Renderer::Gpu(r) => r.render_frame(grid, font_system),
            Renderer::Cpu(r) => {
                r.render_frame(grid, font_system);
                false
            }
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        match self {
            Renderer::Gpu(r) => r.resize(width, height),
            Renderer::Cpu(r) => r.resize(width, height),
        }
    }

    /// Update font metrics for a new scale factor without recreating the surface.
    pub fn update_scale(&mut self, scale_factor: f32, font_system: &mut FontSystem) {
        match self {
            Renderer::Gpu(r) => r.update_scale(scale_factor, font_system),
            Renderer::Cpu(r) => r.update_scale(scale_factor, font_system),
        }
    }
}
