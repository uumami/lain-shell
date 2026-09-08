# BEBOP Input — Key->PTY Encoding Implementation Plan (Plan 4 of N)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn key presses into the correct PTY byte sequences (xterm-style) so a real `$SHELL` in `bebop run` is actually usable — arrows, Ctrl/Alt combos, function keys, Home/End/PageUp/etc., respecting application-cursor-keys (DECCKM) mode.

**Architecture:** A backend-neutral `KeyInput` (key + modifiers) lives in `lain-types` so `terminal-core` never depends on `winit`. A **pure** `encode(&KeyInput, app_cursor) -> Option<Vec<u8>>` in `terminal-core::keys` does the xterm escape-sequence encoding (table-testable, no terminal state). `Terminal::on_input(&KeyInput) -> InputOutcome` reads its own DECCKM mode from `alacritty_terminal` and delegates to `encode`, returning `Bytes(..)` or `Unhandled`. The `bebop` binary tracks modifier state (winit delivers it via a separate `ModifiersChanged` event), translates a winit `KeyEvent` into the neutral `KeyInput`, calls `on_input`, and writes the bytes to the PTY — replacing the four-arm spike-grade input.

**Tech Stack:** Rust, the existing `lain-types` / `terminal-core` workspace, `alacritty_terminal` 0.24 (`TermMode::APP_CURSOR` via `term.mode()`), `winit` 0.30 (`KeyEvent` + `ModifiersChanged`).

**Source of truth:** `docs/superpowers/specs/2026-06-08-bebop-terminal-core-design.md` §6 (input — two layers; layer 1 = key->PTY encoding lives in BEBOP, always; "protocol correctness depending on terminal state, *not* user-configurable") and §7 (`on_input(ev) -> InputOutcome`). This is build-order step 4 from §11.

**Scope (LOCKED, per decision):** This plan builds **layer 1 only** — key->PTY byte encoding. **Deferred (own cycle, "Plan 4b"):** the action-bindings layer (the shared keymap resolver seam + BEBOP terminal actions copy/paste/scroll/zoom) — its consumers (clipboard, a selection model, NAVI's action vocabulary) do not exist yet, so building it now is YAGNI/risk. **Also deferred:** the **kitty keyboard protocol** (the `DISAMBIGUATE_ESC_CODES` / `REPORT_*` mode flags) — a 5-layer opt-in enhancement; this cycle is the xterm baseline that bash/vim/tmux expect by default. **Also deferred:** **application-keypad** mode (DECKPAM) — correct numpad app-mode needs winit physical-key/location mapping; `app_cursor` (DECCKM) is the high-value mode and is done here. Because the seam is `InputOutcome` (an enum), adding an `Action(..)` variant later is additive.

**Builds on Plans 1-3** (committed + pushed on `new-seed`): `lain-types` (`ByteStream`/`GridSnapshot`/`Cursor`/`Damage`/`TermError`); `terminal-core` exports `Terminal` (`new`/`feed`/`snapshot`/`resize`/`take_pty_writes`), `LocalPty`, `ReaderPump`, `CpuRenderer`/`GpuRenderer`; the `bebop run [--cpu] [--smoke]` dev binary whose `KeyboardInput` handler is the spike-grade four-arm match this plan replaces.

---

## File structure

```
crates/lain-types/src/lib.rs            # add Modifiers, NamedKey, Key, KeyInput, InputOutcome (+tests)
crates/terminal-core/src/keys.rs        # NEW: pure encode(&KeyInput, app_cursor) -> Option<Vec<u8>> (+table tests)
crates/terminal-core/src/terminal.rs    # add Terminal::on_input; read APP_CURSOR via alacritty TermMode
crates/terminal-core/src/lib.rs         # mod keys; re-export the lain-types input seam types
crates/terminal-core/src/bin/bebop.rs   # track modifiers; translate winit KeyEvent -> KeyInput; call on_input
```

`keys.rs` is a pure module (no `winit`, no `wgpu`, no terminal state) — that is what makes the encoding exhaustively unit-testable.

---

## Task 1: Neutral input seam types in `lain-types`

The host (binary/NAVI) speaks `winit`; `terminal-core` must not. So the seam carries a backend-neutral key representation. These are plain data types (no deps beyond std), matching `lain-types`' role.

**Files:**
- Modify: `crates/lain-types/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add this test to the `mod tests` block at the bottom of `crates/lain-types/src/lib.rs`:
```rust
    #[test]
    fn key_input_constructs_and_compares() {
        let a = KeyInput {
            key: Key::Named(NamedKey::Up),
            mods: Modifiers { ctrl: true, ..Modifiers::default() },
        };
        let b = KeyInput {
            key: Key::Named(NamedKey::Up),
            mods: Modifiers { ctrl: true, ..Modifiers::default() },
        };
        assert_eq!(a, b);
        assert!(a.mods.ctrl && !a.mods.alt);
        assert_eq!(Key::Char('x'), Key::Char('x'));
        assert_eq!(NamedKey::F(5), NamedKey::F(5));
        // InputOutcome is the on_input return seam (Bytes | Unhandled).
        assert_eq!(InputOutcome::Bytes(vec![0x1b]), InputOutcome::Bytes(vec![0x1b]));
        assert_ne!(InputOutcome::Bytes(vec![0x1b]), InputOutcome::Unhandled);
    }
```

- [ ] **Step 2: Run it, verify it FAILS to compile**

Run: `cargo test -p lain-types 2>&1 | tail -15`
Expected: FAIL — `Modifiers`, `Key`, `NamedKey`, `KeyInput`, `InputOutcome` do not exist yet.

- [ ] **Step 3: Add the types**

In `crates/lain-types/src/lib.rs`, add these definitions (place them after the `Cursor` struct, before `GridSnapshot`):
```rust
/// Keyboard modifier state, backend-neutral. `logo` = Super/Command/Windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub logo: bool,
}

