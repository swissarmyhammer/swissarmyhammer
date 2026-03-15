---
assignees:
- assistant
depends_on:
- 01KKEZAWBW9NMJWNPA0JY7PYTJ
- 01KKEZBH4FK62AN2Z3BT0MCMY8
position_column: done
position_ordinal: d680
title: Refactor statusline to use LSP registry instead of raw SQL
---
## What

Both statusline modules (`languages.rs` and `index.rs`) currently query the code-context SQLite database directly with raw SQL and maintain their own hardcoded language-to-LSP mapping tables. Replace all of this with calls to the LSP registry from card 1 and the `distinct_extensions` query from card 2.\n\nAlso change the \"missing LSP\" message from `missing: pyright,solargraph` to `/lsp to fix` — a clear call-to-action.\n\n### Files to change:\n- `swissarmyhammer-statusline/src/modules/languages.rs` — delete `LANGUAGES` constant and `has_extension()`. Use `swissarmyhammer_lsp::registry::all_servers()` for icon/extension mapping, and `distinct_extensions()` from code-context ops for presence detection.\n- `swissarmyhammer-statusline/src/modules/index.rs` — delete `LSP_SERVERS` constant and `has_extension()` and `find_missing_lsps()`. Use registry. Change missing-LSP output to `/lsp to fix`.\n\n## Acceptance Criteria\n- [ ] No raw SQL in either statusline module\n- [ ] No hardcoded language/extension/LSP tables in statusline crate\n- [ ] `languages.rs` derives icons from LSP YAML specs via registry\n- [ ] `index.rs` shows `/lsp to fix` (yellow) when LSPs are missing with install hints\n- [ ] `/lsp to fix` only appears when there's an actionable fix (install_hint is present)\n- [ ] Icons still dim when LSP is missing (existing behavior preserved)\n\n## Tests\n- [ ] `cargo test -p swissarmyhammer-statusline`\n- [ ] Manual: run statusline in a Rust+JS project, verify icons show, missing LSPs show `/lsp to fix`