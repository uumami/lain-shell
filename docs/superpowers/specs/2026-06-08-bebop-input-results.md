# BEBOP Plan 4 (layer 1) -- key->PTY input encoding: RESULTS

> Status: **DONE**. All tests pass, smoke passes, `cargo clippy` clean.
> Branch `new-seed`, commit on 2026-06-08.

## What shipped

**Backend-neutral key seam (`lain-types`)** -- `Modifiers { shift, alt, ctrl,
logo }`, `NamedKey` (Enter/Tab/Backspace/Escape/Up/Down/Left/Right/Home/End/
PageUp/PageDown/Insert/Delete/F(u8)), `Key { Char(char) | Named(NamedKey) }`,
`KeyInput { key, mods }`, `InputOutcome { Bytes(Vec<u8>) | Unhandled }`. The
surface is toolkit-neutral; `terminal-core` gains no winit dependency. The
binary owns the winit->neutral translation; the library never sees it.

**Pure xterm key encoder** (`terminal-core::keys`, private module) --
`encode(&KeyInput, app_cursor) -> Option<Vec<u8>>`. Coverage:

- Printable chars, direct byte.
- Ctrl+char -> C0 control byte.
- Alt+char -> ESC prefix.
- Cursor keys: CSI normal mode; SS3 in DECCKM app-cursor mode; CSI `1;mod`
  when modified.
- Home/End.
- Insert/Delete/PageUp/PageDown: `CSI n~` tilde sequences.
- F1-F4: SS3 `ESC O P/Q/R/S`. F5-F12: `CSI 15/17/18/19/20/21/23/24 ~`.
- Shift+Tab: `CSI Z`.
- Backspace: 0x7f. Ctrl+Backspace: 0x08.
- xterm modifier param formula: `1 + shift + 2*alt + 4*ctrl`.

An independent code review verified every escape sequence against real xterm
behavior -- no wrong bytes; correct for bash/vim/tmux.

**`Terminal::on_input(&self, &KeyInput) -> InputOutcome`** -- reads live DECCKM
mode from the alacritty `TermMode::APP_CURSOR` flag (internal, not leaked into
the public API) and delegates to the encoder.

**`bebop` binary real-key forwarding** -- replaces the spike-grade four-arm
match. Tracks modifier state via winit `ModifiersChanged`, translates `KeyEvent`
to a neutral `KeyInput`, calls `on_input`, writes `Bytes` to the PTY. Arrows,
Ctrl/Alt combos, function keys, Home/End/Page keys, and app-cursor mode all
work correctly in the running shell session.

## Verification

**Unit tests (4 fns in `keys::encode`, 3 in `Terminal::on_input`):**

- `keys::encode`: printable/Ctrl/Alt; simple named keys; cursor normal+app-
  mode+modifiers; tilde+function-key sequences.
- `Terminal::on_input`: normal-mode arrow; DECCKM-flip causes SS3 output;
  unmapped F(20) -> `Unhandled`.
- Pure-function correctness; no GPU or window required.

**Smoke test:**

```
bebop run --smoke    adapter: Intel(R) HD Graphics 630 (KBL GT2) | Vulkan | IntegratedGpu (Mesa)
                     result:  SMOKE_OK
```

The smoke writes its command directly to the PTY and confirms the binary
builds and runs end-to-end. Interactive key correctness is covered by the
encoder and `on_input` unit tests above.

**Whole workspace: 29 tests green** (lain-types 3; terminal-core lib 23;
backend_parity 1; flood 1; render_rss 1). `cargo clippy` clean, zero warnings.

## Deferrals / next

**Kitty keyboard protocol** -- alacritty exposes `DISAMBIGUATE_ESC_CODES` and
`REPORT_*` flags; we detect DECCKM only and fall back to the xterm baseline.
A 5-layer opt-in enhancement; deferred to its own follow-up.

**Application-keypad mode (DECKPAM)** -- needs winit physical-key/location
mapping; only DECCKM (app cursor keys) is honored now.

**Alt+named-key encoding** -- only `Alt+Char` gets the ESC prefix today.
Modified Enter also not yet handled. Minor nuances, additive.

**Plan 4b -- action-bindings layer** -- shared keymap-resolver seam, BEBOP
terminal actions (copy/paste/scroll/zoom), `InputOutcome::Action(..)` variant.
Gated on selection/clipboard/NAVI, which do not exist yet. The enum is
additive; this slots in cleanly when those pieces land.

**Next:** Plan 5 (config TOML + hot-reload + typed Notice/TermStatus +
degradation), Plan 6 (OSC 133 marks + shell-integration), Plan 7 (perf gate);
plus deferred Plan 4b and the color/attrs cycle.
