---
position_column: done
position_ordinal: o2
title: Fork sem into swissarmyhammer org + subtree into vendor/sem/
---
## What
Fork [Ataraxy-Labs/sem](https://github.com/Ataraxy-Labs/sem) into the swissarmyhammer GitHub org. Subtree it into `vendor/sem/` in swissarmyhammer-tools so we can edit in-place and `git subtree push` corrections back upstream.

Spec: `ideas/code-context-architecture.md` — "Migration from treesitter tool" step 1.

## Acceptance Criteria
- [ ] Fork exists at `swissarmyhammer/sem` on GitHub
- [ ] `vendor/sem/` directory exists in swissarmyhammer-tools via `git subtree add`
- [ ] `vendor/sem/sem-core/Cargo.toml` is a valid crate
- [ ] `sem-core` added to workspace `Cargo.toml` as path dependency: `sem-core = { path = "vendor/sem/sem-core" }`
- [ ] `cargo check -p sem-core` passes (or identifies tree-sitter version issues to resolve)

## Tests
- [ ] `cargo check -p sem-core` compiles successfully
- [ ] Verify `git subtree push` workflow works with a no-op commit