/// A functional (non-text) key. `F(n)` is a function key (F1..F12 used now).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),
}

/// A key press, backend-neutral: either a produced character (post-shift, e.g.
/// '!' for Shift+1) or a functional key. The host translates its native key
/// events (winit, etc.) into this; `terminal-core` never sees a toolkit type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Named(NamedKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
    pub key: Key,
    pub mods: Modifiers,
}

/// Result of feeding a key press to a terminal: bytes to write to the PTY, or
/// unhandled (the host may bind it to an action -- a future `Action(..)` variant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOutcome {
    Bytes(Vec<u8>),
    Unhandled,
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p lain-types 2>&1 | tail -15`
Expected: PASS (existing 2 tests + the new one).

- [ ] **Step 5: Commit**

```bash
git add crates/lain-types/src/lib.rs
git commit -m "feat(lain-types): backend-neutral key input seam (KeyInput/Key/NamedKey/Modifiers/InputOutcome)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: Pure xterm key encoder (`terminal-core::keys`)

The heart of the plan: a pure function turning a `KeyInput` (+ the DECCKM app-cursor flag) into the bytes a terminal sends to the PTY. No terminal state owned here; the mode is a parameter, which is what makes it exhaustively table-testable.

**Files:**
- Create: `crates/terminal-core/src/keys.rs`
- Modify: `crates/terminal-core/src/lib.rs` (add `mod keys;`)

- [ ] **Step 1: Create `keys.rs` with the encoder + failing tests**

Create `crates/terminal-core/src/keys.rs`:
```rust
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
        // Normal mode -> CSI; app-cursor mode -> SS3; modified -> always CSI 1;mod.
        assert_eq!(encode(&named(NamedKey::Up, false, false, false), false), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode(&named(NamedKey::Up, false, false, false), true), Some(b"\x1bOA".to_vec()));
        assert_eq!(encode(&named(NamedKey::Up, true, false, false), false), Some(b"\x1b[1;2A".to_vec()));
        assert_eq!(encode(&named(NamedKey::Up, false, false, true), true), Some(b"\x1b[1;5A".to_vec())); // ctrl wins over app
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
```

- [ ] **Step 2: Wire the module**

In `crates/terminal-core/src/lib.rs`, add `mod keys;` to the `mod` block (e.g. after `mod terminal;`). Do not re-export `keys` publicly — it is consumed only by `Terminal::on_input` (Task 3). Keep all other lines.

- [ ] **Step 3: Run the encoder tests**

Run: `cargo test -p terminal-core --lib keys:: 2>&1 | tail -20`
Expected: all 4 test fns PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/terminal-core/src/keys.rs crates/terminal-core/src/lib.rs
git commit -m "feat(terminal-core): pure xterm key->PTY encoder (keys::encode)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `Terminal::on_input` (reads DECCKM, delegates to the encoder)

Wire the encoder into the `Terminal` handle: `on_input` reads the live application-cursor-keys mode from `alacritty_terminal` and returns an `InputOutcome`. This is the public seam the host calls; it keeps mode-reading inside `terminal-core` so the host never touches `alacritty`.

