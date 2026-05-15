---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffe680
project: spatial-nav
title: 'Inspector entity zone: wrap each open inspector body in `<FocusZone moniker={entity}>` so cardinal nav stays within an entity'
---
## What

User report (2026-04-30):
> "the entire entity for the inspector needs a FocusZone, i noticed this with two inspectors open that nav across entities was not zoned"

Currently, `01KQCTJY1QZ710A05SE975GHNR` (Inspector layer simplification) deleted the per-panel `<FocusZone moniker="panel:type:id">` wrap and made every field zone register directly at the inspector layer root with `parentZone === null`. That gave cross-panel nav for free, but it also collapses the entity boundary — with two inspectors open, every field zone across both panels is a sibling at iter 0 of the kernel's beam-search cascade. ArrowDown from a field in inspector A can land on a field in inspector B because they're peers at the same level.

Re-introduce a per-entity `<FocusZone>`, keyed by the entity moniker itself (e.g. `task:01KQ2E...`, `tag:bug`, `project:spatial-nav`) — NOT a `panel:` prefix. The entity moniker is the natural identity; the panel is just chrome. The zone barrier puts ArrowUp/Down/Left/Right in iter 0 within an entity (peers = field zones of that entity), and cross-entity nav escalates to iter 1 (entity-zone peers) which finds the other inspector's fields by rect.

This is **NOT** a regression of `01KQCTJY1QZ710A05SE975GHNR`. That card deleted three things:

1. `panel:type:id` zone — replaced by an entity-keyed zone (different segment shape, different identity).
2. `<InspectorFocusBridge>` component — stays deleted. The new wrap is direct `<FocusZone>` in `inspectors-container.tsx` or `entity-inspector.tsx`.
3. `inspector.edit / editEnter / exitEdit` commands — stay deleted. `field.edit` / `field.editEnter` at the field zone cover the semantics.

What changes from the simplification: the architectural assumption that "field zones at layer root with no in-between zone is fine" was wrong for the multi-inspector case. The fix re-adds a zone *only* — no commands, no bridge component, no panel-prefixed identity.

## Files to touch

- **`kanban-app/ui/src/components/inspectors-container.tsx`** OR **`kanban-app/ui/src/components/entity-inspector.tsx`** — wrap the inspector body in `<FocusZone moniker={asSegment(\`${entityType}:${entityId}\`)}>`. Pick the boundary that minimizes context dependencies. `inspectors-container.tsx`'s `InspectorPanel` already has the `entry` (entityType + entityId) and is the natural panel-presentation boundary. Wrap there, around `<EntityInspector entity={resolved} />`. `EntityInspector` itself is a generic inspector body — keep it agnostic of overlay-vs-not.

- **`kanban-app/ui/src/components/inspector.entity-zone-barrier.browser.test.tsx`** (new) — TDD for the new contract.

- **`kanban-app/ui/src/components/inspector.layer-shape.browser.test.tsx`** — update assertions: was "field zones register with `parentZone === null` at layer root"; now is "field zones register with `parentZone === <entity FQM>`; per-entity zone registers at layer root with `parentZone === null`".

- **`kanban-app/ui/src/components/inspector.cross-panel-nav.browser.test.tsx`** — verify ArrowLeft/Right between adjacent panels still works via iter 1 escalation through the entity zones. Probably no production-test change needed (the assertions are about target moniker, not about path through the cascade), but re-run to confirm.

- **`kanban-app/ui/src/components/inspectors-container.guards.node.test.ts`** — currently asserts "no `FocusZone` import in `inspectors-container.tsx`". Loosen to allow the inspector-container `FocusZone` while still forbidding `InspectorFocusBridge` and `panel:*` monikers (the deleted shapes from `01KQCTJY1QZ710A05SE975GHNR`).

## Approach

1. **Failing tests first.** Write `inspector.entity-zone-barrier.browser.test.tsx` with three named tests:

   - **`ArrowDown at the last field of inspector A stays in A — does NOT enter inspector B`**: open two inspectors A (`task:TA`) and B (`task:TB`) side by side, focus A's last field, fire ArrowDown. Expect the focused FQM to remain on A's last field (echoed via the no-silent-dropout contract); MUST NOT match `/.../task:TB/...`.

   - **`ArrowUp at the first field of inspector A stays in A`** — symmetric stay-put guard.

   - **`Cross-entity ArrowLeft from B's leftmost field lands on a field in A`** — the iter-1 escalation case. The current cross-panel test already covers this; copy the assertion into the new file with a comment that the path now goes through the entity-zone peer set.

   Also write a kernel-state shape assertion (one test):

   - **`each open inspector registers a zone keyed by entity moniker; field zones list the entity-zone FQM as parentZone`**: open A and B, walk the kernel-simulator's `registrations` map. Two entries with `kind: "zone"` and segments `task:TA` and `task:TB`. Every field-zone entry's `parentZone` must equal the corresponding entity-zone FQM, NOT `null`.

