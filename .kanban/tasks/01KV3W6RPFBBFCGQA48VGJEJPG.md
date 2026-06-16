---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffad80
project: local-review
title: 'Harden review module: path-traversal confinement, shared probe-evidence renderer, refuted_by rename, missing type docs'
---
## What

Pre-existing review-module issues surfaced by the over-broad review of task 01KV3PJGP487TN46CH8JQD6RP7 (its `review/*.rs` glob swept the whole module, not its two changed files). These are real but were NOT introduced by that task — split here so they're tracked without blocking it. Ordered by severity.

### Substantive
- [x] **Path-traversal confinement** — `drive.rs` `read_text_file_under_repo` now confines via new `confine_under_repo`: canonicalizes `repo_root` + the candidate, requires the canonical target `starts_with` the canonical root, else returns an error mapped to `invalid_params`. Absolute-but-in-repo paths still honored (boundary is location, not shape). Doc comment corrected to describe the enforced confinement.
- [x] **Duplicate `render_probe_evidence`** — extracted one shared `pub(crate) render_probe_evidence(out, results, show_kind)` in `probes.rs`; `fleet.rs` (show_kind=false) and `verify.rs` (show_kind=true) both call it; the two byte-for-byte copies are gone.
- [x] **`refuted_by` misleading wire-contract name** — renamed `VerifiedFinding::refuted_by` → `decided_by` (changes the wire key), updated all writers/readers + the verify.rs module doc to "which layer reached the verdict"; added doc noting `decided_by.is_some()` is NOT the "was refuted" test.
- [x] **`read_at_ref`/`read_working` swallow real errors** — both now return `Result<Option<String>, AvpError>`: only genuine not-found maps to `Ok(None)` (the Added/Deleted signal); permission errors and binary/non-UTF8 blobs propagate as `AvpError::Context`. Callers updated. `git2` promoted to a direct (non-optional) dep so the `git2::ErrorCode::NotFound` discrimination compiles.

### Refactor-altitude (lower priority; address only if cheap)
- [x] `scope.rs` — hoisted `const SCOPE_VALIDATOR: &str = "scope"` (3 hand-written `"scope"` literals).
- [ ] `scope.rs:214` — `scope_review` extract helpers. (deferred — not cheap, scope creep)
- [ ] `fleet.rs:230` — `run_validator_fleet` extract submit/collect. (deferred)
- [ ] `scope.rs` — `bounded_slice` flatten. (deferred)
- [ ] `synthesize.rs:118` — `synthesize` extract render_sections. (deferred)
- [ ] `types.rs:1` — `extract_json_value` extract strategies. (deferred)
- [ ] `synthesize.rs:40` — drop `FleetTally::new`. (deferred — touches many shared-file test call sites)

### Missing type-level docs (nits)
- [x] Verified: `FleetConfig`, `FleetOutcome`, `ProbeOp`, `CATALOG`, `ChangeEntry`, `FileChange`, `ProbeRow`, `ProbeResult`, `VerifyOutcome`, `Candidate` all ALREADY carry `///` type docs (task line numbers were stale). No action needed.

## Acceptance Criteria
- [x] `read_text_file_under_repo` rejects `..`-escape and absolute-path reads outside `repo_root` (canonicalize + `starts_with` check), returning an invalid_params error; legitimate in-repo reads still work.
- [x] One shared `render_probe_evidence`; no second copy in verify.rs.
- [x] The verdict-layer field is renamed to reflect "who decided" (`decided_by`); wire contract corrected; module docs match.
- [x] `read_at_ref`/`read_working` distinguish not-found from real errors.
- [x] `cargo test -p swissarmyhammer-validators` (301 passed) and `cargo clippy -p swissarmyhammer-validators -p swissarmyhammer-tools --all-targets -- -D warnings` green.

## Tests
- [x] `drive.rs`: `read_text_file_under_repo` with `../secret.txt` and `/etc/passwd` → error; in-repo relative + in-repo absolute → Ok.
- [x] `probes.rs`: shared renderer with/without kind annotation + empty/no-rows sentinels.
- [x] `types.rs`: serde test asserting a confirmed finding does NOT serialize a `refuted_by` field and uses `decided_by`.
- [x] `scope.rs`: not-found path → Ok(None); binary/non-UTF8 working file and committed blob → AvpError::Context (not silently added/deleted).

## Workflow
- Used `/tdd` (RED→GREEN for each substantive item).