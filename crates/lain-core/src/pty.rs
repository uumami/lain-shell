use alacritty_terminal::event::WindowSize;
use alacritty_terminal::tty;

pub fn create_pty(
    cols: u16,
    rows: u16,
    cell_width: u16,
    cell_height: u16,
) -> std::io::Result<tty::Pty> {
    tty::setup_env();

    let window_size = WindowSize {
        num_lines: rows,
        num_cols: cols,
        cell_width,
        cell_height,
    };

    let options = tty::Options::default();
    tty::new(&options, window_size, 0)
}
