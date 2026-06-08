//! terminal-core (codename BEBOP): single-terminal primitive.

mod pty;
mod pump;
mod terminal;

pub use lain_types::{ByteStream, Cursor, Damage, GridSnapshot};
pub use pty::LocalPty;
pub use pump::ReaderPump;
pub use terminal::Terminal;
