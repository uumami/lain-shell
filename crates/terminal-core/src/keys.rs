//! Pure key -> PTY byte encoding (xterm-style). No terminal state is owned here;
//! the one relevant mode -- application cursor keys (DECCKM) -- is passed in.
//! The kitty keyboard protocol and application-keypad mode are intentionally out
//! of scope this cycle (xterm baseline; see the Plan 4 scope note).

use lain_types::{Key, KeyInput, NamedKey};

/// xterm modifier parameter: `1 + shift + 2*alt + 4*ctrl`. `None` when no
/// modifier is active (the caller then uses the unmodified escape form).
fn modifier_param(input: &KeyInput) -> Option<u8> {
    let m = input.mods;
    let bits = (m.shift as u8) + 2 * (m.alt as u8) + 4 * (m.ctrl as u8);
    if bits == 0 {
        None
    } else {
        Some(1 + bits)
    }
}

/// Encode a key press into the bytes to send to the PTY, or `None` if the key is
/// not encodable (pure modifier / unmapped key) -- the caller treats `None` as
/// unhandled. `app_cursor` = DECCKM (application cursor keys) is active.
pub fn encode(input: &KeyInput, app_cursor: bool) -> Option<Vec<u8>> {
    match &input.key {
        Key::Char(c) => encode_char(*c, input),
        Key::Named(named) => encode_named(*named, input, app_cursor),
    }
}

fn encode_char(c: char, input: &KeyInput) -> Option<Vec<u8>> {
    let mods = input.mods;
    let mut bytes: Vec<u8> = if mods.ctrl {
        match control_byte(c) {
            Some(b) => vec![b],
            None => c.to_string().into_bytes(), // ctrl with no C0 mapping -> raw char
        }
    } else {
        c.to_string().into_bytes()
    };
    if mods.alt {
        // Alt/Meta: ESC prefix (xterm "metaSendsEscape").
        let mut out = Vec::with_capacity(bytes.len() + 1);
        out.push(0x1b);
        out.append(&mut bytes);
        return Some(out);
    }
    Some(bytes)
}

/// C0 control byte for Ctrl + the given character, xterm-style.
fn control_byte(c: char) -> Option<u8> {
    match c {
        'a'..='z' => Some(c as u8 - b'a' + 1), // ^A=1 .. ^Z=26
        'A'..='Z' => Some(c as u8 - b'A' + 1),
        '@' | ' ' => Some(0),                   // ^@ / ^Space = NUL
        '[' => Some(27),
        '\\' => Some(28),
        ']' => Some(29),
        '^' => Some(30),
        '_' | '/' => Some(31),
        '?' => Some(127),
        _ => None,
    }
}

fn encode_named(key: NamedKey, input: &KeyInput, app_cursor: bool) -> Option<Vec<u8>> {
    let modp = modifier_param(input);
    let b = |s: &str| Some(s.as_bytes().to_vec());
    match key {
        NamedKey::Enter => b("\r"),
        NamedKey::Tab => {
            if input.mods.shift {
                b("\x1b[Z") // CBT (back-tab)
            } else {
                b("\t")
            }
        }
        NamedKey::Backspace => {
            if input.mods.ctrl {
                Some(vec![0x08])
            } else {
                Some(vec![0x7f])
            }
        }
        NamedKey::Escape => Some(vec![0x1b]),
        NamedKey::Up => cursor_like('A', app_cursor, modp),
        NamedKey::Down => cursor_like('B', app_cursor, modp),
        NamedKey::Right => cursor_like('C', app_cursor, modp),
        NamedKey::Left => cursor_like('D', app_cursor, modp),
        NamedKey::Home => cursor_like('H', app_cursor, modp),
        NamedKey::End => cursor_like('F', app_cursor, modp),
        NamedKey::Insert => tilde(2, modp),
        NamedKey::Delete => tilde(3, modp),
        NamedKey::PageUp => tilde(5, modp),
        NamedKey::PageDown => tilde(6, modp),
        NamedKey::F(n) => function_key(n, modp),
    }
}

/// Cursor/Home/End keys whose final byte is in {A,B,C,D,H,F}. SS3 (ESC O) in
/// app-cursor mode with no modifiers; CSI (ESC [) otherwise; a modified key
/// always uses the CSI `1;mod` form.
fn cursor_like(final_byte: char, app_cursor: bool, modp: Option<u8>) -> Option<Vec<u8>> {
    match modp {
        Some(m) => Some(format!("\x1b[1;{m}{final_byte}").into_bytes()),
        None if app_cursor => Some(format!("\x1bO{final_byte}").into_bytes()),
        None => Some(format!("\x1b[{final_byte}").into_bytes()),
    }
}

/// CSI `n ~` keys (Insert/Delete/PageUp/PageDown and F5-F12), optional modifier.
fn tilde(n: u8, modp: Option<u8>) -> Option<Vec<u8>> {
    match modp {
        Some(m) => Some(format!("\x1b[{n};{m}~").into_bytes()),
        None => Some(format!("\x1b[{n}~").into_bytes()),
    }
}

