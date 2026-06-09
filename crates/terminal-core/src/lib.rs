//! terminal-core (codename BEBOP): single-terminal primitive.

mod pty;
mod pump;
mod render;
mod render_gpu;
mod terminal;

pub use lain_types::{ByteStream, Cursor, Damage, GridSnapshot};
pub use pty::LocalPty;
pub use pump::ReaderPump;
pub use render::{cell_size, CpuRenderer, PixelBuffer, Renderer};
pub use render_gpu::{try_headless_gpu, GpuRenderer};
pub use terminal::Terminal;
