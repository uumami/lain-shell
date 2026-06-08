//! terminal-core (codename BEBOP): single-terminal primitive.

mod pty;
mod terminal;

pub use pty::LocalPty;
pub use terminal::Terminal;
