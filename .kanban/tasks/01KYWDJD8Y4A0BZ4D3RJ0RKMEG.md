---
assignees:
- claude-code
position_column: todo
position_ordinal: cc80
title: Unvalidated session_id reaches read_ralph/write_ralph file paths (path traversal)
---
`session_id` arrives from the caller and is used to build a filesystem path — `.ralph/<session_id>.md` — with no validation. A value containing `../` escapes the `.ralph/` directory.

Two findings, one root cause. Split out of ^634hqth, where the review engine surfaced them. **Pre-existing**: the engine cited `ralph/execute/mod.rs:244` and `:253`, which are PRE-image line numbers; in the post-image those land inside that commit's new doc comment. The code actually described is the pre-existing `execute()` match arms reaching `read_ralph` / `write_ralph`.

## Why it is reachable

`session_id` is not an internal value. It comes in over two paths a caller controls:

- the MCP tool argument
- piped stdin on the CLI — `echo '{"session_id":"..."}' | sah tool ralph ralph check --`

Confirmed reachable from the CLI: `merge_parsed_stdin` (`apps/swissarmyhammer-cli/src/main.rs`) merges arbitrary stdin JSON/YAML straight into the tool arguments.

So `{"session_id":"../../../../tmp/pwned"}` writes outside `.ralph/`. A `ralph set` is a file write, so this is write-side, not only read-side.

## Required change

1. Validate `session_id` before it reaches any path construction. Reject anything that is not a plain identifier — no path separators, no `..`, no absolute prefix, no NUL. A Claude Code session id is an opaque string, so the rule should be a character allowlist, not a blocklist of known-bad sequences.
2. Do it once, at the point the id enters `RalphState` / the path builder, not per call site — there are several.
3. Reject loudly. An invalid id must be an error the caller sees, not a silent fallback to a default file. Note the board has now hit four separate defects in the accept-then-silently-discard family (^1t92gnj, ^t7ebyn8, ^634hqth, ^ezgxksb); do not add a fifth.

Related but do NOT conflate: ACP session ids are opaque strings and must not be ULID-validated. This is about path safety, not format conformance — validate the characters, not the shape.

## Acceptance

- `{"session_id":"../../../../tmp/pwned"}` is rejected with a clear error, and no file is created outside `.ralph/`. Prove RED first — confirm the traversal actually works before the fix, in a temp dir, and never against a real home.
- Same for an absolute path, a `..` segment mid-string, and a separator on both `/` and `\`.
- A legitimate opaque session id — mixed case, digits, hyphens, underscores — still works.
- Validation lives in one place; a test asserts every entry point routes through it.
- `cargo nextest run -E 'rdeps(swissarmyhammer-tools)'`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean. #bug #security