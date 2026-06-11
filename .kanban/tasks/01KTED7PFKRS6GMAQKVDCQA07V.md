---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9780
project: ui-command-cleanup
title: Card E — Move editor drill-in commands to plugins + handler bus
---
## What
Move the three editor "drill-in" command DEFINITIONS out of React and into a plugin, routing their CM6/editor-focus behaviors through the handler bus (Card B).

Sites:
- `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx` — `filter_editor.drillIn` (focus the CM6 filter editor).
- `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx` — `ui.ai-panel.composer.drillIn`.
- `apps/kanban-app/ui/src/components/ai-elements/elicitation.tsx` — `ui.ai-panel.elicitation.field.drillIn:${key}` (note the per-field dynamic id suffix).

Approach:
- Define `filter_editor.drillIn`, `ui.ai-panel.composer.drillIn`, and the elicitation field drill-in in a plugin (likely `builtin/plugins/ui-commands/index.ts`), with id/name/keys/scope; no backend op, marked "handled in webview".
- For the elicitation per-field dynamic id (`...drillIn:${key}`): the plugin defines the base command; the dynamic key is passed as an ARG at dispatch (the bus handler reads the field key from ctx.args) — do NOT mint one plugin command per field. Confirm whether the existing palette/keymap needs the suffix or whether arg-passing suffices; document the decision in the card's implementation notes.
- Replace the client-side defs with `registerWebviewCommandHandler` registrations; the editors keep owning their CM6 focus logic.

## Acceptance Criteria
- [x] `filter_editor.drillIn`, `ui.ai-panel.composer.drillIn`, and the elicitation field drill-in are plugin-defined; the three components no longer DEFINE them.
- [x] Editor focus (CM6 filter, composer, elicitation field) still occurs on drill-in, via the bus.
- [x] The elicitation per-field variation is expressed as a dispatch ARG, not N minted command ids. (See Implementation Notes — superseded by focus-gated registration: ONE base id, ZERO args needed.)
- [x] GUARD (presentation-only invariant): drill-in handlers only focus a live editor instance (no durable mutation). perspective-tab-bar.tsx, ai-prompt-composer.tsx, and ai-elements/elicitation.tsx must NOT import `@/lib/mcp-transport`. `webview-command-bus.guard.node.test.ts` stays green.

## Tests
- [x] UI: extend `apps/kanban-app/ui/src/components/perspective-tab-bar.filter-enter.spatial.test.tsx` (filter drill focuses CM6), `apps/kanban-app/ui/src/components/ai-panel-elicitation.spatial.test.tsx` (elicitation field drill focuses the right field by key), and add/extend an ai-prompt-composer test for composer drill-in.
- [x] Plugin e2e: the three drill-in ids are registered with expected metadata.
- [x] `webview-command-bus.guard.node.test.ts` green with the three drill-in components as registration sites.
- [x] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Implementation Notes (Card E, 2026-06-11)

**Where the defs landed:** `builtin/plugins/ui-commands/index.ts` — the `UI_SURFACE_COMMANDS` data table grew from 4 (Card D) to 7 entries. All three drill-ins: keys `{cua,vim,emacs}: Enter` (1:1 from the retired React defs), `undoable: false`, inert `{ok:true}` host execute, scope-gated.

**Scopes:**
- `filter_editor.drillIn` → `scope: ["ui:filter_editor"]` — NEW constant marker (`FILTER_EDITOR_COMMAND_SCOPE`, exported from perspective-tab-bar.tsx) mounted via `CommandScopeProvider` above the dynamic `filter_editor:{perspectiveId}` FocusScope, exactly like Card D's `ui:field`.
- `ui.ai-panel.composer.drillIn` → `scope: ["ui:ai-panel.composer"]` — the composer FocusScope's OWN constant moniker; no marker needed (a FocusScope mounts its segment as a command-scope moniker, so the chain walk matches it literally).
- `ui.ai-panel.elicitation.field.drillIn` → `scope: ["ui:ai-panel.elicitation.field"]` — NEW constant marker (`ELICITATION_FIELD_COMMAND_SCOPE`, exported from elicitation.tsx) above each text-like field's dynamic `ui:ai-panel.elicitation.field:{key}` leaf.

