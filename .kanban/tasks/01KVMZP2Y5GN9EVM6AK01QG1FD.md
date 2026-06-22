---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvmzy6we5nnxzh2fe4yshc4s
  text: |-
    Implemented per the plan (TDD red→green throughout):

    error.rs: added `#[error("{0}")] NotFound(String)` variant with a doc comment distinguishing it from `Pattern` (the regex-only carrier).

    get_callgraph.rs: `resolve_by_name` now returns `NotFound("symbol not found: {name}")` instead of `Pattern(...)`; updated the `get_callgraph` doc comment (was promising Pattern). Updated `test_symbol_not_found` to assert `NotFound` + no "invalid regex pattern" substring. Watched it fail on Pattern first.

    get_blastradius.rs: split the empty-roots branch on `options.symbol` —
    - `Some(symbol)` (named filter matched nothing) → `NotFound("symbol '{symbol}' not found in file '{file}'")`.
    - `None` (whole-file query, file has no indexed symbols) → `Ok(BlastRadius{roots:[],hops:[],totals 0})`, NOT an error.
    Updated the doc comment. Rewrote `test_blast_radius_symbol_not_found` to seed a file WITH a symbol and ask for a missing one → asserts `NotFound`; added `test_blast_radius_whole_file_no_symbols_is_empty_not_error` → asserts `Ok` with empty roots/hops. Both watched RED first (returned Pattern), then GREEN.

    grep_code.rs: left `Pattern` on the genuine `Regex::new` failure (line 86). Existing `test_grep_invalid_pattern` ("[invalid") already guards that the legit Pattern use is intact — confirmed still green.

    MCP boundary (swissarmyhammer-tools mod.rs `context_err`): the match is `ReadOnlyFollower => invalid_request`, catch-all `other => internal_error(format!("{}", other))`. `NotFound`'s `#[error("{0}")]` Display renders the clean unprefixed message through the catch-all — no match-arm change needed, and the bug (misleading prefix) is gone. swissarmyhammer-tools compiles clean.

    Results:
    - `cargo test -p swissarmyhammer-code-context`: 690 lib + 26 integration + 2 doc tests, 0 failed.
    - `cargo clippy -p swissarmyhammer-code-context --all-targets`: clean, exit 0.
    - `cargo build -p swissarmyhammer-tools`: ok.
  timestamp: 2026-06-21T11:44:30.606295+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffcb80
title: 'code-context callgraph/blastradius: not-found errors mislabeled "invalid regex pattern"'
---
## Report (from monitoring agent)

First real use of the structural ops `get callgraph` / `get blastradius` (the search merge got agents using them) exercised two error paths that return confusing messages:

1. `get callgraph {symbol: "build_editor", direction: "inbound"}` on a symbol not in the index →
   `MCP error -32603: invalid regex pattern: symbol not found: build_editor`
2. `get blastradius {file_path: "src/repl.rs"}` on a file with no indexed symbols →
   `MCP error -32603: invalid regex pattern: no symbols found in file 'src/repl.rs' matching '*'`

## Verified root cause (corrects the agent's hypothesis)

Read the code in the `search` worktree. **Both messages are the SAME bug**, and the agent's theory that blastradius compiles `*` as a regex is **incorrect**:

- `CodeContextError::Pattern(String)` has Display `#[error("invalid regex pattern: {0}")]` (`crates/swissarmyhammer-code-context/src/error.rs:23`). It is being misused as a generic carrier for not-found conditions, so the "invalid regex pattern:" prefix is bolted onto plain not-found text.
- `get_callgraph.rs:266` — `find_symbol` returns `Pattern("symbol not found: {name}")` for a legit not-found. Mislabel.
- `get_blastradius.rs:100` — when `find_roots` returns empty it returns `Pattern("no symbols found in file '{}' matching '{}'")`. The `'*'` is just `options.symbol.as_deref().unwrap_or("*")` — a **cosmetic placeholder** in the human message meaning "all symbols." `find_roots` (`get_blastradius.rs:173`) is a plain SQL `SELECT ... WHERE file_path = ?1` + string matching — **there is no regex compilation of `*` anywhere**. So this is the same mislabel, not a separate regex defect.
- The ONLY legitimate use of `Pattern` is `grep_code.rs:86` (`Regex::new(pattern).map_err(... Pattern(e.to_string()))`) — a genuine invalid-regex. That must keep using `Pattern`.

## Fix design

**1. Stop mislabeling not-found as a regex error.**
- Add a dedicated variant to `CodeContextError` (error.rs), e.g. `#[error("{0}")] NotFound(String)` (an unprefixed message; `QueryError` already has `#[error("{0}")]` and could be reused, but a named `NotFound` reads better at the MCP boundary).
- `get_callgraph.rs:266` → return `NotFound("symbol not found: build_editor")`.
- Update the two doc comments that promise `Pattern`: `get_callgraph.rs:99` and `get_blastradius.rs:88`.
- Leave `grep_code.rs:86` on `Pattern`.

**2. blastradius behavior on a file with no indexed symbols (decision baked in — override if preferred).**
- Whole-file query (`options.symbol == None`) whose file has no indexed symbols → **return an empty `BlastRadius`** (`roots: [], hops: [], totals 0`), NOT an error. An unindexed or symbol-free file legitimately has an empty blast radius; erroring forces every caller to special-case a normal condition. This is what the monitoring agent asked for.
- Symbol-filtered query (`options.symbol == Some(x)`) that matches nothing → return `NotFound("symbol '{x}' not found in file '{file}'")` — the caller named a symbol that isn't there, which is a real miss worth signaling (correctly labeled, no "invalid regex pattern" prefix).

## Tests (real-path, in the ops' own test modules)
- callgraph: missing symbol → `NotFound`, and assert the rendered message does NOT contain "invalid regex pattern".
- blastradius: whole-file on an empty/unindexed file → `Ok(BlastRadius)` with empty roots/hops (not `Err`).
- blastradius: bogus `symbol` filter on an indexed file → `NotFound` (message has no "invalid regex pattern" prefix).
- Guard that `grep_code` still returns `Pattern` for a genuinely invalid regex (e.g. `"*"` or `"["`).

## Key files (all in `swissarmyhammer-search` worktree, branch `search`)
- `crates/swissarmyhammer-code-context/src/error.rs` (add `NotFound` variant)
- `crates/swissarmyhammer-code-context/src/ops/get_callgraph.rs` (`find_symbol`, ~line 266 + doc ~99)
- `crates/swissarmyhammer-code-context/src/ops/get_blastradius.rs` (`get_blastradius`/`find_roots`, lines 99-105/173 + doc ~88)
- `crates/swissarmyhammer-code-context/src/ops/grep_code.rs` (leave Pattern as-is; add the regression guard test)

Note: these ops were never invoked in prior runs, so this is first-exposure, not a behavioral regression from the merge — only newly-exercised. No `repo_path`/indexing change needed.