**Files:**
- Modify: `crates/terminal-core/src/terminal.rs`
- Modify: `crates/terminal-core/src/lib.rs` (re-export the seam types)

- [ ] **Step 1: Write the failing tests**

Add these tests to the `mod tests` block in `crates/terminal-core/src/terminal.rs`:
```rust
    #[test]
    fn on_input_encodes_arrow_in_normal_mode() {
        let t = Terminal::new(20, 5);
        let up = lain_types::KeyInput {
            key: lain_types::Key::Named(lain_types::NamedKey::Up),
            mods: lain_types::Modifiers::default(),
        };
        assert_eq!(t.on_input(&up), lain_types::InputOutcome::Bytes(b"\x1b[A".to_vec()));
    }

    #[test]
    fn on_input_respects_application_cursor_mode() {
        let mut t = Terminal::new(20, 5);
        t.feed(b"\x1b[?1h"); // DECCKM set -> application cursor keys
        let up = lain_types::KeyInput {
            key: lain_types::Key::Named(lain_types::NamedKey::Up),
            mods: lain_types::Modifiers::default(),
        };
        assert_eq!(t.on_input(&up), lain_types::InputOutcome::Bytes(b"\x1bOA".to_vec()));
    }

    #[test]
    fn on_input_unhandled_for_unmapped_key() {
        let t = Terminal::new(20, 5);
        let f20 = lain_types::KeyInput {
            key: lain_types::Key::Named(lain_types::NamedKey::F(20)),
            mods: lain_types::Modifiers::default(),
        };
        assert_eq!(t.on_input(&f20), lain_types::InputOutcome::Unhandled);
    }
```

- [ ] **Step 2: Run, verify they FAIL to compile**

Run: `cargo test -p terminal-core --lib terminal:: 2>&1 | tail -15`
Expected: FAIL — `Terminal::on_input` does not exist.

- [ ] **Step 3: Implement `on_input`**

In `crates/terminal-core/src/terminal.rs`, add the `TermMode` import. The existing `use alacritty_terminal::term::{Config, Term};` line becomes:
```rust
use alacritty_terminal::term::{Config, Term, TermMode};
```
Then add this method inside `impl Terminal` (e.g. after `snapshot`):
```rust
    /// Encode a key press for the PTY, honoring this terminal's live modes
    /// (application cursor keys / DECCKM). Returns `Unhandled` for keys this
    /// layer does not encode (the host may bind them to actions later).
    pub fn on_input(&self, input: &lain_types::KeyInput) -> lain_types::InputOutcome {
        let app_cursor = self.term.mode().contains(TermMode::APP_CURSOR);
        match crate::keys::encode(input, app_cursor) {
            Some(bytes) => lain_types::InputOutcome::Bytes(bytes),
            None => lain_types::InputOutcome::Unhandled,
        }
    }
```

- [ ] **Step 4: Re-export the seam types**

In `crates/terminal-core/src/lib.rs`, extend the `lain_types` re-export so the binary (and NAVI later) get the input types from `terminal_core`. Change the existing line:
```rust
pub use lain_types::{ByteStream, Cursor, Damage, GridSnapshot};
```
to:
```rust
pub use lain_types::{
    ByteStream, Cursor, Damage, GridSnapshot, InputOutcome, Key, KeyInput, Modifiers, NamedKey,
};
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p terminal-core --lib terminal:: 2>&1 | tail -15`
Expected: all PASS (existing terminal tests + the 3 new `on_input` tests).

- [ ] **Step 6: Commit**

```bash
git add crates/terminal-core/src/terminal.rs crates/terminal-core/src/lib.rs
git commit -m "feat(terminal-core): Terminal::on_input encodes keys honoring DECCKM mode

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Wire real input into the `bebop` binary

Replace the spike-grade four-arm `KeyboardInput` match with: track modifier state (winit delivers it via a separate `ModifiersChanged` event), translate the winit `KeyEvent` into a neutral `KeyInput`, call `Terminal::on_input`, and write `Bytes` to the PTY.

**Files:**
- Modify: `crates/terminal-core/src/bin/bebop.rs`

- [ ] **Step 1: Update imports**

In `crates/terminal-core/src/bin/bebop.rs`, the `terminal_core` use-list must include the input types, and the winit keyboard import must be aliased to avoid clashing with the neutral `Key`/`NamedKey`. Change:
```rust
use terminal_core::{cell_size, CpuRenderer, GpuRenderer, LocalPty, ReaderPump, Renderer, Terminal};
```
to:
```rust
use terminal_core::{
    cell_size, CpuRenderer, GpuRenderer, InputOutcome, Key, KeyInput, LocalPty, Modifiers, NamedKey,
    ReaderPump, Renderer, Terminal,
};
```
and change:
```rust
use winit::keyboard::{Key, NamedKey};
```
to:
```rust
use winit::event::Modifiers as WinitModifiers;
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamed};
```

- [ ] **Step 2: Add a modifier-state field to `App`**

In the `struct App { ... }` definition, add a field:
```rust
    mods: WinitModifiers,
