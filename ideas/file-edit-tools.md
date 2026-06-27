# Best-in-Class Operation-Based File Edit Tools

## Starting point

`files` is operation-based, registered in `FILE_OPERATIONS`: `read` (`ReadFile` —
`path`/`offset`/`limit`), `write` (`WriteFile`), `edit` (`EditFile` —
`file_path`/`old_string`/`new_string`/`replace_all`), `glob`, `grep`. Infrastructure
is right: encoding detection/preservation, line-ending detection (the CRLF/LF/CR
counting in `EditValidation`), atomic temp-and-rename via `edit_file_atomic`, and
`shared_utils` (`validate_file_path`, `FilePathValidator` workspace boundary,
`check_file_permissions`).

Three weak spots: `edit` matches exact substring only (`content.matches(old_string)`),
hard-erroring "String not found" on a miss; multi-edit exists only in `edit`'s
executor (`EditRequest.edits: Vec<EditOperation>`) and is **not** declared in
`ParamMeta`, so schema and grammar don't see it; and standalone
`ReadFileTool`/`GlobFilesTool`/`GrepFilesTool` duplicate the `files` ops. Keep the
infrastructure, change the operations below.

## Edit format: landscape and decision

The decisive finding from surveying other agents: edit-format reliability is coupled
to whether the *model was trained on the format*, and the biggest gains land on the
weakest models — which is exactly our target.

| Format | Notes |
|---|---|
| **Hashline** (oh-my-pi, Can Bölük, Submersible) | Read tags each line `N:HH` (line number + 2-char hash of normalized content). Edits reference anchors, never reproduce text. Hash mismatch → reject (free per-line staleness). Benchmarks: matches/beats str_replace for most models, **weakest models gain up to 10x**. |
| **Search/replace** (str_replace — Claude Code, ours) | Model-agnostic, no training. Weakness is exact reproduction; the ladder mitigates it but can mis-apply a fuzzy match (fails wrong, not safe). |
| **Unified diff** (Aider udiff) | Efficient, but ~70–80% accuracy on complex files; line numbers break across multi-turn edits. |
| **apply_patch / V4A** (Codex) | Only reliable on models trained on it; ~50%+ failure otherwise. |
| **Whole file** | Always applies; token-expensive, elision risk. New/small files only. |
| **Fast-apply** (Morph/Relace, Cursor) | Lazy edit + a dedicated apply-model. Two models, two failure points; "dead for frontier models" but not for weak ones. |

The weak-model evidence points straight at us, and hashline's failure mode is the
right one: a wrong anchor *rejects* rather than mis-applying, whereas a confident-but-
wrong fuzzy match applies silently. One credible critique also argues fuzzy matching
targets a phantom (models reproduce exactly or hallucinate wholesale, never near-miss)
and that an **LSP feedback loop confounds edit-format results** — which matters because
we're building that loop. So hashline and the diagnostics loop are partial
substitutes; format matters less when diagnostics are strong. That argues for not
over-investing in format, not against hashline (cheap, helps weak models regardless).

Decision — one forgiving `edit files`, no modes, no flags. The model supplies `find`
and `replace`; the tool infers the style from the *shape* of `find` and falls back
gracefully, so the model is never punished for its choice:

- **Hashline anchor** (`N:HH`, optionally `|text`) that resolves → line-addressed,
  hash-verified edit.
- **Bare string** → literal search/replace: exact, then normalized, then fuzzy.

No regex: it can't be told apart from a literal by shape, and supporting it would mean
the explicit flag we're avoiding. Two styles, both shape-inferred. `read files` emits
hashline tags, so anchors are the path of least resistance and the model gravitates to
them, but bare strings work too. Whole-file `write` stays for new/large files.
apply_patch/V4A rejected (parse-only); unified diff and fast-apply skipped/parked.

Safety rule that makes inference non-dangerous: a structured interpretation only
*wins* when it resolves — `42:a3` is an anchor only if line 42 exists and hashes to
`a3`, else it's literal text. When both a resolving anchor and a literal match exist,
surface both and let the model pick (do-more-per-call); never guess.

## Operations: current → target

Per op, what changes:

**`read files` — emit hashline tags.** Add a `format` param (`hashline` default,
`plain` available). In hashline form prefix each line `N:HH|` — N the absolute 1-based
line number, HH the 2-char hash of the whitespace-normalized line (see core below).
Keep `offset`/`limit`; anchors use absolute N so they stay stable across windows.
Leave the binary/base64 path untagged.

**`edit files` — one forgiving op.** Params: `file_path`, `find`, `replace`,
`replace_all?`. Both `find` and `replace` accept **aliases** (declared in
`ParamMeta.aliases`) and may be a **scalar or an array**, and an `edits?: Array` of
`{find, replace}` is also accepted — three equivalent input shapes, all normalized to
the same canonical pair list.

