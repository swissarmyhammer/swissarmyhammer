/**
 * Three-inspector fixture for vitest-browser spatial-nav tests.
 *
 * ## Purpose
 *
 * Pairs with `spatial-inspector-fixture.tsx` (one inspector) and
 * `spatial-inspector-over-grid-fixture.tsx` (one inspector over a dense
 * background) to cover the real-world topology where the user has
 * opened three or more inspectors in a row — for example, "drill from
 * a project to one of its tasks to that task's parent". The production
 * `InspectorsContainer` stacks a `SlidePanel` per entry in
 * `inspector_stack`, each wrapped in an `InspectorFocusBridge` that
 * pushes its own `<FocusLayer name="inspector">`. With three inspectors
 * open the shim's layer stack is `[window, insp-1, insp-2, insp-3]` and
 * the active layer is `insp-3`. Every field-row `FocusScope` in
 * `insp-1`/`insp-2` must be invisible to spatial navigation while
 * `insp-3` is on top.
 *
 * This fixture models exactly that shape without pulling in the real
 * `InspectorsContainer` (which would need schema, entity store, Tauri
 * invoke mocks, etc.). The three inspector bodies are rendered inline,
 * each in its own `<FocusLayer name="inspector">` with a `<FocusScope
 * spatial={false}>` entity wrapper — identical to
 * `InspectorFocusBridge` — and four field rows apiece.
 *
 * ## Shape
 *
 * ```
 * <FocusLayer name="window">            // FixtureShell
 *   <card FocusScope>                   // opens the first inspector
 * </FocusLayer>
 *
 * <FocusLayer name="inspector">         // insp-1 (bottom)
 *   <FocusScope spatial={false}>        // entity scope (i1)
 *     <4 field rows, y:0..160>
 *   </FocusScope>
 * </FocusLayer>
 *
 * <FocusLayer name="inspector">         // insp-2 (middle)
 *   <FocusScope spatial={false}>        // entity scope (i2)
 *     <4 field rows, y:0..160>          // same y range as insp-1
 *   </FocusScope>
 * </FocusLayer>
 *
 * <FocusLayer name="inspector">         // insp-3 (active top)
 *   <FocusScope spatial={false}>        // entity scope (i3)
 *     <4 field rows, y:0..160>          // same y range as insp-1 & insp-2
 *   </FocusScope>
 * </FocusLayer>
 * ```
 *
 * Every inspector's field rows occupy the same vertical band — this is
 * the worst case for layer isolation, because a filter gap would hand
 * the win to the wrong layer on Up/Down beam-test ties.
 *
 * ## What the fixture does NOT do
 *
 * - No `SlidePanel` animation or `right` offset simulation — layer
 *   isolation is about the spatial registry, not visual layout.
 *   Horizontal offsets (x:300, x:600, x:900) keep the rects cleanly
 *   separated for left/right nav assertions but the y overlap is
 *   deliberate.
 * - No real editor stack, no `useInspectorNav`. Field rows are plain
 *   `<FocusScope>` cells as in the other inspector fixtures.
 * - No `ui.inspect` dispatch — tests mount all three inspectors up
 *   front via the `inspectorCount` prop.
 */

import { useEffect, useRef } from "react";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import { fieldMoniker, moniker } from "@/lib/moniker";
import { FixtureShell } from "./spatial-fixture-shell";

/** Canonical entity type for the three inspector entities. */
export const FIXTURE_CARD_TYPE = "task";

/**
 * The four field names rendered by each inspector, in display order.
 * Same shape as `spatial-inspector-fixture` — four is the minimum to
 * distinguish "j moves", "j at last clamps", "k at first clamps".
 */
export const FIXTURE_FIELD_NAMES = ["title", "status", "body", "tags"] as const;

/** IDs for the three inspector entities, bottom-to-top in the stack. */
export const FIXTURE_ENTITY_IDS = ["i1", "i2", "i3"] as const;

/**
 * Pre-computed entity monikers for every inspector. Index matches
 * `FIXTURE_ENTITY_IDS` so tests can address inspectors by stack position.
 */
export const FIXTURE_ENTITY_MONIKERS: readonly string[] =
  FIXTURE_ENTITY_IDS.map((id) => moniker(FIXTURE_CARD_TYPE, id));

/**
 * Pre-computed field monikers, keyed by (entity index, field index).
 * `FIXTURE_FIELD_MONIKERS[e][f]` is the moniker of the f-th field in
 * the e-th inspector. Tests use this grid to assert exactly which
 * field receives focus.
 */
export const FIXTURE_FIELD_MONIKERS: readonly (readonly string[])[] =
  FIXTURE_ENTITY_IDS.map((id) =>
    FIXTURE_FIELD_NAMES.map((name) =>
      fieldMoniker(FIXTURE_CARD_TYPE, id, name),
    ),
  );

/** Field row height in pixels. Matches the sibling inspector fixtures. */
const FIELD_ROW_HEIGHT = 40;

/** Horizontal offset between stacked inspectors. Tests use this to
 * reason about left/right nav — `x:300` is insp-1, `x:600` is insp-2,
 * etc. The column offsets match production's `right: N*PANEL_WIDTH`
 * behavior qualitatively but with smaller numbers for test ergonomics. */
const PANEL_X_GAP = 300;

/**
 * One inspector field row. Mirrors `FixtureFieldRow` in
 * `spatial-inspector-fixture.tsx` exactly — each field is a
 * `FocusScope` keyed by `fieldMoniker(type, id, fieldName)`.
 */