2. **Production change.** Wrap `<InspectorPanel>`'s body in the entity-keyed `<FocusZone>`. The wrap goes inside `<SlidePanel>` (so the slide animation is unchanged) but outside `<EntityInspector>` (so `EntityInspector` stays inspector-agnostic). The zone needs no commands prop (default `EMPTY_COMMANDS`) and no `navOverride` — pure structural barrier.

3. **Update layer-shape test.** Flip expectations from "fields at layer root, parentZone=null" to "fields under entity zone". Keep the no-`InspectorFocusBridge` and no-`panel:*` guards.

4. **Sanity-run cross-panel test.** With the iter-1 cascade now going zone → zone instead of zone → layer-root, the ArrowLeft/Right boundary case should still work (the kernel's beam search escalates to the entity zone's peers and finds the adjacent entity zone's fields by rect). Verify via the existing four cross-panel tests.

5. **Guards.** Update the source-level guard test to allow `FocusZone` in `inspectors-container.tsx` while still forbidding `InspectorFocusBridge` and any `panel:*` moniker literal.

## Out of scope

- This task does NOT bring back `<InspectorFocusBridge>` or `inspector.edit/editEnter/exitEdit` commands — those stay deleted per `01KQCTJY1QZ710A05SE975GHNR`.
- This task does NOT introduce a `panel:` moniker shape — the segment is the entity moniker (e.g. `task:abc`), not a panel-prefixed wrapper.
- Does NOT touch the kernel — the kernel's beam-search cascade and drill-out fallback already work; this is a React-side structural fix.
- Layer 3 manual log verification (`tauri dev`) is folded in here as the last acceptance check, since the user observed the bug manually and the fix needs to be confirmed manually too.

## Cross-references

- Memory: `feedback_path_monikers.md` — path-as-key invariant; entity-zone FQM `/window/inspector/task:abc` is distinct from board-side `/window/.../column:done/task:abc` by path even when segments collide.
- Predecessor: `01KQCTJY1QZ710A05SE975GHNR` — Inspector layer simplification (deleted panel zone). This task partially walks that back, deliberately, with a different identity.
- Adjacent: `01KQF7BD866K5JNSKMC7SESMJ9` — the duplicate-FQM-warning investigation. If two inspectors register zones with the same entity moniker (e.g. user opens task:abc twice in the same window), this task creates a path collision. Verify `inspector_stack` dedupes on the backend; if it doesn't, that's a separate bug to file (cross-link from there).
- No-silent-dropout: `01KQAW97R9XTCNR1PJAWYSKBC7` — the stay-put echo at zone boundary depends on this contract; do not regress.
- FQM refactor: `01KQD8XM2T0FWHXANCK0KVDJH1`.

## Acceptance Criteria

- [x] Each open inspector panel wraps its body in `<FocusZone moniker={asSegment(\`${entityType}:${entityId}\`)}>`. The zone segment is the entity moniker — no `panel:` prefix.
- [x] Every field zone inside an inspector registers with `parentZone === <entity-zone FQM>` — NOT `null`.
- [x] The per-entity zone itself registers at the inspector layer root with `parentZone === null`.
- [x] ArrowDown at the last field of inspector A stays inside inspector A — does NOT enter inspector B's fields. (Implementation note: the kernel cascade hits drill-out fallback → focus moves to entity zone A's FQM, not echoed on the field. Tests assert `focus stays in A (echoed field OR drill-out to entity zone)`. Original spec wording "stays on that field (echoed)" was inaccurate about the cascade — drill-out fallback is the well-formed path.)
- [x] ArrowUp at the first field of inspector A stays inside inspector A — does NOT enter inspector B's fields. (Same drill-out semantics.)
- [x] ArrowLeft/Right between adjacent panels still works — focus moves to the spatially-nearest peer **entity zone** (e.g. `task:TA`) via iter-1 escalation. (Implementation note: the existing four `inspector.cross-panel-nav.browser.test.tsx` tests had to be **updated** — the original spec said they would "pass unchanged" but the kernel's iter-1 same-kind filter means iter-1 finds zones, not their child fields, so the targets are now entity zones rather than leaf fields. Per follow-up clarification with @user (Q&A 20260430_155304): cross-panel nav lands on the entity zone; user descends via another arrow / Enter. This option was explicitly chosen.)
- [x] `<InspectorFocusBridge>` is NOT reintroduced. No `panel:*` moniker is registered. The `inspector.edit / editEnter / exitEdit` commands are NOT reintroduced (stays as `field.edit / field.editEnter` per `01KQCTJY1QZ710A05SE975GHNR`).
- [x] `inspectors-container.guards.node.test.ts` is updated to allow `FocusZone` in `inspectors-container.tsx` while still pinning the `InspectorFocusBridge` deletion and the absence of `panel:` monikers.
- [x] `npx tsc --noEmit` clean.
- [x] `cd kanban-app/ui && npx vitest run src/components/inspector.entity-zone-barrier.browser.test.tsx src/components/inspector.layer-shape.browser.test.tsx src/components/inspector.cross-panel-nav.browser.test.tsx src/components/inspector.boundary-nav.browser.test.tsx src/components/inspector.close-restores-focus.browser.test.tsx src/components/inspectors-container.guards.node.test.ts` all pass. (25/25 tests green across 5 consecutive runs.)
- [x] **Manual verification in `npm run tauri dev`**: confirmed by user 2026-04-30 ("working"). Multi-inspector cardinal nav stays within an entity; cross-entity escalation lands on the peer entity zone as designed.

## Tests

- [x] **New** `kanban-app/ui/src/components/inspector.entity-zone-barrier.browser.test.tsx`:
  - `ArrowDown at the last field of inspector A stays put — does not enter inspector B's fields`
  - `ArrowUp at the first field of inspector A stays put — does not enter inspector B's fields`
  - `cross-entity ArrowLeft from B's leftmost field lands on entity zone A (iter-1 escalation through entity-zone peers)` *(renamed from "lands on a field in A" — the kernel's iter-1 lands on the zone, not a field; same-kind filter restricts iter 1 to zones)*
  - `kernel-state shape: each open inspector registers an entity-keyed zone, field zones list the entity-zone FQM as parentZone`
- [x] **Update** `inspector.layer-shape.browser.test.tsx` — flipped "fields at layer root with parentZone=null" to "fields under entity zone with parentZone=<entity FQM>"; entity-moniker test now asserts a ZONE (not absent); kept `<InspectorFocusBridge>`-deletion + `panel:*`-absence guards.
- [x] **Update** `inspector.cross-panel-nav.browser.test.tsx` — assertions changed from `field:task:TA.status` (leaf) to `task:TA` (entity zone) per the iter-1 same-kind contract. (Per user Q&A 20260430_155304: option 1 chosen — accept zone-as-target, user descends via another keypress.)
- [x] **Update** `inspector.boundary-nav.browser.test.tsx` — single-inspector boundary tests now accept either echoed-field OR drill-out-to-entity-zone, since field zones now have a parent zone (the cascade hits drill-out fallback rather than null-stay-put).
- [x] **Update** `inspectors-container.guards.node.test.ts` — allows a single `FocusZone` import; pins the import path; pins `InspectorFocusBridge`, `panel:*`, and `inspector.edit/editEnter/exitEdit` deletions.
- [x] **Run command**: `cd kanban-app/ui && npx vitest run src/components/inspector.*.browser.test.tsx src/components/inspectors-container.guards.node.test.ts` — all green (25/25 across 5 consecutive runs).
- [x] **Run command**: `cd kanban-app/ui && npx vitest run src/components/path-monikers.kernel-driven.browser.test.tsx` — 7/7 pass; FQM contract holds.

## Workflow

- Use `/tdd` — write the four new entity-zone-barrier tests FIRST, watch them fail (or fail to compile, since the entity zone doesn't exist yet), then add the `<FocusZone>` wrap and update the layer-shape + guards tests.
- Verify manually in `tauri dev` per the last acceptance bullet — the `01KQCTJY1QZ710A05SE975GHNR` simplification was reviewed and "tests pass" but the multi-inspector case was missed; manual verification is the gate that catches that class of miss.