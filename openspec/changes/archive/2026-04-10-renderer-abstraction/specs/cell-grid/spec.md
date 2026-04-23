## ADDED Requirements

### Requirement: CellGrid extracts cell data from Term
The `CellGrid` struct SHALL extract cell data from `alacritty_terminal::Term` by locking the `Arc<FairMutex<Term>>`, iterating `renderable_content().display_iter`, and producing a flat `Vec<CellInfo>` indexed by `row * cols + col`. The lock SHALL be released before any rendering work begins.

#### Scenario: Cell extraction produces correct grid
- **WHEN** the Term contains visible content (e.g., shell prompt with colored text)
- **THEN** `CellGrid::extract()` produces a `Vec<CellInfo>` where each entry has the correct character, foreground color, background color, and bold flag matching the Term's state

#### Scenario: Lock is held only during extraction
- **WHEN** `CellGrid::extract()` is called
- **THEN** the `FairMutex` lock is acquired, cells are copied into the grid, and the lock is released before the method returns — rendering never holds the Term lock

### Requirement: CellGrid tracks cursor state
The `CellGrid` SHALL include cursor position (column, row) and cursor shape (Block, Underline, Beam) extracted from `renderable_content().cursor`.

#### Scenario: Cursor position is accurate
- **WHEN** the user types a character and the cursor advances
- **THEN** `CellGrid.cursor.col` and `CellGrid.cursor.row` reflect the new position after the next extraction

### Requirement: CellGrid provides row-level dirty detection
The `CellGrid` SHALL compare the current frame's cells against the previous frame's cells and produce a `dirty_rows: Vec<bool>` indicating which rows changed. Dirty detection is based on cell content (character, colors, bold) only — cursor position changes SHALL NOT mark rows as dirty. On the first frame, all rows SHALL be marked dirty.

#### Scenario: Only changed rows are marked dirty
- **WHEN** one row of terminal content changes between frames (e.g., new character typed)
- **THEN** `dirty_rows` has `true` only for the rows that differ from the previous frame

#### Scenario: Cursor movement alone does not dirty rows
- **WHEN** the cursor moves between frames but no cell content changes
- **THEN** all entries in `dirty_rows` are `false`

#### Scenario: First frame marks all rows dirty
- **WHEN** `CellGrid::extract()` is called for the first time (no previous frame)
- **THEN** all entries in `dirty_rows` are `true`

#### Scenario: Grid resize marks all rows dirty
- **WHEN** the terminal grid size changes (cols or rows differ from previous frame)
- **THEN** all entries in `dirty_rows` are `true` and `prev_cells` is reset

### Requirement: CellGrid owns the FontSystem
The `CellGrid` SHALL own the `cosmic_text::FontSystem` instance. Backends SHALL receive `&mut FontSystem` when preparing text for rendering.

#### Scenario: FontSystem is created once
- **WHEN** `CellGrid` is constructed
- **THEN** a single `FontSystem` is created and reused across all subsequent frames
