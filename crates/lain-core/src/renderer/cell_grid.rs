use std::sync::Arc;

use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};
use cosmic_text::{Color, FontSystem};

use crate::terminal::JsonLessListener;

const PREFERRED_MONOSPACE_FAMILIES: &[&str] = &[
    "JetBrainsMono Nerd Font Mono",
    "JetBrainsMono Nerd Font",
    "Symbols Nerd Font Mono",
];

/// Flattened cell info extracted from alacritty_terminal's Term.
#[derive(Clone, PartialEq)]
pub struct CellInfo {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
}

/// Cursor position and shape, extracted from Term.
#[derive(Clone, PartialEq)]
pub struct CursorState {
    pub col: usize,
    pub row: usize,
    pub shape: CursorShape,
    pub visible: bool,
}

/// Shared cell extraction layer consumed by both GPU and CPU renderers.
pub struct CellGrid {
    pub cells: Vec<CellInfo>,
    pub prev_cells: Vec<CellInfo>,
    pub dirty_rows: Vec<bool>,
    pub cursor: CursorState,
    pub cols: u16,
    pub rows: u16,
    pub font_system: FontSystem,
    /// Lines scrolled above the live viewport (0 = at bottom). Used for
    /// mouse hit-testing when the user has scrolled into history.
    pub display_offset: usize,
}

impl CellGrid {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        configure_terminal_font(&mut font_system);

        Self {
            cells: Vec::new(),
            prev_cells: Vec::new(),
            dirty_rows: Vec::new(),
            cursor: CursorState {
                col: 0,
                row: 0,
                shape: CursorShape::Block,
                visible: true,
            },
            cols: 0,
            rows: 0,
            font_system,
            display_offset: 0,
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        let size = cols as usize * rows as usize;
        self.cells.resize(
            size,
            CellInfo {
                c: ' ',
                fg: Color::rgb(204, 204, 204),
                bg: Color::rgba(0, 0, 0, 0),
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                dim: false,
            },
        );
        self.prev_cells.clear();
        self.dirty_rows = vec![true; rows as usize];
    }

