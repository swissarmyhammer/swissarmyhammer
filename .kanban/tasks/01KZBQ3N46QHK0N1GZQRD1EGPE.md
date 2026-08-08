---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzce19bntt4ac30c641xwn8q
  text: |-
    Research done.

    The forbidding sentence is not in the Doctor Pattern section. It is Practice 3 of section 2 (MCP Architecture): "Doctor collects from the tool registry. Don't add health checks outside the `Doctorable` trait." The Doctor Pattern section is in section 4 (Command Line Programs) and describes only the `Doctorable` path. Both places need the update.

    Fact-producer path as shipped:
    - `mirdan::status` — `check_all_doctored()` makes `ComponentStatus` facts; `statuses_to_checks()` converts them to `Check` rows and holds the filter and the scope-pair policy. Consumers: `mirdan::doctor::MirdanDoctor::check_install_stack`, `swissarmyhammer-cli` `commands::doctor::checks::check_install_stack` (test seam `check_install_stack_with`), and `mirdan::status::run_status`, which reads the same facts with no doctor.
    - `swissarmyhammer-validators::doctor` — `check_review_engine()` makes `ReviewEngineStatus` facts; `to_checks()` converts them; `check_review_engine_with()` is the test seam. Consumer: `swissarmyhammer-cli` `commands::doctor::checks::check_review_engine`, which turns a load failure into one Error row.

    No validator rules apply to `.md` (`dump validators` returned rule_count 0). No test reads ARCHITECTURE.md.
  timestamp: 2026-08-06T21:00:28.917131+00:00
- actor: claude-code
  id: 01kzce2wvk88r1kwa08qsz68n3
  text: |-
    ### implement — changed
    - evidence: ARCHITECTURE.md — the Doctor Pattern section (section 4) now names both paths. `Doctorable` is "the component checks itself". The fact producer is "the library reports facts and the CLI makes the rows", with the status struct, the `to_checks()`/`statuses_to_checks()` conversion, the thin loader plus `_with` test seam, and the CLI error-row rule. The two shipped examples are named by module path: `mirdan::status` and `swissarmyhammer-validators::doctor`. Practice 3 of section 2 no longer forbids health checks outside `Doctorable`; it now limits that rule to tools and points at the Doctor Pattern section.
    - next: /review

    Note: the change is documentation only. No validator rules match `.md`, and no test reads ARCHITECTURE.md, so there is no test to run.
  timestamp: 2026-08-06T21:01:21.651047+00:00
- actor: claude-code
  id: 01kzcej1gd1tk95d7pr9j3bk4h
  text: |
    ### review — clean
    - evidence: review sha HEAD~1..HEAD (ee5ae2a20) — findings 0, confirmed 0, refuted 0, attempted 0, failed 0, skipped 0. The commit changes ARCHITECTURE.md and the kanban ledger only; no code file matched a validator.
    - next: none. Task moved to done.
  timestamp: 2026-08-06T21:09:37.933050+00:00
- actor: claude-code
  id: 01kzcejs5sbbw2va33v7z7xadn
  text: |-
    ### finish iteration 1 — clean
    - implement: changed — ARCHITECTURE.md
    - test: green — cargo fmt --check clean, cargo clippy --workspace --all-targets --all-features -D warnings clean, cargo nextest run --workspace 13626 passed
    - commit: ee5ae2a20
    - review: clean — review sha HEAD~1..HEAD, 0 findings; task moved to done
  timestamp: 2026-08-06T21:10:02.169980+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffb380
title: 'ARCHITECTURE.md: document the fact-producer doctor path'
---
Found during ^2hk89aj. The Doctor Pattern section of ARCHITECTURE.md says: do not add health checks outside the `Doctorable` trait. That statement predates the `mirdan::status` fact-producer pattern. Two shipped modules now use the fact-producer path: `mirdan::status::check_install_stack` and `swissarmyhammer-validators::doctor::check_review_engine`.

Work:
- Update the Doctor Pattern section to describe both paths and when each applies:
  - `Doctorable` trait: a component checks itself.
  - Fact producer: a library produces status structs; `to_checks()` converts them to doctor `Check` rows; the CLI wires a thin loader with a `_with` test seam.
- Name the two fact-producer examples by module path.

Acceptance:
- The section no longer forbids the pattern two shipped modules use.

#tool-validators