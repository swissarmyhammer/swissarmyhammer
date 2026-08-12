---
assignees:
- claude-code
position_column: todo
position_ordinal: ffda80
title: Prune a retired RULE file from a deployed validator store
---
`mirdan::retired_validators::prune_unmodified_retired_sets` prunes a whole
retired SET directory from a deployed store (`~/.validators/` or
`./.validators/`). It has no facility for a retired RULE FILE inside a set that
still ships.

So a rule deleted from `builtin/validators/<set>/rules/` survives in every store
an earlier `sah init` wrote. `install_profile_validators` overwrites each
embedded active file and adds nothing, so nothing removes the leftover. The
loader then reads the stale rule at user or project precedence and the rule
keeps running.

Measured on this machine after ^wwb6hk7 deleted `duplication-parsed` and
`no-commented-code-parsed`:

- `sah doctor` with the real `$HOME`: two degraded rows, `code-hygiene/no-commented-code-parsed`
  and `duplication/duplication-parsed`, each reading
  `tool missing: bash: : command not found`, because `~/.validators/` still holds
  the two rule files and `SAH_BIN` no longer reaches the scripts.
- `sah doctor` with `$HOME` pointed at an empty directory: neither rule appears,
  and every remaining tool rule reports `tool present; fixtures pass`.

## What to build

Extend the retired snapshot to the FILE level, with the same honesty contract
the set-level prune keeps: an exact byte-for-byte match against the shipped
snapshot removes the file; any difference leaves it alone. Add the two rule
files above as the first entries.

## Done when

- A store holding a stale `duplication/rules/duplication-parsed.md` and
  `code-hygiene/rules/no-commented-code-parsed.md` loses both after `sah init`,
  and a store whose copy of either was edited keeps it.
- `sah doctor` on a machine whose store was written before ^wwb6hk7 reports no
  tool rule for duplication or commented code.

#tool-validators