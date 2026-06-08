## ADDED Requirements

### Requirement: Renderer backend selected at startup
The application SHALL select a renderer backend (GPU or CPU) at startup. The choice SHALL be determined by: (1) `LAIN_RENDERER` environment variable if set, (2) auto-detection if unset.

#### Scenario: Explicit GPU selection
- **WHEN** `LAIN_RENDERER=gpu` is set
- **THEN** the application uses the GPU (wgpu) renderer backend

#### Scenario: Explicit CPU selection
- **WHEN** `LAIN_RENDERER=cpu` is set
- **THEN** the application uses the CPU (softbuffer) renderer backend

#### Scenario: Auto-detection prefers GPU
- **WHEN** `LAIN_RENDERER` is not set and a GPU adapter is available
- **THEN** the application uses the GPU renderer backend

#### Scenario: Auto-detection falls back to CPU
- **WHEN** `LAIN_RENDERER` is not set and `instance.request_adapter()` returns None
- **THEN** the application uses the CPU renderer backend

### Requirement: Renderer is an enum with two variants
The `Renderer` type SHALL be an enum with `Gpu(GpuRenderer)` and `Cpu(CpuRenderer)` variants. Dispatch SHALL use match expressions, not trait objects.

#### Scenario: Enum exhaustiveness
- **WHEN** a new render dispatch point is added
- **THEN** the compiler requires handling both `Gpu` and `Cpu` variants

### Requirement: Backend choice is fixed for session lifetime
Once a backend is selected at startup, it SHALL NOT change for the duration of the application session. Switching backends requires restarting the application.

#### Scenario: No runtime switching
- **WHEN** the application is running with the GPU backend
- **THEN** there is no API or mechanism to switch to CPU without restarting
