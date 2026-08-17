---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m07wqftkjm2s237404vw29ea
  text: |-
    Research. The three placements of the timing table are three commits of the rule file:

    - the shell placement — the `tool.run` block at `ba36dc365^`
    - the file-list placement — the `tool.run` block at `ba36dc365`
    - the same, with the real path of each listed file — the `tool.run` block on this branch, which is the shipped one

    `run_shell` in `crates/swissarmyhammer-validators/src/doctor.rs` runs a rule script as `bash -c <script> bash`, with the workspace root as the working directory. The re-measurement below ran each of the three blocks the same way.

    This workspace holds 2 `tsconfig.json` projects: `apps/kanban-app/ui` and `apps/mirdan-app/ui`.
  timestamp: 2026-08-17T12:56:46.163643+00:00
- actor: claude-code
  id: 01m07wqtkrknadmmr3xaja1kr0
  text: |-
    Re-measurement. No number of the old table is carried forward. The old readings came from an earlier session, and the file itself says a reading moves between sessions.

    Method: one cycle runs the three blocks one after another, so all three meet the same machine. The cycle repeated 15 times after one warm-up cycle whose readings were dropped. 45 measured runs in all.

    Machine: Apple M5 Max, macOS 27.0. Tools: ts-prune 0.10.3, TypeScript 5.9.3, Node 25.2.1.

    | placement | lowest | highest | spread |
    |---|---|---|---|
    | the shell placement this replaced | 6.03 s | 6.19 s | 0.16 s |
    | the file-list placement | 6.68 s | 7.00 s | 0.32 s |
    | the same, reading the real path of each listed file as well | 6.70 s | 7.29 s | 0.59 s |

    Every one of the 45 runs answered 58 findings, the same bytes on stdout, 0 items declined and 0 bytes on stderr.

    The delta the card is about is now 0.65 s, not 0.55 s. 0.65 s stands ABOVE each of the three spreads (0.16, 0.32, 0.59). More than that, the two sets of readings do not meet: the highest shell reading, 6.19 s, is under the lowest file-list reading, 6.68 s. So the card's second exit applies — more readings put the delta above every spread — and the file states the cost as measured, with the evidence beside it.

    The real-path delta is now 0.02 s, not 0.31 s. It stays noise: it is under each spread, and the two sets of readings lie over each other.

    An earlier attempt piped the driver output through `tail`, which threw away the per-cycle lines of the first shape. The whole set of 15 cycles was taken again and written to a file. Do not pipe the driver through `tail`.
  timestamp: 2026-08-17T12:56:57.208791+00:00
- actor: claude-code
  id: 01m07wr408d0mh6v47dbf999sm
  text: |-
    ### implement — changed
    - evidence: 1 file — builtin/validators/code-hygiene/rules/dead-code-typescript.md. The change is PROSE ALONE. The `tool.run` block is byte for byte the same: every diff hunk stands after line 1014, and the front matter ends at line 425.
    - NOTHING holds this claim true over time. A wall-clock assertion is not a test this repo can keep green, so no test was added. What the file carries instead is the evidence a reader can check: 15 readings for each placement, the lowest and the highest of each, the spread as the difference of the two, the machine, and the three tool versions. The paragraph beside the table states that a reading moves between sessions, so the readings are compared with each other alone.
    - tests: `cargo nextest run -p swissarmyhammer-validators --no-fail-fast` — 821 tests run, 821 passed, 0 failed.
    - next: /review
  timestamp: 2026-08-17T12:57:06.824993+00:00
- actor: claude-code
  id: 01m07xnesc2wdtz21y1wxwbqpe
  text: |-
    ### review — clean
    - evidence: `review sha HEAD~1..HEAD` (126c08c07). counts: 0 findings, 0 confirmed, 0 refuted, 0 attempted. 0 files reviewed. The 4 `.kanban/` files of the commit are dropped by `.reviewignore`. The fifth file, `builtin/validators/code-hygiene/rules/dead-code-typescript.md`, matches no validator: every one of the 15 validators lists source-code globs alone, and not one lists `*.md`. So no rule can make a finding on a prose change to a rule file.
    - The card carries no earlier `## Review Findings` section, so no item stays open.
    - next: card moved to done.
  timestamp: 2026-08-17T13:13:08.140531+00:00
- actor: claude-code
  id: 01m07xp40wg1rtyjx5zzjfrpbf
  text: |
    ### finish iteration 1 — clean
    - implement: changed — 1 file, prose alone. The timing passage is rewritten from a fresh measurement of 45 runs: 15 cycles of 3 placements after one dropped warm-up cycle. The real delta is 0.65 s, not 0.55 s. Each placement states its lowest, highest and spread, with the machine and the tool versions.
    - test: green — cargo nextest run --workspace, 14164 passed, 0 failed. fmt and clippy clean. The test step proved the `tool.run` block byte-identical to HEAD, and re-measured with 36 runs of its own: the delta stands above every spread and the two sets of readings do not meet.
    - commit: 126c08c07
    - review: clean — 0 findings. The engine reviewed 0 files, because no validator of the 15 declares a `*.md` glob.
    - note: nothing holds this claim true over time. A wall-clock number is not a test this repo can keep green. What stands instead is the evidence in the file: the run count, the lowest, the highest, the spread, the machine and the tool versions.
  timestamp: 2026-08-17T13:13:29.884674+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffa980
title: dead-code-typescript states the 0.55 s placement cost without saying it is under a measured spread
---
`builtin/validators/code-hygiene/rules/dead-code-typescript.md` holds a timing table with three placements, three readings each, a lowest and a spread:

| placement | the three readings | lowest | spread |
|---|---|---|---|
| the shell placement this replaced | 6.31 s, 6.72 s, 6.76 s | 6.31 s | 0.45 s |
| the file-list placement | 6.86 s, 6.96 s, 8.18 s | 6.86 s | 1.32 s |
| the same, reading the real path of each listed file as well | 7.17 s, 7.37 s, 7.85 s | 7.17 s | 0.68 s |

Every number of the table is correct. 6.76 - 6.31 = 0.45. 8.18 - 6.86 = 1.32. 7.85 - 7.17 = 0.68. 6.86 - 6.31 = 0.55. 7.17 - 6.86 = 0.31.

The paragraph under the table treats the two deltas differently. It states the 0.31 s and then says it is "under each of the three spreads above, so this measurement does not tell that cost from noise either". It states the 0.55 s as "what the WHOLE placement costs over the shell loop it replaced", and says only that the measurement does not divide it between the two added calls.

The 0.55 s is under the spread of the second row, 1.32 s. So the table does not tell the 0.55 s from noise either, and the text does not say so. A reader takes the 0.55 s as a measured cost and the 0.31 s as noise, and the three readings carry no such difference.

Say of the 0.55 s what the file already says of the 0.31 s: name the spread it stands under, or take more readings until the delta stands above every spread.

This is prose. The shipped script is unaffected. Raised by the round-7 review of ^yxky1aj and filed apart, because the card it was found on carries no behavior defect.

#tool-validators