/// F1-F12. F1-F4 use SS3 (ESC O P/Q/R/S) unmodified; F5-F12 use CSI `n ~`.
/// A modified F1-F4 uses the CSI `1;mod` letter form.
fn function_key(n: u8, modp: Option<u8>) -> Option<Vec<u8>> {
    let f1_4 = |letter: char| match modp {
        Some(m) => Some(format!("\x1b[1;{m}{letter}").into_bytes()),
        None => Some(format!("\x1bO{letter}").into_bytes()),
    };
    match n {
        1 => f1_4('P'),
        2 => f1_4('Q'),
        3 => f1_4('R'),
        4 => f1_4('S'),
        5 => tilde(15, modp),
        6 => tilde(17, modp),
        7 => tilde(18, modp),
        8 => tilde(19, modp),
        9 => tilde(20, modp),
        10 => tilde(21, modp),
        11 => tilde(23, modp),
        12 => tilde(24, modp),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lain_types::Modifiers;

    fn k(key: Key, shift: bool, alt: bool, ctrl: bool) -> KeyInput {
        KeyInput { key, mods: Modifiers { shift, alt, ctrl, logo: false } }
    }
    fn ch(c: char, shift: bool, alt: bool, ctrl: bool) -> KeyInput {
        k(Key::Char(c), shift, alt, ctrl)
    }
    fn named(n: NamedKey, shift: bool, alt: bool, ctrl: bool) -> KeyInput {
        k(Key::Named(n), shift, alt, ctrl)
    }

    #[test]
    fn plain_and_modified_chars() {
        assert_eq!(encode(&ch('a', false, false, false), false), Some(b"a".to_vec()));
        assert_eq!(encode(&ch('c', false, false, true), false), Some(vec![3])); // Ctrl-C
        assert_eq!(encode(&ch('a', false, false, true), false), Some(vec![1])); // Ctrl-A
        assert_eq!(encode(&ch('A', false, false, true), false), Some(vec![1])); // Ctrl-Shift-A same C0
        assert_eq!(encode(&ch('b', false, true, false), false), Some(vec![0x1b, b'b'])); // Alt-b
        assert_eq!(encode(&ch('a', false, true, true), false), Some(vec![0x1b, 1])); // Ctrl-Alt-a
        assert_eq!(encode(&ch('[', false, false, true), false), Some(vec![27])); // Ctrl-[
    }

    #[test]
    fn simple_named_keys() {
        assert_eq!(encode(&named(NamedKey::Enter, false, false, false), false), Some(b"\r".to_vec()));
        assert_eq!(encode(&named(NamedKey::Tab, false, false, false), false), Some(b"\t".to_vec()));
        assert_eq!(encode(&named(NamedKey::Tab, true, false, false), false), Some(b"\x1b[Z".to_vec()));
        assert_eq!(encode(&named(NamedKey::Backspace, false, false, false), false), Some(vec![0x7f]));
        assert_eq!(encode(&named(NamedKey::Backspace, false, false, true), false), Some(vec![0x08]));
        assert_eq!(encode(&named(NamedKey::Escape, false, false, false), false), Some(vec![0x1b]));
    }

    #[test]
    fn cursor_keys_respect_app_mode_and_modifiers() {
        assert_eq!(encode(&named(NamedKey::Up, false, false, false), false), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode(&named(NamedKey::Up, false, false, false), true), Some(b"\x1bOA".to_vec()));
        assert_eq!(encode(&named(NamedKey::Up, true, false, false), false), Some(b"\x1b[1;2A".to_vec()));
        assert_eq!(encode(&named(NamedKey::Up, false, false, true), true), Some(b"\x1b[1;5A".to_vec()));
        assert_eq!(encode(&named(NamedKey::Left, false, false, false), false), Some(b"\x1b[D".to_vec()));
        assert_eq!(encode(&named(NamedKey::Home, false, false, false), false), Some(b"\x1b[H".to_vec()));
        assert_eq!(encode(&named(NamedKey::Home, false, false, false), true), Some(b"\x1bOH".to_vec()));
        assert_eq!(encode(&named(NamedKey::End, false, false, false), false), Some(b"\x1b[F".to_vec()));
    }

    #[test]
    fn tilde_and_function_keys() {
        assert_eq!(encode(&named(NamedKey::Delete, false, false, false), false), Some(b"\x1b[3~".to_vec()));
        assert_eq!(encode(&named(NamedKey::Delete, false, false, true), false), Some(b"\x1b[3;5~".to_vec()));
        assert_eq!(encode(&named(NamedKey::PageUp, false, false, false), false), Some(b"\x1b[5~".to_vec()));
        assert_eq!(encode(&named(NamedKey::Insert, false, false, false), false), Some(b"\x1b[2~".to_vec()));
        assert_eq!(encode(&named(NamedKey::F(1), false, false, false), false), Some(b"\x1bOP".to_vec()));
        assert_eq!(encode(&named(NamedKey::F(1), true, false, false), false), Some(b"\x1b[1;2P".to_vec()));
        assert_eq!(encode(&named(NamedKey::F(5), false, false, false), false), Some(b"\x1b[15~".to_vec()));
        assert_eq!(encode(&named(NamedKey::F(12), false, false, false), false), Some(b"\x1b[24~".to_vec()));
        assert_eq!(encode(&named(NamedKey::F(20), false, false, false), false), None); // unmapped
    }
}
