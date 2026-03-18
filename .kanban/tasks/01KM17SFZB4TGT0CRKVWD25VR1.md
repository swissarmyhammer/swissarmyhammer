---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffd980
title: '[WARNING] PLANNING_GUIDE.md partial replaces a slightly different "What" description line'
---
## What

The two original inline copies of the card-standards block were not byte-for-byte identical. The version in `PLANNING_GUIDE.md` had a more specific "What" placeholder:

```
<what to implement — full paths of files to create or modify, approach, context>
```

The version now in `builtin/_partials/card-standards.md` uses the shorter form from `SKILL.md`:

```
<what to implement — affected files, approach, context>
```

The phrase "full paths of files to create or modify" gave more concrete guidance to autonomous agents (which `PLANNING_GUIDE.md` is written for). That specificity has been silently dropped.

Files:
- `builtin/_partials/card-standards.md` (line 13)
- Previously in `builtin/skills/plan/PLANNING_GUIDE.md`

## Acceptance Criteria
- [ ] Either: the partial's "What" placeholder is updated to the more specific wording ("full paths of files to create or modify"), OR the PLANNING_GUIDE acknowledges the difference and adds a local annotation after the include.
- [ ] The change is intentional and documented (not silently lost).

## Tests
- [ ] Rendered output of `skill { op: "use skill", name: "plan" }` in agent/PLANNING_GUIDE context contains the accepted wording. #review-finding #warning