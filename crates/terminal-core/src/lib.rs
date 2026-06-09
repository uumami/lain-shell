//! terminal-core (codename BEBOP): single-terminal primitive.

mod config;
mod keys;
mod pty;
mod pump;
mod render;
mod render_gpu;
mod terminal;

pub use config::{
    config_rejected_notice, load_from_path, parse_toml, ConfigError, ConfigReload,
    ConfigReloader, CursorConfig, CursorStyle, FontConfig, RenderBackendPreference, TermConfig,
};
pub use lain_types::{
    ByteStream, Cursor, Damage, DegradedReason, GridSnapshot, InputOutcome, Key, KeyInput,
    Modifiers, NamedKey, Notice, NoticeAction, NoticeCode, Severity, TermStatus,
};
pub use pty::LocalPty;
pub use pump::ReaderPump;
pub use render::{cell_size, CpuRenderer, PixelBuffer, Renderer};
pub use render_gpu::{try_headless_gpu, GpuRenderer};
pub use terminal::Terminal;
