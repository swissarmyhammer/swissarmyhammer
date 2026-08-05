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

## Streaming

A `review file/working/sha` call streams the run as it happens over two MCP
channels, so a client can start resolving findings in parallel instead of
waiting for the final `ReviewReport`:

- `notifications/progress` — pair-count ticks (`progress`/`total`/`message`) as
  each `(validator, file)` pair is reviewed; advisory, for progress bars. Only
  emitted when the call carries `_meta.progressToken` (the MCP spec requires
  echoing a client-supplied token).
- `notifications/message` — the review's ACTUAL content as it resolves, carried
  as structured `data` under logger `"review"`, level `info`. Not token-gated:
  it flows to the calling peer with or without a `progressToken`.
  - `{"kind": "review.findings", "validator": "<name>", "findings": [<Finding>…]}`
    — emitted when a validator task completes, with every finding it parsed (an
    empty array means that validator came back clean). The `Finding` objects are
    complete and never truncated.
  - `{"kind": "review.verdict", "finding": <Finding>, "confirmed": <bool>,
    "reason": "<why>"}` — emitted as each candidate's verdict resolves (guard
    refutation or adversarial-agent verdict).
  - `{"kind": "review.keep-alive", "message": "review in progress"}` — a
    periodic re-assurance during engine silence on calls with no
    `progressToken` (tokenful calls keep alive by re-sending the latest
    progress param instead); safe to ignore when accumulating findings.

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
  (`builtin` | `user` | `project` | `all`) and a path/glob `match`. Set
  `rules: true` and each row also carries that validator's rules — every rule's
  `name` plus its verbatim `body`. Use it for summaries and introspection.
- `dump validators` — write every rule the given `paths` match to ONE markdown
  file in the system temp directory and return its path, plus a JSON summary:
  the deduplicated `validators`, the total `rule_count`, a `matched` map from
  each path to its validator names, and the distinct `extensions`.
- `get validator` — one validator's frontmatter, probes, and full rule bodies,
  by `name`.
- `check validators` — lint every loaded validator: globs compile, no stray
  trigger, declared probes exist in the catalog.

Read the rules BEFORE you edit: rules match by file pattern, so one example
file per distinct extension gives the full rule set. One call,
`{"op": "dump validators", "paths": ["src/auth.rs", "web/app.ts"]}`, writes
every applicable rule body verbatim to one file — read that file whole, one
time. Each path runs through the engine's own file matcher, so the file holds
exactly the rules a `review` run enforces on those paths — no per-file calls,
no per-name `get validator` calls, no guessing.
