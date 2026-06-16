Operation-based local multi-agent code review and validator introspection.

A single op-dispatched tool. The `review` verb runs the review pipeline over a
scope; the scope target is the noun:

- `review file` — review an explicit file path or glob.
- `review working` — review uncommitted changes vs HEAD (the everyday op).
- `review sha` — review the changes in/since a commit or range.

Each returns a `ReviewReport { markdown, counts }` and accepts the shared
`validators?[]` and `backend?` (`session` | `local`) modifiers.

`review working` is **incremental** once it has run: each reviewed file's
content+rules hash is recorded under `.validators/.hashes/`, and a later pass
subtracts files unchanged since their last review (a no-op pass short-circuits to
a clean report). Pass `force` (alias `all`) to ignore tracking and review the
whole change set; clearing `.validators/.hashes/` has the same effect. A rule
edit invalidates every entry and re-sweeps the full set.

The loader-read ops introspect what is plugged in (no agent, fast):

- `list validators` — one summary row per loaded RuleSet, filterable by `source`
  (`builtin` | `user` | `project` | `all`) and a path/glob `match`.
- `get validator` — one validator's frontmatter, probes, and full rule bodies.
- `check validators` — lint every loaded validator: globs compile, no stray
  trigger, declared probes exist in the catalog.
