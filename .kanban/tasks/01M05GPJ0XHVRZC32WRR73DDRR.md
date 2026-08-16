---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m063h3cw2y2rbp26bnkv799s
  text: |-
    Measured first, then wrote. Every number in the rule body comes from ruff 0.14.5 on this machine, against the shipped command line.

    Report, stderr and exit for each shape (each run beside `judged.py`, which holds one `PLR2004` on row 2):

    | the run | report | stderr | exit |
    |---|---|---|---|
    | `judged.py` alone | one `PLR2004` row | 0 bytes | 1 |
    | the unparsable file alone | one `invalid-syntax` row | 0 bytes | 1 |
    | both together | one `PLR2004` AND one `invalid-syntax` | 0 bytes | 1 |
    | a path that holds no file | the `PLR2004` alone | `warning: Failed to lint absent.py: No such file or directory (os error 2)` | 1 |
    | a file whose bytes are not UTF-8 | the `PLR2004` alone | `warning: Failed to lint notutf8.py: stream did not contain valid UTF-8` | 1 |
    | a file with no read permission | the `PLR2004` alone | `warning: Failed to lint noread.py: Permission denied (os error 13)` | 1 |
    | a broken symbolic link | the `PLR2004` alone | `warning: Failed to lint brokenlink.py: No such file or directory (os error 2)` | 1 |
    | a symbolic link that points at itself | the `PLR2004` alone | `warning: Failed to lint looplink.py: Too many levels of symbolic links (os error 62)` | 1 |
    | a directory with no read permission | the `PLR2004` alone | `warning: Encountered error: Permission denied (os error 13)` | 1 |
    | a directory that holds no Python file | the `PLR2004` alone | 0 bytes | 1 |
    | that directory ALONE | `[]` | `warning: No Python files found under the given path(s)` | 0 |
    | `--select ZZ999` | 0 bytes | 93 bytes, `error: invalid value 'ZZ999' for '--select <RULE_CODE>'` | 2 |
    | `--output-format zzz` | 0 bytes | 217 bytes | 2 |

    THE STDERR CHANNEL IS OPEN, so the script forwards the whole of it. Three heads met: `Failed to lint `, `Encountered error: `, and `No Python files found under the given path(s)`. The last two carry NO path, so a scan of one head answers for one head alone. This is the lesson `^hqe8qwv` recorded, and the measurement repeats here.

    A SOUND RUN WRITES 0 BYTES on that channel. Measured over 10 shapes against this command line: `judged.py` alone, `judged.py` two times, a module that names its own limit alone and beside `judged.py`, `judged.py` beside a text file that is not Python, `judged.py` beside a directory with no Python file, a file carrying `# noqa: PLR2004`, a file opening with a byte-order mark, a file holding JSON, and a comparison in an `async def`. That is what makes the whole channel readable as declines.

    The old pipe, measured on each of the three defects:
    1. Over `judged.py` and the unparsable file: 2 findings at exit 0, one of them `{"file":"...broken.py","line":2,"message":"invalid-syntax unexpected EOF while parsing"}` — a magic-numbers finding ruff never reported.
    2. Against a stub ruff that exits 2: exit 0, 0 bytes on stdout — a broken tool as a clean tree.
    3. Over `judged.py` beside the absent path: 1 finding at exit 0 with ruff's UNMARKED `Failed to lint` line, which the engine drops as chatter — the path read as clean.

    The shipped script, over the same shapes: exit 0 with the finding kept and one marked line for each declined item; exit 1 with the rule's own words for a status over 1 and for a report jq cannot read. Over `judged.py` beside the unparsable file, all three refusing paths AND the unreadable directory: 1 finding, 5 marked lines, exit 0.

    The temporary directory is removed on every exit. Measured by counting entries under TMPDIR: 5 clean runs, 1 run that declines 3 items, and 1 run that exits 1 all leave the count unchanged.
  timestamp: 2026-08-16T20:17:07.996504+00:00
