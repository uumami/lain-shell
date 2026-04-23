## ADDED Requirements

### Requirement: GpuRenderer uses low-latency present mode
The `GpuRenderer` SHALL configure its wgpu surface with `PresentMode::AutoNoVsync`, delegating platform-specific present mode selection to wgpu. This eliminates the VSync latency floor imposed by `PresentMode::Fifo`.

#### Scenario: Frame delivers without VSync delay where supported
- **WHEN** the GPU is running on a platform where Mailbox or Immediate present modes are available
- **THEN** frames are presented as soon as they are ready, without waiting for the next VSync interval

#### Scenario: Falls back to Fifo where low-latency modes are unavailable
- **WHEN** the GPU backend runs on a platform where only Fifo is available (e.g., certain Wayland compositors)
- **THEN** `AutoNoVsync` transparently falls back to `Fifo` with no error or configuration change required

### Requirement: Suboptimal surface is reconfigured before rendering
When `surface.get_current_texture()` returns `Suboptimal`, the `GpuRenderer` SHALL reconfigure the surface immediately and retry acquiring a texture — identical to how `Outdated` and `Lost` are handled. No suboptimal frame SHALL be presented.

#### Scenario: Suboptimal during live resize triggers reconfigure
- **WHEN** the window is being resized interactively and `get_current_texture()` returns `Suboptimal`
- **THEN** the surface is reconfigured at the current dimensions and a fresh texture is acquired before rendering

#### Scenario: Suboptimal handling matches Outdated handling
- **WHEN** comparing `Suboptimal` and `Outdated` surface states
- **THEN** both follow the same code path: configure → retry → render → present