**DECISION — elicitation per-field suffix vs ARG (the card's open question):** Neither the suffix NOR an arg is needed. The keymap layer dispatches bare command ids (no args can ride the Enter path), so arg-passing at dispatch was never viable. Instead the per-field variation is carried by Card D's focus-gated bus registration (`useFocusedWebviewCommandHandlers`, built after this card was written): each field instance registers the ONE base id's handler only while spatial focus is within ITS leaf, and the handler closure owns its own `inputRef`. Disjoint subtrees ⇒ the bus slot always holds exactly the focused field's closure. Palette/keymap need only the base id. The retired minted form `...drillIn:${key}` is gone; the new plugin-owned guard test rejects it (including the template-literal form) if it ever reappears.

**Handler registration:** all three components use `useFocusedWebviewCommandHandlers` (Card D hook, reused — not rebuilt): FilterFormulaBarFocusable on segment `filter_editor:{id}`, AiPromptComposer on `ui:ai-panel.composer`, and elicitation's `useFieldDrillIn(key, inputRef)` (re-shaped from CommandDef-minting to bus-registering) on `fieldSegment(key)`. Handlers are pure presentation: one `editorRef/inputRef.current?.focus()` call. TextEditor untouched — focus semantics live at the callers, per the TextEditor principle.

**Cleanup:** `AiPanelFocusScope`'s `commands` prop was removed (no production consumer remained after the move).

**Test layers shipped:**
- Rust e2e: `builtin_ui_commands_e2e.rs` 14→17 commands + 3 metadata asserts + inert-execute coverage (red-first: failed with the 3 ids missing, green after plugin entries).
- `full_baseline_e2e.rs`: 92→95 ids (red-first).
- Mirror: `mock-command-list.ts::UI_SURFACE_PLUGIN_COMMANDS` 4→7 + `ui-surface-plugin-commands-mirror.spatial.node.test.ts` (drift guard went red on the plugin change, green after mirror update).
- Plugin-owned guard: NEW `editor-drill-in-commands.plugin-owned.node.test.ts` (red naming the 3 offender files, green after def removal).
- Behavior (red-first via def removal, green after bus wiring): NEW filter drill-in test in `perspective-tab-bar.filter-enter.spatial.test.tsx` (CM6 focus spy); existing composer test extended with a bus-not-backend negative assertion; existing elicitation drill-in/drill-out tests re-prove the bus path.

**Out of scope, tracked separately:** pre-existing `perspective-tab-bar.filter-migration.test.tsx` failures (2 tests, empty `spatial_focus.fq`) — verified to reproduce against HEAD's perspective-tab-bar.tsx; card 01KTVMX1XBF6VCBHV0V3NH9NSV.

## Review Findings (2026-06-11 10:45)

### Warnings
- [x] `apps/kanban-app/ui/src/lib/webview-command-bus.guard.node.test.ts:42` — The guard's `registersWebviewHandler` detector only matches direct `registerWebviewCommandHandler(` calls, but all three Card E components (plus Card D's `field.tsx` / `pressable.tsx` — now 5 files) register via the `useFocusedWebviewCommandHandlers` hook and are therefore invisible to the scan. The card's acceptance criterion claims the guard covers "the three drill-in components as registration sites" — it does not: if one of these files later imported `@/lib/mcp-transport`, the guard would stay green and the presentation-only invariant would be violated silently. (Verified the components are transport-free today — the invariant holds in fact, not mechanically.) Suggested fix: extend the detector to also match `useFocusedWebviewCommandHandlers\s*\(` and add a known-bad unit case for the hook-mediated form.

### Nits
- [x] `apps/kanban-app/ui/src/components/ai-panel-elicitation.spatial.test.tsx:604` — The per-kind drill-in test renders all 4 text-like fields concurrently and proves the focused field's closure wins for each key (strong disjointness coverage), but each case is a fresh render: no single render exercises an explicit field A → field B focus handoff followed by Enter (proving A's stale closure never lingers in the bus slot after the swap). The bus's ownership-guarded cleanup makes this safe by construction; a one-render A→B sequence would pin it behaviorally.

## Review Fixes (2026-06-11)

**Warning (guard blind to hook-mediated registration):** `registersWebviewHandler` in `webview-command-bus.guard.node.test.ts` now matches BOTH `registerWebviewCommandHandler(` and `useFocusedWebviewCommandHandlers(` call sites, excluding both mechanism modules' own `export function …` declarations. TDD red-first: added (1) a hook-form detector unit case, (2) a synthetic known-bad case (hook call + `@/lib/mcp-transport` import) per the file's self-proving pattern, and (3) a real-file assertion that the 5 hook sites (`fields/field.tsx`, `pressable.tsx`, `perspective-tab-bar.tsx`, `ai-prompt-composer.tsx`, `ai-elements/elicitation.tsx`) are detected as registration sites AND stay transport-free. Watched all 3 fail (`expected false to be true` — detector blind to the hook form), then extended the detector: 8/8 green; the directory scan now mechanically covers hook-mediated sites.

**Nit (A→B handoff):** added `"Enter after a field A → field B focus handoff drills into B's input, not A's"` to `ai-panel-elicitation.spatial.test.tsx` — single render of the all-kinds form, `commitFocus` on summary (A), then `drillInto` amount (B); asserts `document.activeElement` is B's input and not A's. Green on first run, as expected for a coverage-pinning nit (the bus's ownership-guarded cleanup already guarantees the behavior).

**Verification:** `npx vitest run src/lib/webview-command-bus.guard.node.test.ts src/components/ai-panel-elicitation.spatial.test.tsx` → 2 files passed, 21/21 tests (8 unit + 13 browser). `npx tsc --noEmit` → clean.