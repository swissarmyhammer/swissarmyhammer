---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
project: local-review
title: check-sah skill still documents deleted .validators/.hashes store
---
Follow-up surfaced while implementing `6kjk1qs` (delete dead .hashes incremental tracking, commit d5763f398).

`builtin/skills/check-sah/SKILL.md` still documents a `$HASHES=.validators/.hashes` store layout it reads when monitoring other repos' finish runs — but that store was deleted with the tracking subsystem. The doc is now stale and would tell a monitoring agent to inspect a path that no longer exists.

## Work
- Grep `builtin/skills/check-sah/SKILL.md` for `.hashes` / `.validators/.hashes` / `HASHES` / skip-hash / incremental-tracking references.
- Remove or rewrite those sections: there is no per-file hash store anymore; review is `sha HEAD~1..HEAD`-scoped per finish checkpoint with no skip-hash caching. Update any token-saving / re-review-storm guidance that assumed the hash store existed.
- Edit `builtin/skills/check-sah/SKILL.md` (source of truth), NOT generated `.skills/`.

Docs-only; verify with a grep that no stale `.hashes` reference remains in the skill.