- actor: claude-code
  id: 01m063he2p02sh1h3ea07z8rzk
  text: |-
    RED before GREEN. The seven new acceptance tests were driven against the OLD pipe first: 7 of 7 failed, each for its own reason (`it stated []` for the five decline probes, and no break for the two gate probes). The script then made all seven pass. The old pipe was put back and taken away again by rewriting the file, not by a git command.

    Two things outside the rule file had to move with it:

    - `builtin/validators/README.md` quoted this rule's frontmatter word for word and called it "all 25 lines of it, and its `run` is the zero-argument guard this contract states plus one pipe". That sentence is false after the rewrite. The example now shows the keys with the `run` script left out, and the sentence states the true counts: 50 frontmatter lines, a `run` script of 30, beside `missing-docs-python` at 52. No shipped `files`-scope rule kept a short frontmatter to take the example over.
    - `temp_directory.rs` holds a roster COUNT of the rules that make a temporary directory. This rule now makes one, so the count moved 21 → 22. The guard did its job: it broke the moment the roster grew.

    `verify_staged_tree_breaks` and `verify_staging_breaks` drive the REAL tool, and no staged file can make ruff exit over 1 under `--isolated`. The two break probes therefore use `drive_shipped_script_with_stub` plus `assert_shipped_break`, which is the shape `function-length-typescript` (^rfjsx87) takes for the same gate.
  timestamp: 2026-08-16T20:17:18.934117+00:00
- actor: claude-code
  id: 01m063hngwh86mf4bb14axt8n6
  text: |-
    ### implement — changed
    - evidence: 4 files — `builtin/validators/code-hygiene/rules/magic-numbers-python.md` (pipe → script: status gate, filter gate, `PLR2004` selection, whole-stderr declines, `mktemp`+trap, and the measurements in the body), `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/magic_numbers.rs` (7 new acceptance tests, RED verified against the old pipe), `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/temp_directory.rs` (roster count 21 → 22), `builtin/validators/README.md` (the example this rule falsified).
    - tests: `cargo nextest run -p swissarmyhammer-validators` — 785 tests run, 785 passed, 0 skipped. `cargo nextest run -p mirdan` — 503 passed. `cargo clippy -p swissarmyhammer-validators --all-targets` — 0 warnings.
    - next: `/review`.
  timestamp: 2026-08-16T20:17:26.556834+00:00