```
and in `main`'s `App { ... }` construction add:
```rust
        mods: WinitModifiers::default(),
```

- [ ] **Step 3: Handle `ModifiersChanged` and replace the `KeyboardInput` arm**

In `window_event`, add a `ModifiersChanged` arm and replace the existing `KeyboardInput` arm. The `WindowEvent::CloseRequested`/`Resized`/`RedrawRequested` arms stay as-is. Replace the whole existing `WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => { ... }` block with these two arms:
```rust
            WindowEvent::ModifiersChanged(m) => {
                self.mods = m;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                let key = match &event.logical_key {
                    // Space arrives as a named key but is a printable char to the PTY.
                    WinitKey::Named(WinitNamed::Space) => Some(Key::Char(' ')),
                    WinitKey::Named(n) => translate_named(*n).map(Key::Named),
                    WinitKey::Character(s) => s.chars().next().map(Key::Char),
                    _ => None,
                };
                if let Some(key) = key {
                    let st = self.mods.state();
                    let ki = KeyInput {
                        key,
                        mods: Modifiers {
                            shift: st.shift_key(),
                            alt: st.alt_key(),
                            ctrl: st.control_key(),
                            logo: st.super_key(),
                        },
                    };
                    if let Some(t) = self.term.as_ref() {
                        let outcome = t.on_input(&ki);
                        if let InputOutcome::Bytes(bytes) = outcome {
                            if let Some(p) = self.pty.as_mut() {
                                let _ = p.write(&bytes);
                            }
                        }
                    }
                }
            }
```

- [ ] **Step 4: Add the winit->neutral named-key translation**

Add this free function near the bottom of `crates/terminal-core/src/bin/bebop.rs` (outside `impl App`, e.g. just above `fn main`):
```rust
/// Map the subset of winit named keys this cycle encodes into the neutral
/// `NamedKey`. Unmapped keys return `None` (the press is ignored for now).
fn translate_named(n: WinitNamed) -> Option<NamedKey> {
    Some(match n {
        WinitNamed::Enter => NamedKey::Enter,
        WinitNamed::Tab => NamedKey::Tab,
        WinitNamed::Backspace => NamedKey::Backspace,
        WinitNamed::Escape => NamedKey::Escape,
        WinitNamed::ArrowUp => NamedKey::Up,
        WinitNamed::ArrowDown => NamedKey::Down,
        WinitNamed::ArrowLeft => NamedKey::Left,
        WinitNamed::ArrowRight => NamedKey::Right,
        WinitNamed::Home => NamedKey::Home,
        WinitNamed::End => NamedKey::End,
        WinitNamed::PageUp => NamedKey::PageUp,
        WinitNamed::PageDown => NamedKey::PageDown,
        WinitNamed::Insert => NamedKey::Insert,
        WinitNamed::Delete => NamedKey::Delete,
        WinitNamed::F1 => NamedKey::F(1),
        WinitNamed::F2 => NamedKey::F(2),
        WinitNamed::F3 => NamedKey::F(3),
        WinitNamed::F4 => NamedKey::F(4),
        WinitNamed::F5 => NamedKey::F(5),
        WinitNamed::F6 => NamedKey::F(6),
        WinitNamed::F7 => NamedKey::F(7),
        WinitNamed::F8 => NamedKey::F(8),
        WinitNamed::F9 => NamedKey::F(9),
        WinitNamed::F10 => NamedKey::F(10),
        WinitNamed::F11 => NamedKey::F(11),
        WinitNamed::F12 => NamedKey::F(12),
        _ => return None,
    })
}
```

- [ ] **Step 5: Build + clippy**

Run: `cargo build -p terminal-core --bin bebop 2>&1 | tail -15 && cargo clippy --bin bebop 2>&1 | tail -10`
Expected: clean build, no clippy errors. If winit's `Key`/`NamedKey` variant names differ from the aliases used (e.g. `ArrowUp` vs `Up`), fix MINIMALLY to the winit 0.30 names and note it — the winit 0.30 `NamedKey` enum uses `ArrowUp`/`ArrowDown`/`ArrowLeft`/`ArrowRight`, `Enter`, `Backspace`, `Tab`, `Escape`, `Home`, `End`, `PageUp`, `PageDown`, `Insert`, `Delete`, `F1`..`F35`, `Space`. If a name is wrong, report the exact variant the compiler suggests.

- [ ] **Step 6: Smoke test (best-effort, needs a display)**

Run: `DISPLAY=:1 timeout 30 cargo run -p terminal-core --bin bebop -- run --smoke 2>&1 | tail -8`
Expected: `SMOKE_OK` (the `--smoke` path writes its command directly to the PTY, so it is unaffected by the input change — this confirms the binary still builds and runs end-to-end). `SMOKE_SKIP` (no display) is acceptable; `SMOKE_TIMEOUT`/panic is a failure. Note: real interactive input correctness is covered by the pure `keys::encode` table tests and the `on_input` tests, not by the smoke run.

- [ ] **Step 7: Full suite**

Run: `cargo test 2>&1 | tail -20`
Expected: all green (the new lain-types test, the 4 keys tests, the 3 on_input tests, plus everything from Plans 1-3).

- [ ] **Step 8: Commit**

```bash
git add crates/terminal-core/src/bin/bebop.rs
git commit -m "feat(terminal-core): bebop forwards real keys via Terminal::on_input

