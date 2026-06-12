---
assignees:
- claude-code
depends_on:
- 01KTBN925WPAWDYXS12W5HETEH
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff080
project: local-review
title: Finding data model + structured agent-output schema
---
## What
Define the structured finding type that flows through the whole pipeline — emitted by fleet agents, consumed by the verifier, rendered by synthesis. Lives in the engine crate `swissarmyhammer-validators` (`src/review/types.rs`), per the conversation.

Types:
- `Finding { file: String, line: u32, validator: String, rule: Option<String>, severity: Severity, claim: String, evidence: String, suggestion: Option<String> }`
  - **`validator`** — the source validator (the shard that produced it).
  - **`rule`** — optional: which specific rule inside the validator fired (traceability; agents cite it when known).
  - **`claim`** — what's wrong **and why it matters**; the human-facing sentence the synthesis render uses.
  - **`evidence`** — the *proof* the issue is real (the probe hit or code citation, e.g. "`find_duplicates`: 0.94 match at `bar.rs:88`"). Verifier/audit-facing; NOT the same as "why it matters" (that's in `claim`).
  - **`suggestion`** — the fix (optional).
- `Severity { Blocker, Warning, Nit }` with `serde` rename to lowercase (`blocker`/`warning`/`nit`) matching the review skill's checklist sections.
- `VerifiedFinding { finding: Finding, confirmed: bool, reason: String }`.
- JSON Schema (or serde example) the fleet-agent prompt instructs agents to emit so a raw response parses into `Vec<Finding>`. Reference the existing `parse_validator_response` for fence-stripping; this is a richer multi-finding schema.
- `parse_findings(agent_text) -> Result<Vec<Finding>>` tolerant of prose/``` fences around the JSON.

## Acceptance Criteria
- [x] `Finding` (with `validator`, optional `rule`, and the `claim`/`evidence`/`suggestion` semantics above), `Severity`, `VerifiedFinding` exist with round-trip serde tests.
- [x] `Severity` serializes to exactly `blocker` / `warning` / `nit`.
- [x] `parse_findings` extracts a `Vec<Finding>` from a realistic response (prose + fenced JSON), tolerates a missing `rule`/`suggestion`, and errors clearly on malformed input.
- [x] Field names/semantics are documented on the type (claim = what+why; evidence = proof).

## Tests
- [x] Unit tests: serde round-trip per type (incl. a finding with `rule: None` and one with `Some`); `parse_findings` on (a) clean JSON, (b) JSON in ```json fences with surrounding prose, (c) malformed → `Err`.
- [x] `cargo test -p swissarmyhammer-validators review::types` green.

## Workflow
- Use `/tdd` — round-trip and parser tests first. Reuse fence-stripping from the existing validator response parser rather than writing a new one. Depends on the rename (engine crate name). (Fan-out prompt must request `rule` + the claim/evidence split — handled in the fan-out task.)

## Implementation Notes
- `parse_validator_response`/`extract_json` were removed from the tree in the RuleSet-architecture refactor (commit 89dc90bad). Recovered the proven fence-stripping algorithm from git history (`avp-common/src/validator/executor.rs`) and ported it into `src/review/types.rs` as `extract_json_array`, generalized from a single JSON object (`{}`) to the JSON array (`[]`) findings are emitted as. Honours string literals/escapes so a `]` inside a claim string is not mistaken for the array close.
- New module wired in: `src/review/mod.rs` (re-exports) + `pub mod review;` in `lib.rs`.
- Errors flow through the crate's existing `AvpError::Json` (`#[from] serde_json::Error`).
- Verification: `cargo test -p swissarmyhammer-validators` → 144 lib + 2 doc tests pass, 0 failures; `cargo clippy -p swissarmyhammer-validators --all-targets` clean.