    pub fn extract(&mut self, term: &Arc<FairMutex<Term<JsonLessListener>>>) {
        let cols = self.cols;
        let rows = self.rows;
        let size = cols as usize * rows as usize;

        // Reset cells to default
        let default_cell = CellInfo {
            c: ' ',
            fg: Color::rgb(204, 204, 204),
            bg: Color::rgba(0, 0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            dim: false,
        };
        self.cells.clear();
        self.cells.resize(size, default_cell);

        // Lock Term and extract
        {
            let term = term.lock();
            // Read display_offset first — needed to map grid line → display row.
            // History cells have negative line values; the formula is:
            //   display_row = grid_line + display_offset
            let display_offset = term.grid().display_offset() as i32;
            let content = term.renderable_content();
            let colors = content.colors;

            for indexed in content.display_iter {
                let col = indexed.point.column.0;
                let grid_line = indexed.point.line.0; // i32; negative = scrollback history
                let display_row = grid_line + display_offset;
                if display_row < 0 || display_row >= rows as i32 {
                    continue;
                }
                let row = display_row as usize;
                if col < cols as usize {
                    let idx = row * cols as usize + col;
                    let cell = &indexed.cell;
                    self.cells[idx] = CellInfo {
                        c: cell.c,
                        fg: ansi_to_color(&cell.fg, colors),
                        bg: ansi_to_color(&cell.bg, colors),
                        bold: cell.flags.contains(Flags::BOLD),
                        italic: cell.flags.contains(Flags::ITALIC),
                        underline: cell.flags.contains(Flags::UNDERLINE),
                        strikethrough: cell.flags.contains(Flags::STRIKEOUT),
                        dim: cell.flags.contains(Flags::DIM),
                    };
                }
            }

            let cursor = content.cursor;
            // Map cursor grid line to display row the same way
            let cursor_display_row = {
                let gl = cursor.point.line.0 + display_offset;
                if gl >= 0 { gl as usize } else { 0 }
            };
            self.cursor = CursorState {
                col: cursor.point.column.0,
                row: cursor_display_row,
                shape: cursor.shape,
                visible: true,
            };
            self.display_offset = display_offset as usize;

            // Apply selection highlight
            if let Some(ref selection) = term.selection {
                if let Some(range) = selection.to_range(&*term) {
                    let offset = self.display_offset as i32;
                    for row in 0..rows as usize {
                        for col in 0..cols as usize {
                            let line = Line(row as i32 - offset);
                            let point = Point::new(line, Column(col));
                            let selected = if range.is_block {
                                let (min_line, max_line) = if range.start.line <= range.end.line {
                                    (range.start.line, range.end.line)
                                } else {
                                    (range.end.line, range.start.line)
                                };
                                let (min_col, max_col) = if range.start.column <= range.end.column {
                                    (range.start.column, range.end.column)
                                } else {
                                    (range.end.column, range.start.column)
                                };
                                line >= min_line
                                    && line <= max_line
                                    && point.column >= min_col
                                    && point.column <= max_col
                            } else {
                                let after_start = line > range.start.line
                                    || (line == range.start.line
                                        && point.column >= range.start.column);
                                let before_end = line < range.end.line
                                    || (line == range.end.line && point.column <= range.end.column);
                                after_start && before_end
                            };
                            if selected {
                                let idx = row * cols as usize + col;
                                if let Some(cell) = self.cells.get_mut(idx) {
                                    cell.fg = Color::rgb(255, 255, 255);
                                    cell.bg = Color::rgb(67, 132, 211);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Term lock released

        // Compute dirty rows by comparing against prev_cells
        if self.prev_cells.len() != size {
            // Size changed or first frame — all dirty
            self.dirty_rows = vec![true; rows as usize];
        } else {
            self.dirty_rows.clear();
            self.dirty_rows.reserve(rows as usize);
            for row in 0..rows as usize {
                let start = row * cols as usize;
                let end = start + cols as usize;
                let dirty = self.cells[start..end] != self.prev_cells[start..end];
                self.dirty_rows.push(dirty);
            }
        }

        // Swap for next frame comparison
        self.prev_cells.clone_from(&self.cells);
    }
}

fn configure_terminal_font(font_system: &mut FontSystem) {
    let preferred_family = font_system.db().faces().find_map(|face| {
        face.families
            .iter()
            .map(|(family, _)| family.as_str())
            .find(|family| PREFERRED_MONOSPACE_FAMILIES.contains(family))
            .map(str::to_owned)
    });

    if let Some(family) = preferred_family {
        log::info!("using terminal monospace font family `{family}`");
        font_system.db_mut().set_monospace_family(family);
    } else {
        log::warn!(
            "no preferred Nerd Font monospace family found; keeping cosmic-text default monospace"
        );
    }
}

/// Convert alacritty ANSI color to cosmic-text Color.
pub fn ansi_to_color(color: &AnsiColor, colors: &alacritty_terminal::term::color::Colors) -> Color {
    match color {
        AnsiColor::Named(named) => {
            let (r, g, b) = match named {
                NamedColor::Black => (0, 0, 0),
                NamedColor::Red => (204, 0, 0),
                NamedColor::Green => (78, 154, 6),
                NamedColor::Yellow => (196, 160, 0),
                NamedColor::Blue => (52, 101, 164),
                NamedColor::Magenta => (117, 80, 123),
                NamedColor::Cyan => (6, 152, 154),
                NamedColor::White => (211, 215, 207),
                NamedColor::BrightBlack => (85, 87, 83),
                NamedColor::BrightRed => (239, 41, 41),
                NamedColor::BrightGreen => (138, 226, 52),
                NamedColor::BrightYellow => (252, 233, 79),
                NamedColor::BrightBlue => (114, 159, 207),
                NamedColor::BrightMagenta => (173, 127, 168),
                NamedColor::BrightCyan => (52, 226, 226),
                NamedColor::BrightWhite => (238, 238, 236),
                NamedColor::Foreground => (204, 204, 204),
                NamedColor::Background => (13, 13, 18),
                _ => (204, 204, 204),
            };
            Color::rgb(r, g, b)
        }
        AnsiColor::Spec(rgb) => Color::rgb(rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(idx) => {
            if let Some(rgb) = colors[*idx as usize] {
                Color::rgb(rgb.r, rgb.g, rgb.b)
            } else {
                xterm_256_color(*idx)
            }
        }
    }
}

fn xterm_256_color(idx: u8) -> Color {
    if idx < 16 {
        Color::rgb(204, 204, 204)
    } else if idx < 232 {
        let idx = idx - 16;
        let r = (idx / 36) % 6;
        let g = (idx / 6) % 6;
        let b = idx % 6;
        let to_val = |c: u8| if c == 0 { 0u8 } else { 55 + 40 * c };
        Color::rgb(to_val(r), to_val(g), to_val(b))
    } else {
        let val = 8 + 10 * (idx - 232);
        Color::rgb(val, val, val)
    }
}
