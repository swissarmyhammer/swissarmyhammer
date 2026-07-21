---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky1g6b4g0d91ptpdnzj1ksz0
  text: 'Batch /finish #review picked this next (security > features; review column empty). Highest-stakes remaining item — real arbitrary-file-read via read_working path join, surfaced by ^1wdaqqy''s field run. Iteration 1: delegating to /implement.'
  timestamp: 2026-07-21T04:50:51.152553+00:00
- actor: claude-code
  id: 01ky1gmxd78d2kn99x1bzqgkyy
  text: |-
    Implemented the containment guard at the choke point `read_working` in crates/swissarmyhammer-validators/src/review/scope.rs.

    How containment is enforced:
    - New `confine_to_repo(repo_path, path)`: (a) rejects absolute input outright (Path::join with an absolute arg replaces the base -> the /etc/passwd escape); (b) canonicalizes the repo root once; (c) for an EXISTING candidate, canonicalizes it (follows symlinks, so a repo-internal symlink whose target is outside the root resolves outside and is rejected); (d) for a NOT-yet-existing candidate, normalizes `.`/`..` lexically (no fs access, preserving the absent-path Ok(None) behavior); (e) rejects any resolved path not `starts_with` the canonical root (component-wise, no prefix false-positives).
    - Typed error via new `path_escapes_repo_root` helper -> AvpError::Validator{validator: "scope", message: "path '<path>' escapes the repository root"} — lowercase, no trailing punctuation, full untruncated path. Reused for both the absolute and escape cases (no repeated literal).
    - read_working now routes through confine_to_repo first; Ok(None) for absent, Context error for non-absent read failures unchanged. resolve_glob/resolve_working also read via read_working so the single guard covers them; tracked repo-relative paths are NOT rejected (glob tests green).
    - Updated docs on read_working and resolve_file to state the containment contract; added # Errors docs.

    TDD: wrote 4 failing scope.rs unit tests first (parent traversal, absolute path, escaping symlink [cfg(unix)], nested relative unchanged) — watched the 3 escape tests leak `Ok(Some("TOP SECRET"))` (RED), then implemented to GREEN. Plus MCP-level test review_file_with_a_traversal_path_is_rejected in tools review/tests.rs (review file `../<marker>` returns an error result naming the path).

    Gates (all green): `cargo nextest run -p swissarmyhammer-validators review::scope` 33/33; `cargo nextest run -p swissarmyhammer-tools -E 'test(review)'` 45/45; `cargo fmt`; `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` clean; `cargo check --workspace --all-targets` clean. Left in doing for review.
  timestamp: 2026-07-21T04:58:48.615458+00:00
- actor: claude-code
  id: 01ky1gww6xs2jjkxmhg9cqj0kx
  text: |-
    Adversarial double-check: VERDICT PASS. Confirmed the fix closes the absolute / `..` / symlink read-outside-root hole completely, including the tricky non-existent-leaf symlink case (canonicalize returns NotFound only when a component is missing, in which case read_to_string also hits NotFound -> Ok(None); leaking outside content requires the target to exist, which forces the canonicalize-succeeds branch that rejects the escape). Error-message contract and legitimate-path behavior verified; blast radius is the 3 in-file callers only.

    Applied the one advisory hardening it flagged: confine_to_repo now returns the canonicalized `resolved` path instead of the raw join, so read_working does not re-walk symlinks — closes the theoretical check-then-read TOCTOU window at no cost. Re-ran: scope tests 33/33 green, clippy -D warnings clean, cargo fmt clean.
  timestamp: 2026-07-21T05:03:09.533832+00:00
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffff380
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