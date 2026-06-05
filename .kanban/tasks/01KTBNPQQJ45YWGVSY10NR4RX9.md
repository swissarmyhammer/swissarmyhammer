---
assignees:
- claude-code
depends_on:
- 01KTBNHSR4EVTVJ35MGGD510R2
- 01KTBQR87DKQF750JTJ3G52FZR
position_column: todo
position_ordinal: '8e80'
project: local-review
title: 'Author focused validators: duplication, reuse, data-driven, dead-code (+ re-homed quality)'
---
## What
Promote the exact problems the user complains about — duplicated/copy-pasted code, ignoring shared libraries, needless helpers, hardcoded instead of data-driven — into first-class FOCUSED VALIDATORS (rules-as-data), one concern per validator so each gets its own focused fleet agent. Today these are buried as Layer-2 footnotes a single agent rushes.

Split the monolithic `code-quality` validator into focused validators under `builtin/validators/` (new format: `name`, `description` = the agent mandate, `match.files` globs, `severity`, `probes` — using the catalog names `callers`/`duplicates`/`similar`; most validators have NO probe and that's correct):

| Validator | Concern | `probes` | severity |
|-----------|---------|----------|----------|
| `duplication` | verbatim/near-verbatim copied blocks | `[duplicates]` | blocker |
| `reuse` | reimplements an existing shared fn/library instead of calling it; needless helper wrapping one call site | `[similar]` | warning |
| `data-driven` | hardcoded literals / `match`/`if`-chains over a known set that should be a table; repeated literals → named constant | (none) | warning |
| `dead-code` | added symbol with no inbound callers, not an entry point/export/test; orphaned modules, unreachable branches, commented-out code | `[callers]` | blocker |
| `complexity` | cognitive complexity (re-homed) | (none) | warning |
| `function-length` | over-long functions (re-homed) | (none) | warning |
| `naming` | naming consistency (re-homed) | (none) | nit/warning |
| `no-commented-code` | commented-out code (re-homed) | (none) | nit |
| `magic-numbers` | unexplained literals (re-homed) | (none) | nit |
| `missing-docs` | missing doc comments (re-homed) | (none) | nit |

- Each validator's rule bodies state the carve-outs that prevent over-flagging: `reuse`/`data-driven` honor rule-of-three (two is coincidence, three is a pattern) and no speculative abstraction; `dead-code` exempts entry points, exported public API, and tests.
- Probe names must be the catalog's (`duplicates`/`similar`/`callers`); NO `search_symbol` or `get_blastradius`. In-file validators declare no probes.
- Delete the monolithic multi-concern `code-quality` set once its rules are re-homed; `no-hard-code` ("return 42 to pass a test") goes to the test-integrity validator (safety-migration task), not here.

## Acceptance Criteria
- [ ] `duplication`, `reuse`, `data-driven`, `dead-code` exist as separate focused validators with the probes/severities above; `reuse` is split out from `duplication`.
- [ ] The six re-homed quality validators exist; the monolithic `code-quality` set is gone; no validator carries `trigger`.
- [ ] Every declared probe is a real catalog name (`duplicates`/`similar`/`callers`) and passes `probe_exists`; in-file validators declare none.
- [ ] Each validator names its carve-outs.

## Tests
- [ ] Loader/parse test: every authored `VALIDATOR.md` + `rules/*.md` parses; every declared probe passes `probe_exists`; `cargo test -p swissarmyhammer-validators` green.
- [ ] `check validators` reports OK for the authored set (no malformed frontmatter, no unknown probe).
- [ ] (Behavioral proof that the validators actually catch planted copy-paste / reuse-miss / hardcoding lives in the end-to-end task, not here.)

## Workflow
- Content-authoring task grounded in code. Read `builtin/skills/review/SKILL.md` Layer 2 and the existing `builtin/validators/code-quality/rules/*.md` first; reuse their wording. Pair each probe-bearing validator with its catalog probe. Depends on the format task (`probes` schema) and the probe registry (catalog names + `probe_exists`).