*Aliases* — `find` ← {`find`, `search`, `old`, `old_string`, `from`, `target`,
`match`}; `replace` ← {`replace`, `new`, `new_string`, `to`, `with`, `replacement`}.
The grammar emits canonical `find`/`replace`; the parser accepts any alias, so a model
off the grammar (or a different host) still lands.

*Argument normalization (really forgiving)* — collect find-ish and replace-ish values
from wherever they appear (top-level scalar, top-level arrays, the `edits` array, under
any alias) and pair them into `Vec<(find, replace)>`:

- N finds + N replaces → zip.
- N finds + 1 replace → broadcast the replace to all (e.g. delete many: finds + one
  empty replace).
- top-level `find`/`replace` *and* `edits[]` → concatenate.
- mismatched array lengths or 1-find-N-replaces → apply what pairs cleanly, surface the
  remainder; never silently drop.

Then each pair runs the cascade on its `find`:

1. If `find` parses as a hashline anchor (`N:HH` or `N:HH|text`) **and resolves** (line
   exists, hash matches; the `|text` serves as verification/fallback) → replace that
   line.
2. Else literal: exact → normalized → fuzzy (the ladder below), span replace.

Replace semantics follow what `find` resolved to: an anchor find replaces the line, a
span find replaces the span. A resolving anchor *and* a literal match both present →
surface both, don't guess. The whole batch is atomic. This supersedes the earlier
edit/replace split — one op, no separate `replace files`; `edit_file_atomic` and the
old executor `edits` array fold in here.

Delete is an empty `replace`. Insert needs no special op: replace a line with itself
plus the new content (e.g. find the closing-brace line, replace it with
`new line\n}`). No anchor-operation vocabulary, no `at` modifier — it's all find and
replace.

### Examples

Given `read files` output:

```
1:4f|fn calculate_total(items: &[Item]) -> f64 {
2:a3|    items.iter().map(|i| i.price).sum()
3:0e|}
```

- **Bare string, exact** — `find: "i.price"`, `replace: "i.price * i.qty"` → literal
  match on line 2, span replaced.
- **Bare string, forgiving** — `find: "items.iter().map(|i| i.price).sum()"` (model
  dropped the leading indentation) → exact misses, normalized matches line 2, applied
  to the original span so indentation is preserved.
- **Hashline anchor** — `find: "2:a3"`,
  `replace: "    items.iter().map(|i| i.price * i.qty).sum()"` → line 2 hashes to a3,
  whole line replaced.
- **Anchor + text** — `find: "2:a3|    items.iter().map(|i| i.price).sum()"` → anchor
  resolves and the text confirms; if line 2 drifted, the text relocates it, else reject.
- **Aliases** — `{old: "i.price", new: "i.price * i.qty"}` → same as the first example.
- **Parallel arrays** — `find: ["1:4f", "3:0e"]`, `replace: ["...", ""]` → two edits
  zipped (replace line 1, delete line 3), atomic.
- **Broadcast delete** — `find: ["2:a3", "3:0e"]`, `replace: ""` → one empty replace
  applied to both.
- **Object array** — `edits: [{search: "2:a3", with: "…"}, {find: "fn calculate_total",
  replace: "fn calc_total"}]` → mixed styles and aliases, each resolved independently.

**`write files` — read-before-write guard.** For an existing file require a freshness
token (a whole-file hash from a prior `read`, or session read-tracking); on divergence
return current content rather than clobbering. New/nonexistent files unguarded. Mirrors
the hashline read-before-edit mandate and composes with the closed-write-surface goal.

**All mutating ops — shared result contract.** Extend `EditResult` (and write
results) with `tagged_content` (re-tagged view of the changed file, so the model chains
edits without re-reading) and `mutated_paths` (drives inline diagnostics via the shared
core). Keep `bytes_written` / `replacements_made` / `encoding_detected` /
`line_endings_preserved`.

**Consolidation.** Delete `read_file.rs` / `glob_files.rs` / `grep_files.rs` and their
registrations; the `files` ops subsume them. Drops the duplicates and a grammar
special-case.

## Hashline core (new pure module/crate)

`swissarmyhammer-hashline` (or a module in the edit crate), no IO:

- `hash_line(&str) -> u8` rendered as 2 hex chars: `crc32fast` over the line with
  leading/trailing spaces and tabs stripped, mod 256. 256 values = staleness detection,
  not uniqueness (~99.6% chance of catching a single-line change; the line number
  disambiguates collisions). Normalization is strip-horizontal-whitespace only — keep
  interior — so formatters don't break anchors.
- `tag(content, start_line) -> String` — annotate `N:HH|line`.
- `parse_anchor("N:HH") -> (usize, u8)`.
- `apply(content, ops) -> Result<Applied>` — resolve each anchor (exact line N, else
  proximity-search nearby lines for one hashing to HH), reject on mismatch with current
  re-tagged content; preserve original line endings and encoding (reuse the
  `edit/mod.rs` detection, or lift it to `shared_utils`).
