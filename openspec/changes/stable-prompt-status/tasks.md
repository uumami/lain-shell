## 1. Prompt Format

- [x] 1.1 Update the generated zsh prompt in `crates/lain-core/src/pty.rs` so `lambda_` always renders with the same identity color.
- [x] 1.2 Add a compact non-zero exit status marker that appears separately from `lambda_`.
- [x] 1.3 Preserve the current directory and optional git branch layout without adding a right prompt.

## 2. Verification

- [x] 2.1 Run `cargo check` and confirm it passes.
- [x] 2.2 Launch lain-shell and run `false`; confirm the next prompt shows the failure marker while `lambda_` keeps the identity color.
- [x] 2.3 Run `true` or `ls`; confirm the failure marker disappears and `lambda_` keeps the same identity color.
- [ ] 2.4 Run `htop`, exit it, and confirm the prompt identity marker color remains stable.
