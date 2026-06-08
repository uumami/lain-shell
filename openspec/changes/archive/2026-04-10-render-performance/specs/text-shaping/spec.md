## ADDED Requirements

### Requirement: Shared text shaping module produces cosmic-text Buffers
A `text_shaping` module SHALL exist within `lain-core`'s renderer that provides `build_row_buffer()` and `needs_advanced_shaping()`. Both `GpuRenderer` and `CpuRenderer` SHALL use this shared module for all row shaping. No independent `build_row_buffer` implementation SHALL exist in individual backend files.

#### Scenario: GPU backend uses shared shaping
- **WHEN** the GPU renderer builds a cosmic-text Buffer for a dirty row
- **THEN** it calls `text_shaping::build_row_buffer()` rather than an inline implementation

#### Scenario: CPU backend uses shared shaping
- **WHEN** the CPU renderer builds a cosmic-text Buffer for a dirty row
- **THEN** it calls `text_shaping::build_row_buffer()` rather than an inline implementation

### Requirement: ASCII and simple-script rows use Basic shaping
Rows whose cells contain only characters that do not require contextual shaping SHALL be shaped with `Shaping::Basic`. This includes ASCII (U+0000–U+007F), Latin (U+0080–U+024F), Greek, Cyrillic, CJK unified ideographs, box-drawing characters (U+2500–U+257F), and block elements (U+2580–U+259F).

#### Scenario: Pure ASCII row uses Basic shaping
- **WHEN** a terminal row contains only ASCII characters (e.g., `ls` output, shell prompt, compiler errors)
- **THEN** `Shaping::Basic` is used, avoiding the HarfBuzz pipeline

#### Scenario: Box-drawing row uses Basic shaping
- **WHEN** a terminal row contains box-drawing characters (U+2500–U+257F) as used by ncurses TUIs like htop
- **THEN** `Shaping::Basic` is used, avoiding the HarfBuzz pipeline

#### Scenario: CJK row uses Basic shaping
- **WHEN** a terminal row contains CJK unified ideographs (U+4E00–U+9FFF)
- **THEN** `Shaping::Basic` is used, since CJK characters are contextually independent

### Requirement: Complex-script rows use Advanced shaping
Rows whose cells contain characters from scripts that require contextual shaping, bidirectional processing, or combining mark positioning SHALL use `Shaping::Advanced`. This includes Arabic, Hebrew, Devanagari, Thai, other Indic scripts, combining diacritical marks (U+0300–U+036F), and emoji with ZWJ sequences (U+1F000+).

#### Scenario: Row with Arabic characters uses Advanced shaping
- **WHEN** a terminal row contains Arabic characters (U+0600–U+06FF)
- **THEN** `Shaping::Advanced` is used to enable correct joining forms and bidirectional layout

#### Scenario: Row with combining marks uses Advanced shaping
- **WHEN** a terminal row contains combining diacritical marks (U+0300–U+036F)
- **THEN** `Shaping::Advanced` is used to correctly position marks relative to base characters

#### Scenario: Mixed row with complex script uses Advanced shaping
- **WHEN** a terminal row contains a mix of ASCII and Arabic characters
- **THEN** `Shaping::Advanced` is used for the entire row (per-row granularity)

### Requirement: Shaping selection is transparent to callers
Callers of `build_row_buffer()` SHALL NOT need to specify or be aware of the shaping mode. The selection SHALL be internal to `text_shaping` based on the cells provided.

#### Scenario: Caller provides only cell data
- **WHEN** a backend calls `build_row_buffer(font_system, cells, cols, font_size, line_height, cell_width)`
- **THEN** the correct shaping mode is selected internally without the caller passing a shaping parameter
