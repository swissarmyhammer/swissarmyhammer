---
assignees:
- claude-code
position_column: todo
position_ordinal: fd80
title: 'Validator hallucinations: missing-docs/no-magic-numbers fire on items that already comply (3 distinct misreads)'
---
## Symptom

Reproducible false positives from qwen-moe-driven Stop hook validators on `swissarmyhammer-common/src/sample_avp_test.rs`. Three distinct hallucination classes observed in three consecutive Stop runs:

### Class 1: missing-docs flags `pub fn`/`pub struct` items that have doc comments

Verified by `grep -B1 -nE 'pub (struct|fn|const)'`:

```
64-/// Connection settings for a retry-aware client.
65:pub struct RetryClient {
86-/// Construct a default [`RetryClient`] populated with placeholder development values.
87:pub fn build_default_client() -> RetryClient {
181-/// timeout without success. Currently this is a stub that always exhausts.
182:pub fn connect_with_retry() -> Result<(), String> {
208-/// Falls back to `"negligible"` when the value is below every band's threshold.
209:pub fn classify_threshold(value: f64) -> &'static str {
220-/// 4-byte checksum, all big-endian, zero-padded out to 32 bytes.
221:pub fn pack_header(version: u8, flags: u16, length: u32, checksum: u32) -> Vec<u8> {
247-/// of the same length to perform the rotation.
248:pub fn rotate_buffer(buf: &mut [u8], rotation: usize) {
```

Every flagged item has a `///` line directly above its `pub fn` / `pub struct`. Two consecutive runs flagged subsets of these items.

In one of those runs the validator also flagged `score_band_for` — which is `fn`, not `pub fn`. Private. The rule targets public items per the rule body.

### Class 2: missing-docs flags **module-private** `const` items as if they were public

Run 3 (most recent): Stop hook blocked claiming "30 public constants lack documentation comments" — listing `DEFAULT_HOST`, `DEFAULT_PORT`, `SCORE_*`, `THRESHOLD_*`, `HEADER_*`, etc. (44 items in total).

Verified by `grep -nE '^pub const|^const '`:

```
5:const DEFAULT_HOST: &str = "10.244.7.99";
6:const DEFAULT_PORT: u16 = 28734;
... (44 lines total)
```

**Zero of those `const` declarations have `pub`.** They are all module-private. The validator hallucinated visibility on every single one.

### Class 3: no-magic-numbers flags values inside named-const lookup tables

Earlier in the same fixture, after `SCORE_BANDS` and `THRESHOLD_LADDER` were already defined as `const`-named lookup tables, the validator listed every value inside those tables as a magic number:

> SCORE_BANDS contains magic values 2000, 1500, 750, 300, 100, weights 31/19/11/6/3/1, multiplier_steps 5/4/2/1; THRESHOLD_LADDER contains magic percentages 99.97, 94.4, 81.2, 67.8, 53.6, 38.5, 24.9, 13.7, 6.2

Values inside a named constant table are by definition not magic — they are the data of a named lookup. Same shape as Class 2: validator's perception of the syntactic context is wrong.

## Likely root cause

Two leading hypotheses:

### Hypothesis A: validator reads diff, not full file

The Stop-hook prompt provides a `## Files Changed This Turn` block and embedded diff blocks. Items added in a *previous* turn (doc comments, `pub` removal, named consts) are not in *this* turn's diff. If qwen-moe builds its judgment primarily from the diff content rather than calling `read_file` to read the full current state, it will:
- Miss every doc comment that's been there for more than one turn (Class 1)
- Misjudge visibility based on stale or partial mental models (Class 2)
- Skip the structural context that distinguishes a named-table value from a free-floating literal (Class 3)

The recordings (when available — see task `01KQAFT5H1CYQ8YDNAM4J0HD1Q`) would confirm this — if `read_file` is in the tool-call sequence, hypothesis A is wrong; if not, it's almost certainly the cause.

### Hypothesis B: model hallucination under contention

qwen-moe is faster but may produce more confident-sounding wrong assertions. Class 2's specific hallucination — claiming `pub` on items that don't have it — is the one most consistent with B over A: even a diff-only read would show `const FOO` not `pub const FOO`. The visibility error suggests the model is filling in syntactic context it didn't actually see.

Most likely: **A is necessary** (model isn't reading the full file) **and B is amplified by A** (the model fills the gap with confident-sounding hallucination). Fix A and B's surface area shrinks.

## Why this matters

- **The Stop hook becomes useless if it reproducibly blocks on issues that don't exist.** This session blocked the user three turns in a row on three different non-issues. The user has to either edit the file pointlessly (churn), suppress the rule, or stop running the validator.
- **It poisons trust in the validator's true findings.** Earlier in this same session the same validator caught real issues (`cognitive-complexity`, `function-length`, the `answer_for_test` test-cheating pattern). With this many false positives, the user can no longer tell which findings to trust without manually verifying each one.
- **Adding pointless docs to private constants to satisfy the validator is a tax on every legitimate change to the codebase.** The developer is forced to bloat their code to comply with rules the file already complies with.

## Suggested fixes

### Fix 1 (highest leverage): Rule prompt requires `read_file` before passed=false

The rule prompts in `builtin/validators/code-quality/rules/missing-docs.md`, `no-magic-numbers.md` should explicitly require:

> Before issuing a `passed=false` verdict, you MUST call `read_file` on every file in the changed-files list and verify the claim against the current file content. The diff alone is insufficient — items that comply may have done so before the current turn, and visibility (pub/private) is not always visible from a small diff hunk.

Add a one-line `## Required tool usage` section to those rule files. Recordings (task `01KQAFT5H1CYQ8YDNAM4J0HD1Q`) will then show whether the model actually obeys this — and if it doesn't, that's a diagnostic.

### Fix 2: Diff-block formatting that includes context lines around named items

If the diff currently shows pure unified-diff context (3 lines), bump to 10–20 lines so doc comments above changed functions and visibility modifiers above changed `const`s land in the context. This is a softer fix that doesn't require tool calls but increases prompt size.

### Fix 3: Lint pre-pass for syntactic-class rules

For rules that are testable by static checks alone (e.g. \"every `pub` item has a `///` immediately above it\"), run a `cargo doc --no-deps` / `rustdoc` / regex pre-pass over public items in the diff. If the static check confirms compliance, the rule is suppressed entirely. The model is only invoked for the genuinely-judgment-based parts.

`missing-docs` for Rust is fully syntactic and shouldn't even need a model. `no-magic-numbers` is half-syntactic (the existence of a named binding is checkable; the *meaning* of the binding isn't).

## Acceptance

- The fixture file as it stands today (every public item has a doc comment, every magic value is a named const, every const is private) passes both `missing-docs` and `no-magic-numbers` for two consecutive Stop runs without any further edits.
- Add a regression test in `avp-common/tests/` that constructs a small fixture file with the three compliance properties (doc comments on pub items, named constants for magic values, private const visibility) and a recording of the validator agent's tool-call sequence. Assert: (a) `read_file` was called for every changed file before any `passed=false` was issued, (b) the verdict for the well-formed fixture was `passed=true`.

## Pairs with

- `01KQAFCT6B4EP1ENW5RHFVFZB2` (more MCP server tracing) — once the recording is durable and tracing is verbose, we can inspect a hallucinating run and see exactly what tools the validator did or didn't call.
- `01KQAFT5H1CYQ8YDNAM4J0HD1Q` (RecordingAgent flush) — same.
- `01KQB0PQV06JZREBKX9Q5EBSHY` (per-rule timeout tuning) — separate axis but same goal: make Stop-hook results trustworthy.

#avp #validator-quality #hallucination