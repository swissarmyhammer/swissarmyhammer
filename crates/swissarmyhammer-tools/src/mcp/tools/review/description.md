Operation-based local multi-agent code review and validator introspection.

A single op-dispatched tool. The `review` verb runs the review pipeline over a
scope; the scope target is the noun:

- `review file` — review an explicit file path or glob, given as `path`:
  `{"op": "review file", "path": "src/auth.rs"}`.
- `review working` — review uncommitted changes vs HEAD (the everyday op):
  `{"op": "review working"}`.
- `review sha` — review the changes in/since a commit or range, given as `sha`:
  `{"op": "review sha", "sha": "HEAD~1..HEAD"}`.

Each returns a `ReviewReport { markdown, counts }` and accepts the shared
`validators?[]` (subset of validator names to run), `backend?`
(`session` | `local`), and `batch_size?` (max inlined file bytes per review
batch, default 262144) modifiers.

## Streaming (when a `progressToken` is supplied)

A `review file/working/sha` call that carries `_meta.progressToken` streams the
run as it happens over two MCP channels, so a client can start resolving
findings in parallel instead of waiting for the final `ReviewReport`:

- `notifications/progress` — pair-count ticks (`progress`/`total`/`message`) as
  each `(validator, file)` pair is reviewed; advisory, for progress bars.
- `notifications/message` — the review's ACTUAL content as it resolves, carried
  as structured `data` under logger `"review"`, level `info`:
  - `{"kind": "review.findings", "validator": "<name>", "findings": [<Finding>…]}`
    — emitted when a validator task completes, with every finding it parsed (an
    empty array means that validator came back clean). The `Finding` objects are
    complete and never truncated.
  - `{"kind": "review.verdict", "finding": <Finding>, "confirmed": <bool>,
    "reason": "<why>"}` — emitted as each candidate's verdict resolves (guard
    refutation or adversarial-agent verdict).

The streamed events are **per-validator granular**: the same finding can be
emitted by more than one validator. The final `ReviewReport` exact-dedups those
across validators by `file:line`, so the report is the deduped subset of the
streamed confirmed verdicts — a dedup, never a retraction. Content rides the MCP
peer transport only; the in-process progress sink carries progress params, so a
caller wired to that sink (or with no transport) receives no content.

Every `review` op resolves its scope through an ignore layer so non-source
artifacts never enter review. On the first run in a repo a `.reviewignore` file
is auto-generated at the repo root (gitignore syntax, defaulting to `.kanban/`);
it is never overwritten, so your edits are authoritative. The repo's own
`.gitignore` is honored on top of it — a gitignored file is never reviewed, even
when tracked. This applies uniformly to `review file`, `review working`, and
`review sha`: a path matched by either file is dropped from the reviewed set (a
`review file` naming an ignored path resolves to an empty review, not an error).

The loader-read ops introspect what is plugged in (no agent, fast):

- `list validators` — one summary row per loaded RuleSet, filterable by `source`
  (`builtin` | `user` | `project` | `all`) and a path/glob `match`.
- `get validator` — one validator's frontmatter, probes, and full rule bodies,
  by `name`.
- `check validators` — lint every loaded validator: globs compile, no stray
  trigger, declared probes exist in the catalog.
