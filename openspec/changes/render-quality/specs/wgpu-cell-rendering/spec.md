## MODIFIED Requirements

### Requirement: Surface uses vsync presentation mode
The wgpu surface SHALL be configured with `PresentMode::AutoVsync` instead of `PresentMode::AutoNoVsync`. Frame presentation SHALL be synchronized to the display refresh cycle to eliminate tearing and provide consistent frame timing.

#### Scenario: Surface configured at initialization
- **WHEN** `GpuRenderer::new()` runs
- **THEN** `SurfaceConfiguration::present_mode` is `PresentMode::AutoVsync`

#### Scenario: Surface reconfigured after resize
- **WHEN** `GpuRenderer::resize()` is called
- **THEN** the reconfigured surface retains `PresentMode::AutoVsync`

### Requirement: Cell dimensions come from font-metrics measurement
The `GlyphonRenderer` SHALL NOT compute `cell_width` as `font_size * 0.6`. Instead, `cell_width` SHALL be set from the measured font advance as defined in the `font-metrics` capability.

#### Scenario: Cell width at initialization
- **WHEN** `GlyphonRenderer::new()` is called
- **THEN** `cell_width` is the measured advance of 'M' at the physical font size, rounded to integer pixels
