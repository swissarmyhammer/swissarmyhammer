---
assignees:
- claude-code
depends_on:
- 01KTBNNTCCVS81QZV4CFQZV4X1
- 01KTBRSCTFPDWX86B075YY92EK
position_column: todo
position_ordinal: '9480'
project: local-review
title: 'Review tool Doctorable: `check validators` surfaces in `sah doctor`'
---
## What
Wire the review tool's `check validators` into `sah doctor` via the standard `Doctorable` trait — the proper replacement for the deleted AVP validator doctor rule. The review tool overrides the blanket OK default with a real diagnostic.

- Implement `Doctorable::run_health_checks` on the review tool (`crates/swissarmyhammer-tools/src/mcp/tools/review/`) to call the engine's `check validators` (lint every loaded validator: frontmatter valid, globs compile, no stray `trigger`, declared `probes` exist in the catalog) and map the result to `HealthCheck`s:
  - all valid → one `HealthCheck::ok("Validators", "N validators loaded, all valid", "validators")`.
  - each problem → a `HealthCheck::error("Validator <name/path>", "<problem>", Some("<fix>"), "validators")`.
  - Use category `"validators"` (or `"tools"` if that reads better alongside the others — match the existing display grouping).
- Ensure the review tool is registered in `collect_all_health_checks()` so `sah doctor` includes it (the blanket-Doctorable task makes registration enumerate all tools; confirm review is among them).
- Keep checks fast and non-blocking (loader read + lint only — no agent, no review run), per the `Doctorable` contract.
- `is_applicable()` stays true (validators always exist — builtin at minimum).

## Acceptance Criteria
- [ ] The review tool's `Doctorable::run_health_checks` runs `check validators` and returns OK when all validators are valid, Error(s) (with fixes) when any is malformed.
- [ ] `sah doctor` output includes the validators check line(s) under the right category.
- [ ] The check is fast (no agent/review execution) and honors session/work-dir CWD resolution, not `current_dir()`.

## Tests
- [ ] Unit/integration test: with a valid builtin validator set → one OK validators check; with a temp project `./.validators` containing a malformed validator → an Error check naming it with a fix.
- [ ] `collect_all_health_checks()` includes the validators check; `cargo test -p swissarmyhammer-tools` green (doctor + review modules).

## Workflow
- Use `/tdd` — write the OK-vs-malformed health-check tests first, then implement the override + ensure registration. Reuse the engine `check validators` (do not re-lint in the tool). Depends on the review tool (for `check validators`) and the blanket-Doctorable task (for the default + all-tool enumeration).