- Pure; property-test: tag→edit→re-tag round-trips, mismatch rejects, proximity finds
  drifted anchors, reformatting preserves anchors.

## The literal-find ladder (cascade step 3)

When `find` is a bare string, it's a *description* of a span, not a byte-exact copy.
Try progressively forgiving matchers; stop at the first unique, confident match.

1. **Exact** — literal match (current behavior).
2. **Normalized** — match on whitespace-normalized forms (trim trailing, normalize
   line endings, optionally collapse indentation); apply to the *original* span.
   Catches the dominant drift.
3. **Anchor** — match unique first/last lines, replace the span between; tolerant of
   interior drift.
4. **Fuzzy** — similarity-scored; accept only if one candidate clears the threshold
   *and* clearly beats the runner-up. Never applied silently.

Each rung returns span, confidence, and which rung matched. The ladder is pure
(`(content, old_string) → match`, no IO) — property-test it: perturb
whitespace/indent/line-endings and assert it lands; assert ambiguity is refused. Its
own module or crate, tested apart from file IO.

## Ambiguity → candidates, not error

Multiple confident matches with `replace_all` false: return the candidates (line
numbers, current text, surrounding context) so the model disambiguates in one
follow-up. Optional `occurrence` / line-hint param to point precisely.

## Failure → structured near-miss, not "not found"

No confident match: return the closest span(s) — current text, line numbers, context,
and a diff against what the model supplied — so it sees exactly how `old_string`
diverged and corrects in one shot.

## Multi-edit

Whatever input shape produced them — parallel arrays, `edits` objects, or a single
pair — the normalized `(find, replace)` list is applied atomically: validate all
before committing; any failure leaves the file untouched (temp write makes this clean).
Return per-edit results with the structured near-miss for any failure. Detect
specifically when a later edit's target was consumed by an earlier one.

## Staleness

An anchor `find` gets this for free and per-line: a stale anchor is a hash mismatch
and rejects with current content shown, so the model can only edit what it recently
read. A literal `find` approximates it at whole-file granularity — `read` returns a
content hash, `edit` accepts an optional expected hash, divergence is surfaced rather
than clobbered. Free when a hash was carried, inert otherwise.

## Idempotency

Reject no-ops (`old == new`). If `new_string` is present and `old_string` absent, the
edit was likely already applied — say so rather than erroring.

## Diagnostics in the result

After a successful mutating edit, the op declares `mutated_paths` and folds in
diagnostics via the shared core (edited file + broken dependents, generous settle,
sharp output — see the diagnostics doc). The edit op is the primary trigger and
delivery surface for diagnostics.

## Crate structure

`swissarmyhammer-diagnostics` (new core) sits *on top of* the shared LSP client — it
owns no client and spawns no server. The single `async-lsp` session lives in
`swissarmyhammer-lsp` (promoted to the one LSP system: supervision + session + shared
open-document set + publishDiagnostics fan-out), and code-context migrates onto it.
The core adds settle/debounce, the `Diagnostic` / `DiagnosticsReport` types, and
config; API `diagnose(paths, opts) -> DiagnosticsReport` plus watch/subscribe. It does
not select paths and knows nothing of MCP, operations, or editing — callers pick
paths, the core reports what's wrong. (The LSP consolidation is in the diagnostics
doc.)

The matching engine is a pure module/crate (`swissarmyhammer-edit-match`), no IO. The
`files` op wraps it with the encoding/atomic-write/dispatch it already owns.

Path selection stays in the tool layer: the `diagnostics` tool resolves
`working`/`sha` via git and dependents via code-context; `files edit` asks the core
for its mutated path plus dependents.

Dependencies, no cycles:
- `files` op → matching engine + `swissarmyhammer-diagnostics`
- `diagnostics` op → `swissarmyhammer-diagnostics` + git + code-context
- `swissarmyhammer-diagnostics` → `swissarmyhammer-lsp` (shared client)
- `swissarmyhammer-code-context` → `swissarmyhammer-lsp` (same client)

`swissarmyhammer-lsp` is the single LSP foundation; code-context and diagnostics are
siblings. code-context is a tool-layer dependency only.

## What "best" means here

Exact-match tools make a near-miss a hard failure; the ladder + candidates +
structured failure + staleness + inline diagnostics *remove* the retry loop instead.
On a slow local model that's the only metric that matters: never send the model back
for a turn the tool could resolve itself.

## Testing

- **Ladder**: pure property tests — perturbation lands, ambiguity refused, fuzzy
  honors its threshold.
- **Multi-edit**: a failed edit leaves the file byte-identical; per-edit results
  correct.
- **Encoding/line-endings**: matched-on-normalized, applied-to-original preserves
  bytes outside the span.
- **Staleness**: stale hash surfaces divergence, no clobber.
- **Diagnostics**: golden test through the core on a fixture crate.