Tracks modifier state (winit ModifiersChanged), translates winit key events to
the neutral KeyInput, and writes the encoded bytes to the PTY -- replacing the
spike-grade four-arm input. Arrows/Ctrl/Alt/function keys now work.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Results note + push

**Files:**
- Create: `docs/superpowers/specs/2026-06-08-bebop-input-results.md`

- [ ] **Step 1: Write the note**

Create `docs/superpowers/specs/2026-06-08-bebop-input-results.md` capturing:
- What shipped: neutral `KeyInput`/`Key`/`NamedKey`/`Modifiers`/`InputOutcome` seam in `lain-types`; pure `keys::encode` (xterm-style); `Terminal::on_input` reading DECCKM; `bebop` real input wiring (modifier tracking + winit translation), replacing the spike input.
- Coverage: the `keys::encode` table (printable, Ctrl C0 bytes, Alt-prefix, cursor keys normal/app/modified, tilde keys, F1-F12, shift-Tab) and the `on_input` DECCKM test; the full workspace test count and clippy status; the `bebop run --smoke` outcome.
- Deferrals carried forward: **kitty keyboard protocol** (DISAMBIGUATE/REPORT_* flags detected by alacritty but not encoded); **application-keypad** (DECKPAM) mode; **action-bindings layer** ("Plan 4b": the keymap-resolver seam + BEBOP terminal actions copy/paste/scroll/zoom, gated on selection/clipboard/NAVI); Alt+named-key and modified-Enter nuances. Note `terminal-core` deliberately stays `winit`-free (the binary owns the translation).
- Next: Plan 5 (config TOML + hot-reload + typed `Notice`/`TermStatus` + degradation), then Plan 6 (OSC 133 marks + shell-integration), Plan 7 (perf gate), plus the deferred Plan 4b and color/attrs cycle.

Keep it concise (~40-70 lines), factual, ASCII-only (no emojis).

- [ ] **Step 2: Commit + push**

```bash
git add docs/superpowers/specs/2026-06-08-bebop-input-results.md
git commit -m "docs(bebop): Plan 4 results note (key->PTY input encoding)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
git push origin new-seed
```

---

## What Plan 4 delivers (and what is next)

After Task 5: a real terminal you can type into — arrows, Ctrl/Alt combos, function keys, Home/End/Page keys, app-cursor-mode-aware — driven by a pure, exhaustively table-tested encoder behind the `Terminal::on_input` seam, with `terminal-core` still free of any toolkit dependency.

**Subsequent plans (spec §11):**
- **Plan 4b — action bindings:** the shared keymap-resolver seam (`chord + binding-config -> action-id`) + BEBOP terminal actions (copy/paste/scroll/zoom) + the `InputOutcome::Action(..)` variant, when selection/clipboard and NAVI's action vocabulary exist.
- **Plan 5 — Config (TOML + hot-reload) + typed `Notice`/`TermStatus` + degradation.**
- **Plan 6 — OSC 133 marks + `bebop shell-integration`;** **Plan 7 — performance regression gate.**
- **(Deferred) kitty keyboard protocol; application-keypad mode; color/attrs.**
```
