use lain_types::{Notice, NoticeAction, NoticeCode, Severity};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub struct TermConfig {
    pub font: FontConfig,
    pub cursor: CursorConfig,
    pub scrollback_lines: usize,
    pub shell: Option<String>,
    pub render_backend: RenderBackendPreference,
    pub vsync: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub fallback: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CursorConfig {
    pub style: CursorStyle,
    pub blink: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Block,
    Underline,
    Beam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackendPreference {
    Auto,
    Gpu,
    Cpu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Read(String),
    Parse(String),
    Validate(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigReload {
    Unchanged,
    Applied(TermConfig),
    Rejected(Notice),
}

pub struct ConfigReloader {
    path: PathBuf,
    last_seen: Option<SystemTime>,
    current: TermConfig,
}

impl Default for TermConfig {
    fn default() -> Self {
        TermConfig {
            font: FontConfig {
                family: "DejaVu Sans Mono".to_string(),
                size: 14.0,
                fallback: Vec::new(),
            },
            cursor: CursorConfig { style: CursorStyle::Block, blink: true },
            scrollback_lines: 10_000,
            shell: None,
            render_backend: RenderBackendPreference::Auto,
            vsync: true,
        }
    }
}

pub fn parse_toml(input: &str) -> Result<TermConfig, ConfigError> {
    RawTermConfig::from_toml(input)?.finish()
}

impl ConfigReloader {
    pub fn new(path: impl Into<PathBuf>, current: TermConfig) -> Self {
        ConfigReloader { path: path.into(), last_seen: None, current }
    }

    pub fn current(&self) -> &TermConfig {
        &self.current
    }

    pub fn reload_now(&mut self) -> ConfigReload {
        match load_from_path(&self.path) {
            Ok(cfg) => {
                self.current = cfg.clone();
                self.last_seen = modified_at(&self.path).ok().flatten();
                ConfigReload::Applied(cfg)
            }
            Err(err) => ConfigReload::Rejected(config_rejected_notice(err)),
        }
    }

    pub fn reload_if_changed(&mut self) -> ConfigReload {
        let modified = match modified_at(&self.path) {
            Ok(m) => m,
            Err(err) => return ConfigReload::Rejected(config_rejected_notice(err)),
        };
        if modified.is_some() && modified == self.last_seen {
            return ConfigReload::Unchanged;
        }
        self.reload_now()
    }
}

pub fn load_from_path(path: &Path) -> Result<TermConfig, ConfigError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::Read(format!("{}: {e}", path.display())))?;
    parse_toml(&text)
}

pub fn config_rejected_notice(err: ConfigError) -> Notice {
    Notice {
        severity: Severity::Warn,
        code: NoticeCode::ConfigRejected,
        message: format!("config rejected; keeping last-good settings: {err}"),
        action: Some(NoticeAction::ReloadConfig),
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Read(s) => write!(f, "read error: {s}"),
            ConfigError::Parse(s) => write!(f, "parse error: {s}"),
            ConfigError::Validate(s) => write!(f, "validation error: {s}"),
        }
    }
}

impl std::error::Error for ConfigError {}

fn modified_at(path: &Path) -> Result<Option<SystemTime>, ConfigError> {
    std::fs::metadata(path)
        .map_err(|e| ConfigError::Read(format!("{}: {e}", path.display())))
        .and_then(|m| m.modified().map(Some).map_err(|e| ConfigError::Read(e.to_string())))
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTermConfig {
    font: Option<RawFontConfig>,
    cursor: Option<RawCursorConfig>,
    scrollback_lines: Option<usize>,
    shell: Option<String>,
    render_backend: Option<String>,
    vsync: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFontConfig {
    family: Option<String>,
    size: Option<f32>,
    fallback: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCursorConfig {
    style: Option<String>,
    blink: Option<bool>,
}

impl RawTermConfig {
    fn from_toml(input: &str) -> Result<Self, ConfigError> {
        toml::from_str(input).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    fn finish(self) -> Result<TermConfig, ConfigError> {
        let mut cfg = TermConfig::default();
        if let Some(font) = self.font {
            if let Some(family) = font.family {
                if family.trim().is_empty() {
                    return Err(ConfigError::Validate("font.family must not be empty".to_string()));
                }
                cfg.font.family = family;
            }
            if let Some(size) = font.size {
                if !size.is_finite() || !(6.0..=96.0).contains(&size) {
                    return Err(ConfigError::Validate("font.size must be between 6 and 96".to_string()));
                }
                cfg.font.size = size;
            }
            if let Some(fallback) = font.fallback {
                cfg.font.fallback = fallback;
            }
        }
        if let Some(cursor) = self.cursor {
            if let Some(style) = cursor.style {
                cfg.cursor.style = match style.as_str() {
                    "block" => CursorStyle::Block,
                    "underline" => CursorStyle::Underline,
                    "beam" => CursorStyle::Beam,
                    _ => {
                        return Err(ConfigError::Validate(
                            "cursor.style must be block, underline, or beam".to_string(),
                        ))
                    }
                };
            }
            if let Some(blink) = cursor.blink {
                cfg.cursor.blink = blink;
            }
        }
        if let Some(lines) = self.scrollback_lines {
            if lines > 1_000_000 {
                return Err(ConfigError::Validate(
                    "scrollback_lines must be <= 1000000".to_string(),
                ));
            }
            cfg.scrollback_lines = lines;
        }
        if let Some(shell) = self.shell {
            if shell.trim().is_empty() {
                return Err(ConfigError::Validate("shell must not be empty".to_string()));
            }
            cfg.shell = Some(shell);
        }
        if let Some(backend) = self.render_backend {
            cfg.render_backend = match backend.as_str() {
                "auto" => RenderBackendPreference::Auto,
                "gpu" => RenderBackendPreference::Gpu,
                "cpu" => RenderBackendPreference::Cpu,
                _ => {
                    return Err(ConfigError::Validate(
                        "render_backend must be auto, gpu, or cpu".to_string(),
                    ))
                }
            };
        }
        if let Some(vsync) = self.vsync {
            cfg.vsync = vsync;
        }
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_full_toml_config() {
        let cfg = parse_toml(
            r#"
shell = "/bin/zsh"
render_backend = "cpu"
vsync = false
scrollback_lines = 5000

[font]
family = "Commit Mono"
size = 16.5
fallback = ["Symbols Nerd Font"]

[cursor]
style = "beam"
blink = false
"#,
        )
        .unwrap();
        assert_eq!(cfg.shell.as_deref(), Some("/bin/zsh"));
        assert_eq!(cfg.render_backend, RenderBackendPreference::Cpu);
        assert!(!cfg.vsync);
        assert_eq!(cfg.scrollback_lines, 5000);
        assert_eq!(cfg.font.family, "Commit Mono");
        assert_eq!(cfg.font.size, 16.5);
        assert_eq!(cfg.font.fallback, vec!["Symbols Nerd Font"]);
        assert_eq!(cfg.cursor.style, CursorStyle::Beam);
        assert!(!cfg.cursor.blink);
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(matches!(
            parse_toml("render_backend = \"quantum\""),
            Err(ConfigError::Validate(_))
        ));
        assert!(matches!(
            parse_toml("[font]\nsize = 2.0"),
            Err(ConfigError::Validate(_))
        ));
    }

    #[test]
    fn reloader_keeps_last_good_after_bad_reload() {
        let path = temp_path("bebop-config-bad");
        fs::write(&path, "shell = \"/bin/sh\"\n").unwrap();
        let mut reloader = ConfigReloader::new(&path, TermConfig::default());
        let applied = reloader.reload_now();
        assert!(matches!(applied, ConfigReload::Applied(_)));
        assert_eq!(reloader.current().shell.as_deref(), Some("/bin/sh"));

        fs::write(&path, "render_backend = \"wrong\"\n").unwrap();
        let rejected = reloader.reload_now();
        assert!(matches!(rejected, ConfigReload::Rejected(_)));
        assert_eq!(reloader.current().shell.as_deref(), Some("/bin/sh"));
        let _ = fs::remove_file(path);
    }

    fn temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("{label}-{nanos}.toml"))
    }
}
