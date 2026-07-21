---
assignees:
- claude-code
position_column: todo
position_ordinal: aa80
title: 'Review scope: reject paths escaping the repo root in resolve_file/read_working'
---
## What

Found by the review engine itself during ^1wdaqqy's live field run: `read_working` in `crates/swissarmyhammer-validators/src/review/scope.rs` resolves a user-supplied path with `repo_path.join(path)` and no containment check. Two escapes: relative traversal (`../../other/file`) and — worse — absolute paths, since `Path::join` with an absolute argument **replaces** the base entirely (`repo_path.join("/etc/passwd")` == `/etc/passwd`). The `review file` MCP op passes its `path` argument straight through `scope_for_path` → `Scope::File` → `resolve_file` → `read_working`, so a caller can make the pipeline read any readable file on the machine and ship its content to the review agent. `resolve_glob` also calls `read_working` but only with tracked-file paths (safe today; the guard should still sit at the choke point).

Fix at the choke point, `read_working`:
- Canonicalize `repo_path` once and the joined candidate (via `std::path::absolute` or component-normalization for not-yet-existing files; `canonicalize` for existing ones — note the file may legitimately not exist, which currently maps to `Ok(None)`, so normalization must not require existence).
- Reject any resolved path that is not strictly under the repo root with a typed `AvpError::Validator` naming the offending path in full (never truncated), e.g. "path '<path>' escapes the repository root".
- Reject absolute input paths outright (a review path is always repo-relative by contract).
- `resolve_file`'s doc updated to state the containment contract. Symlink note: after canonicalization a symlink pointing outside the root resolves outside and is rejected by the same check — cover with a test.

## Acceptance Criteria
- [ ] `review file` with `path: "../<anything>"` or any absolute path returns a validator error naming the path; no file content outside the repo root is ever read into scope
- [ ] A repo-internal symlink whose target lies outside the repo root is rejected by the same containment check
- [ ] Normal relative paths (existing, absent → `Ok(None)`, nested) behave exactly as today — all existing scope tests stay green
- [ ] The error message starts lowercase, has no trailing punctuation, and carries the full untruncated path

## Tests
- [ ] Unit tests in the existing `scope.rs` test module (`TestRepo` harness): traversal rejected (`../outside.txt` written next to the repo dir, asserted unread), absolute path rejected, outside-pointing symlink rejected, absent relative path still `Ok(None)`, normal read unchanged
- [ ] MCP-level test in `crates/swissarmyhammer-tools/src/mcp/tools/review/tests.rs`: `review file` op with a traversal path returns an error result, no findings
- [ ] Run: `cargo nextest run -p swissarmyhammer-validators review::scope` and `cargo nextest run -p swissarmyhammer-tools -E 'test(review)'` — green, under 10s per unit test

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #review #security