---
assignees:
- claude-code
depends_on:
- 01KQSEFZ8VQ67KFA0B4QE84Z2X
position_column: todo
position_ordinal: d680
project: spatial-nav
title: 'Rewrite swissarmyhammer-focus README: short, single-primitive, no FocusZone'
---
## What

Rewrite `swissarmyhammer-focus/README.md` to describe **only the algorithms and rules**. The current ~600-line README is bloated with cross-references, test file pointers, audit history, anti-pattern callouts, deprecation prose, and structural-vs-geometric history. All of that goes.

Two simultaneous wrongs to fix:

1. The README still mentions `FocusZone`. The split is being collapsed (parent `01KQSDP4ZJY5ERAJ68TFPVFRRE`). One primitive: `FocusScope`, may have children.
2. The README is too long. Length is the smell — if the kernel can't be described in a short README, the kernel is over-designed.

The new README is a **rules document**. Each section states the rule, not where it's tested, not who audited it, not why a previous version was wrong.

### Target structure

```
# swissarmyhammer-focus

One paragraph: headless spatial-nav kernel. Knows about FocusScopes
with rects in layers. Doesn't know about DOM, scroll, virtualization.

## Primitive
FocusScope. May have children. Has a rect. Layer-bound. Carries an
FQM (the path through the focus hierarchy) as identity.

## Cardinal nav
Geometric beam pick. For direction D, candidates are all scopes in the
focused scope's layer that:
  1. lie strictly in the half-plane of D from the focused rect,
  2. overlap the focused rect on the cross axis,
  3. are not the focused scope itself.
Pick the candidate minimising the Android beam score `13*major² + minor²`.
Tie: leaves win over scopes-with-children.
Empty candidate set → return focused FQM (stay-put).

## Drill in
Focus a child of the focused scope.
  1. If focused scope has a live `last_focused`, return it.
  2. Else return the topmost-then-leftmost child.
  3. Else (no children) return focused FQM.

## Drill out
Return the focused scope's parent FQM. No parent → return focused FQM.

## First / Last
Focus the topmost-leftmost / bottommost-rightmost child of the focused
scope. Leaf focused → return focused FQM.

## Overrides (rule 0)
Each scope carries a per-direction override map. Runs before any other
rule. `Some(fq)` teleports. `None` is a wall (stay-put).

## No-silent-dropout
Every nav/drill op returns a FullyQualifiedMoniker. Never Option, never
Result. "No motion possible" is signalled by echoing the focused FQM.

## Coordinate system
All registered rects are viewport-relative
(`getBoundingClientRect()`), refreshed on ancestor scroll. The kernel
ranks by raw rect; mixing coordinate systems silently picks wrong
neighbors.

## Scrolling
Not in the kernel. The React layer scrolls on stay-put when the
focused scope's nearest scrollable ancestor in D can scroll further.
```

That's the whole document.

## Acceptance Criteria

- [ ] `wc -l swissarmyhammer-focus/README.md` < 120 lines
- [ ] `grep -c "FocusZone\|is_zone" swissarmyhammer-focus/README.md` == 0
- [ ] No section names tests, audits, commit history, or other crates
- [ ] No "why we changed this" / "anti-pattern callout" / "deprecated alias" prose
- [ ] No ASCII tree diagrams beyond what's needed to state a rule
- [ ] Every section is the rule, period

## Tests

Prose-only change. Verification is the line count + grep above.

- [ ] `wc -l swissarmyhammer-focus/README.md` < 120
- [ ] `grep -c "FocusZone\|is_zone" swissarmyhammer-focus/README.md` == 0

## Dependencies

Runs after the FocusZone collapse sub-tasks (`01KQSEFZ8VQ67KFA0B4QE84Z2X` is the last) so the kernel matches the prose.

#spatial-nav-redesign