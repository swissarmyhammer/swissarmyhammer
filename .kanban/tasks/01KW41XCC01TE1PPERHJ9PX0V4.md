---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw4n2x56v1zmndymy10k1zep
  text: |-
    Docs-only edit to builtin/skills/check-sah/SKILL.md (source of truth; did NOT touch generated .skills/). Removed/rewrote all stale references to the deleted .validators/.hashes store:

    1. Setup bash block: removed the `HASHES="$REPO/.validators/.hashes"` variable + "review-engine hash store" comment.
    2. Layout section: replaced the `$HASHES/<dir>/<file>.yaml` skip-hash bullet with accurate guidance — each /finish review is scoped to the checkpoint commit delta (sha HEAD~1..HEAD), content-batched by batch_size (default 32 KB), and there is NO per-file hash store / skip-hash cache to inspect (the .validators/.hashes/ incremental-tracking subsystem was removed).
    3. Calibration: dropped the "broken-hash run ~= 15M/task" framing; now "flag a run drifting toward 15M+/task as a regression to investigate".
    4. Standing token-saving rec (1): rewrote "don't full-sweep on re-review, share a cached file prefix, stop force:true" (all assumed the hash cache) to: cut fan-out + tune batch_size; review is already commit-delta-scoped so there is no hash cache to skip unchanged files — levers are fan-out width and batch packing, not re-review suppression.
    5. Shell gotchas: "Transcript/hash timestamps are UTC" -> "Transcript timestamps are UTC".

    Verify grep `rg -n -i 'hashes|skip-hash|hash store' builtin/skills/check-sah/SKILL.md`: only one remaining match — line 38, which accurately states the store was removed and there's nothing on disk to inspect (the task explicitly permits an accurate "no longer exists" mention). No stale references remain. Task left in doing.
  timestamp: 2026-06-27T13:42:41.062110+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffea80
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