use alacritty_terminal::event::WindowSize;
use alacritty_terminal::tty;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const LAIN_SHELL_ZSHRC: &str = r#"
if [ -f "$HOME/.zshrc" ]; then
  source "$HOME/.zshrc"
fi

lain_shell_git_prompt() {
  local branch
  branch=$(command git symbolic-ref --quiet --short HEAD 2>/dev/null) || return
  printf '  %s(%s)%s' "%{$fg_bold[red]%}" "$branch" "%{$reset_color%}"
}

PROMPT="%{$fg[cyan]%}%c%{$reset_color%}\$(lain_shell_git_prompt)%(?.. %{$fg_bold[red]%}!%{$reset_color%}):%{$fg_bold[green]%}λ_%{$reset_color%} "
RPROMPT=""
"#;

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

    let mut options = tty::Options::default();
    options.env = terminal_env();
    tty::new(&options, window_size, 0)
}

fn terminal_env() -> HashMap<String, String> {
    let mut env = HashMap::new();

    if let Ok(zdotdir) = ensure_lain_shell_zdotdir() {
        env.insert("ZDOTDIR".to_string(), zdotdir.to_string_lossy().into_owned());
    }

    env
}

fn ensure_lain_shell_zdotdir() -> std::io::Result<PathBuf> {
    let zdotdir = config_home()?.join("lain-shell").join("zsh");
    fs::create_dir_all(&zdotdir)?;
    fs::write(zdotdir.join(".zshrc"), LAIN_SHELL_ZSHRC)?;
    Ok(zdotdir)
}

fn config_home() -> std::io::Result<PathBuf> {
    if let Ok(path) = std::env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var("HOME").map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("HOME is not set: {err}"),
        )
    })?;

    Ok(Path::new(&home).join(".config"))
}