function FixtureFieldRow({
  entityId,
  fieldName,
}: {
  entityId: string;
  fieldName: string;
}) {
  const mk = fieldMoniker(FIXTURE_CARD_TYPE, entityId, fieldName);
  return (
    <FocusScope
      moniker={mk}
      commands={[]}
      data-testid={`field-row-${entityId}-${fieldName}`}
      style={{
        height: `${FIELD_ROW_HEIGHT}px`,
        padding: "8px",
        borderBottom: "1px solid #ccc",
      }}
    >
      <span>
        {entityId}:{fieldName}
      </span>
    </FocusScope>
  );
}

/**
 * Focus the first field of the topmost inspector on mount.
 *
 * Mirrors `useFirstFieldFocus` in the production inspector — when the
 * topmost inspector opens its first field claims focus. Only the
 * topmost inspector activates focus so that test assertions about
 * "which layer is active" match production behavior.
 */
function useFirstFieldOfTopInspectorOnMount(firstFieldMoniker: string): void {
  const { setFocus } = useEntityFocus();
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;

  useEffect(() => {
    setFocusRef.current(firstFieldMoniker);
  }, [firstFieldMoniker]);
}

/**
 * One inspector body. Wraps a column of field rows in a
 * `<FocusLayer name="inspector">` and a `<FocusScope spatial={false}>`
 * for the entity container — identical shape to
 * `InspectorFocusBridge` in production.
 *
 * The `xOffset` prop shifts the panel horizontally so stacked
 * inspectors have visually distinct rects. Without this, every panel
 * would register rects at the same x and `Right` nav would be
 * ambiguous against same-x siblings on the same layer.
 */
const INSPECTOR_PANEL_STYLE = {
  position: "fixed",
  top: 0,
  width: "280px",
  height: "100vh",
  background: "#fff",
  borderLeft: "1px solid #ccc",
  display: "flex",
  flexDirection: "column",
} as const;

function InspectorPanel({
  entityId,
  xOffset,
}: {
  entityId: string;
  xOffset: number;
}) {
  return (
    <div
      data-testid={`inspector-body-${entityId}`}
      style={{ ...INSPECTOR_PANEL_STYLE, left: `${xOffset}px` }}
    >
      {FIXTURE_FIELD_NAMES.map((name) => (
        <FixtureFieldRow key={name} entityId={entityId} fieldName={name} />
      ))}
    </div>
  );
}

function FixtureInspectorBody({
  entityId,
  xOffset,
  isTop,
}: {
  entityId: string;
  xOffset: number;
  isTop: boolean;
}) {
  const mk = moniker(FIXTURE_CARD_TYPE, entityId);
  const firstFieldMoniker = fieldMoniker(
    FIXTURE_CARD_TYPE,
    entityId,
    FIXTURE_FIELD_NAMES[0],
  );

  // Only the topmost inspector claims initial focus — matches production
  // where the most recently opened panel holds focus. Lower panels stay
  // visible but inert.
  useFirstFieldOfTopInspectorOnMount(isTop ? firstFieldMoniker : "");

  return (
    <FocusLayer name="inspector">
      <FocusScope
        moniker={mk}
        commands={[]}
        showFocusBar={false}
        spatial={false}
        data-testid={`inspector-entity-scope-${entityId}`}
      >
        <CommandScopeProvider commands={[]}>
          <InspectorPanel entityId={entityId} xOffset={xOffset} />
        </CommandScopeProvider>
      </FocusScope>
    </FocusLayer>
  );
}

/** Props for `AppWithMultiInspectorFixture`. */
export interface MultiInspectorFixtureProps {
  /**
   * How many inspectors to render, 1..=3. Defaults to 3 — the primary
   * multi-inspector regression coverage case. Tests can pass smaller
   * counts to verify behavior degrades gracefully; the relative order
   * in `FIXTURE_ENTITY_IDS` is preserved (i1 is always the bottom).
   */
  inspectorCount?: 1 | 2 | 3;
}

/**
 * Three-inspector fixture.
 *
 * Renders a `FixtureShell` window layer with a lone card plus `n`
 * inspector bodies (default 3), each on its own `FocusLayer`. The
 * topmost inspector's first field claims focus on mount; tests drive
 * j/k/h/l through `FixtureShell`'s nav commands and observe via the
 * shim handles.
 */
export function AppWithMultiInspectorFixture({
  inspectorCount = 3,
}: MultiInspectorFixtureProps = {}) {
  const extraCommands: CommandDef[] = [];

  const idsToRender = FIXTURE_ENTITY_IDS.slice(0, inspectorCount);
  const topIndex = idsToRender.length - 1;

  return (
    <EntityFocusProvider>
      <FixtureShell
        extraCommands={extraCommands}
        navOverrides={{ navFirstVim: "g g", navLastVim: "Shift+G" }}
      >
        <div
          data-testid="multi-inspector-root"
          style={{
            padding: "16px",
            width: "200px",
          }}
        >
          <FocusScope
            moniker={moniker("task", "background-card")}
            commands={[]}
            data-testid="fixture-background-card"
            style={{
              width: "150px",
              height: "60px",
              padding: "8px",
              border: "1px solid #ccc",
            }}
          >
            <span>Background</span>
          </FocusScope>
        </div>
        {idsToRender.map((id, index) => (
          <FixtureInspectorBody
            key={id}
            entityId={id}
            xOffset={(index + 1) * PANEL_X_GAP}
            isTop={index === topIndex}
          />
        ))}
      </FixtureShell>
    </EntityFocusProvider>
  );
}
