---
assignees:
- claude-code
position_column: todo
position_ordinal: ffce80
title: Shell skill and tool must send file search to `grep files`, and `rg` never `grep -r`
---
## Problem

An agent session ran this shell command and it did not stop:

```
grep -rn "inline_diagnostics\|fold_in_diagnostics\|..." --include=* . \
  | grep -v "^./target/" | grep -v "^./.git/"
```

It used one CPU core at 99% for more than 13 minutes. Measured facts for this repo:

| Item | Size |
|---|---|
| `target/` | 64 GB, 181,276 files |
| `.code-context/index.db` | 389 MB, read as one binary blob |
| tracked source files | 9,567 |

The same search with `rg` took **0.044 seconds**. The difference is about 20,000 times.

Three causes:

1. `grep -r` does not know about `.gitignore`, so it reads `target/`.
2. `--include=*` cancels every exclusion.
3. `| grep -v "^./target/"` filters the **output**. The scan pays the full read cost before the filter can remove one line.

The current shell guidance says only "Do not pipe to `tail`, `head`, or `grep`". That rule is about output capture. It does not tell the agent which tool searches files, so it did not prevent this.

## Change

Add one rule to the shell guidance: **`grep files` searches files; if you shell out, use `rg`, never `grep -r`.**

The `files` tool op `grep files` is preferred. It uses `ignore::WalkBuilder` with `git_ignore(true)`, `git_global(true)`, `git_exclude(true)`, `ignore(true)`, `parents(true)`, `hidden(true)`, and `BinaryDetection::quit(0)`
(`crates/swissarmyhammer-tools/src/mcp/tools/files/grep/mod.rs:303-314`, `:276`). It skips `target/`, `.git/`, and binary files without help.

Keep the wording the same in both files. This guidance is duplicated by design and a test holds the copies together.

### Files

1. `crates/swissarmyhammer-tools/src/mcp/tools/shell/description.md` — add the rule after the no-pipe paragraph (line 5).
2. `builtin/skills/shell/SKILL.md` — add the same rule after the no-pipe paragraph (line 23). Extend the "Instead of / Run" table with a `grep -rn PATTERN .` row.
3. `crates/swissarmyhammer-skills/tests/shell_output_guidance.rs` — add a marker constant for the new rule and assert it, next to `NO_PIPE_MARKER`.
4. `crates/swissarmyhammer-tools/src/mcp/tools/shell/mod.rs:868-874` — add the same marker to the description marker list.
5. `builtin/skills/code-context/SKILL.md:238` — "For one-off live searches, fall back to Grep/ripgrep." names a denied tool. Point it at the `files` op `grep files`.
6. Regenerate `.skills/`. Never edit `.skills/` by hand.

## Acceptance criteria

- Both `description.md` and `builtin/skills/shell/SKILL.md` state the rule with the same words.
- Both guard tests fail if either copy loses the rule. Prove RED to GREEN.
- The rule names `grep files` as preferred and `rg` as the shell fallback, and it says `grep -r` reads ignored directories.
- `.skills/` is regenerated from source, not edited.
- Full test suite is green.