# font-metrics Specification

## Purpose
TBD - created by archiving change render-quality. Update Purpose after archive.
## Requirements
### Requirement: Cell width is measured from the loaded font
The cell width SHALL be derived by laying out 10 repetitions of the character `'M'` in the configured monospace font at the physical font size, dividing the resulting line width by 10, and rounding to the nearest integer pixel. The hardcoded `font_size * 0.6` multiplier SHALL NOT be used.

#### Scenario: Font advance measured at initialization
- **WHEN** the renderer is initialized with a monospace font at size 14px
- **THEN** `cell_width` equals the rounded average advance of 'M' in that font, not 8.4

#### Scenario: Fallback when measurement fails
- **WHEN** the font layout for 'M' returns zero or an invalid width
- **THEN** `cell_width` falls back to `font_size * 0.6` and a warning is logged

### Requirement: Font size scales with the window's scale_factor
The physical font size used for rendering SHALL equal `logical_font_size * scale_factor`. The canonical `logical_font_size` SHALL be 14.0. On a 1x display, physical equals logical. On a 2x display, physical font size is 28.0.

#### Scenario: 1x display
- **WHEN** `window.scale_factor()` returns 1.0
- **THEN** physical font size is 14.0 and cell metrics are measured at 14.0px

#### Scenario: 2x retina display
- **WHEN** `window.scale_factor()` returns 2.0
- **THEN** physical font size is 28.0 and cell metrics are measured at 28.0px

### Requirement: Font metrics are recomputed on scale_factor change
The renderer SHALL handle `WindowEvent::ScaleFactorChanged` by recomputing the physical font size and cell metrics at the new scale, then triggering a resize of the terminal grid to match.

#### Scenario: Window moved between monitors with different DPI
- **WHEN** `ScaleFactorChanged` fires with a new scale factor
- **THEN** cell_width, cell_height, and terminal cols/rows are all recomputed
- **THEN** the PTY and term grid are resized accordingly

### Requirement: cell_metrics() returns physical pixel dimensions
The `cell_metrics() -> (f32, f32)` method on both renderers SHALL return `(cell_width, cell_height)` in physical pixels. Callers SHALL NOT apply scale_factor on top of this return value.

#### Scenario: cell_metrics used for terminal grid sizing
- **WHEN** `cell_metrics()` is called
- **THEN** dividing physical window width by the returned cell_width gives the correct number of terminal columns

