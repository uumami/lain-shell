# BEBOP OSC 133 Shell Integration Decision

> Date: 2026-06-09
> Status: Decided during Plan 6 brainstorming.

## Decision

Plan 6 should use **manual opt-in shell integration now**, while designing the
shell-integration code so **per-session auto-injection can be added later without
a rewrite**.

## Rationale

Manual opt-in is the correct engineering choice for this cycle because it gives
the best balance of security, maintainability, flexibility, and user experience:

- Security: BEBOP must not silently modify shell startup behavior or edit user
  dotfiles. Shell startup is sensitive and user-specific.
- Best practice: keep BEBOP layered. BEBOP captures OSC 133 marks and exposes
  bounded advisory command-block metadata; NAVI owns block UI later.
- Flexibility: the same bash/zsh/fish snippets can later be reused by
  per-session auto-injection.
- User experience: users get a clear command (`bebop shell-integration <shell>`)
  and explicit instructions now; "works by default" can come later when NAVI has
  a real command-block UI.

## Rejected For Plan 6

Permanent rc-file editing is rejected entirely. A security-oriented terminal must
not mutate user dotfiles behind their back.

Per-session auto-injection is deferred. It is acceptable in principle, but it
adds shell-specific startup complexity before the mark model has real consumers.

Full command-block UX is deferred to NAVI/action-bindings work. OSC 133 marks are
advisory metadata, never a security boundary.

## Target Shape

```text
Plan 6 now:
  terminal-core captures OSC 133 if present
  command_blocks exposes advisory block metadata
  metadata is bounded and never security-trusted
  bebop shell-integration prints bash/zsh/fish snippets
  user opts in manually

Later:
  NAVI renders block UX
  MAGI can consume blocks as advisory context
  optional per-session auto-injection reuses the same snippets
```