- actor: claude-code
  id: 01m065w15wg83eqph4rvcwrxnh
  text: |-
    ### test — green

    All commands run directly in this turn (no delegated agents; two earlier speculative sub-agent runs were discarded and every gate re-run by hand per correction).

    - `cargo fmt --check` — clean, exit 0.
    - `cargo nextest run --workspace` run 1: 14127 tests run, 14127 passed, 0 failed, 0 skipped (38 slow, 1 leaky). 290s.
    - `cargo nextest run --workspace` run 2: 14127 tests run, 14127 passed, 0 failed, 0 skipped (33 slow, 1 leaky). 222s. Identical counts to run 1.
    - Go flake (`shipped::function_length_go::*`, 9 tests): 20 sequential rounds at default concurrency — 0 failures. 3 rounds of the full workspace suite at `--test-threads=64` (18-core box) — 1 failure in round 3: `the_shipped_go_function_length_tool_rule_breaks_on_a_file_it_may_not_read`. Root-caused and independently verified: `function-length-go.md`'s golangci-lint cache is named `sah-golangci-lint-$(printf '%s' "$PWD" | cksum | tr -dc '0-9')` under `TMPDIR` and is never removed by design ("the cache stays"). Confirmed 6609 stale `sah-golangci-lint-*` directories accumulated on this machine. Every probe's tempdir path is the same byte length, so `cksum`'s byte-count half contributes no entropy — the real keyspace is the 32-bit CRC alone, and with thousands of same-shaped accumulated names a collision serves a stale warm-cache finding in place of the expected cold-cache result. This bug lives in `function-length-go.md`, untouched by this diff — filed as task ^73pjv4j, not fixed here.
    - The same 64-thread stress runs also surfaced a second, unrelated failure 3/3 times: `swissarmyhammer-diagnostics::leader_watcher::watcher_redreport_on_direct-disk_write`, an LSP/rust-analyzer timing test with a fixed wait deadline that real 64-way CPU oversubscription exceeds. Never fails at default nextest concurrency (both clean full runs above prove this). Pre-existing, untouched by this diff — filed as task ^axr7bvb, not fixed here.
    - Sound-run stderr probe: extracted the `run:` block from `magic-numbers-python.md` (30 lines, matches the README's own count), verified byte-identical to a programmatic re-extraction. Ran against ruff 0.14.5 (matches the rule body's measured version) over a clean file, a file with a genuine `PLR2004` finding, a `# noqa: PLR2004` file, a file with a UTF-8 BOM, and all four together in one invocation — every shape wrote exactly 0 bytes to stderr and exited 0; the finding shape and the combined shape both produced only the expected `PLR2004` JSON row on stdout.
    - `TMPDIR` leak check (isolated empty `TMPDIR`): clean run, finding run, decline run (absent path beside a valid file) and broken run (fake `ruff` on PATH exiting 2) each left the isolated `TMPDIR` with 0 entries before and 0 after — the `trap 'rm -rf "$work"' EXIT` fires on every exit path, including the `exit 1` broken-run branch.
    - Mirdan embed check: `cargo build -p mirdan -vv` confirmed OUT_DIR `target/debug/build/mirdan-2dd4355981eab855/out`; extracted the `"code-hygiene/rules/magic-numbers-python.md"` entry from `builtin_validators.rs` and byte-compared against the file on disk in Python — 19856 bytes both sides, byte-identical.
    - `cargo clippy --workspace --all-targets -- -D warnings` — clean, no warnings.
    - `git status --short` confirms the four files touched by this change (`README.md`, `magic-numbers-python.md`, `magic_numbers.rs`, `temp_directory.rs`) are unchanged from the working tree at the start of this run — nothing staged, nothing committed, nothing reverted.

    No findings against this change. Two pre-existing, unrelated flakes were found and filed as separate cards (^axr7bvb, ^73pjv4j) rather than fixed here or silently dropped.
  timestamp: 2026-08-16T20:58:03.324843+00:00
position_column: doing
position_ordinal: '8280'
title: 'magic-numbers-python is a bare pipe: it reports a parse failure as a finding and reads a broken ruff as clean'
---
`builtin/validators/code-hygiene/rules/magic-numbers-python.md` runs its whole
body in five lines and makes no exit-status decision at all:

    ruff check --isolated --no-cache --select PLR2004 --output-format json "$@" |
      jq -c '.[] | {file: .filename, line: .location.row, message: "\(.code) \(.message)"}'

`builtin/validators/README.md` names each failure this shape carries:

- "A pipe carries one trap... a pipe that ends in `jq` exits 0 whatever the tool
  did. The engine reads exit 0 as 'the script judged the code'."
- "Selection in the pipe is attribution, not exemption." The `.[]` has NO
  `select`, so a row ruff writes for a file it could not PARSE —
  `"code": "invalid-syntax"` — becomes a magic-numbers FINDING, attributed to
  that file and line.
- "Do not stay silent either: a run that reports no finding and exits 0 over an
  item it never judged reads exactly like a clean pass over that item." An
  unreadable path makes ruff write `warning: Failed to lint ...` to stderr and
  exit 0 with `[]`. Nothing reads that stderr.

Three defects, one shape:

1. An `invalid-syntax` row is reported as a magic-numbers finding.
2. A ruff status the command line refused (2) writes 0 bytes, `jq` emits nothing
   and exits 0, so a broken tool reads as a clean tree.
3. A path ruff could not read is silently clean.

The work:

- Measure all three against ruff 0.14.5 and the shipped command line: the
  report, stderr and exit for each.
- Rewrite the pipe as a script: run ruff into a file, test the status against
  the findings status, select the `PLR2004` rows as findings, and write every
  other row and every `warning: Failed to lint ` line under `sah-diagnostic:` at
  exit 0. `function-length-python` holds the worked shape for the same tool and
  the same two decline channels.
- Add the acceptance tests. There are none for any of the three shapes today.
- State each measurement in the rule body.

Found while implementing `^s8d7fva`. #tool-validators #objectivity