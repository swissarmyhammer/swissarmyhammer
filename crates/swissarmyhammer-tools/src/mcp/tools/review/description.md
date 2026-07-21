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
