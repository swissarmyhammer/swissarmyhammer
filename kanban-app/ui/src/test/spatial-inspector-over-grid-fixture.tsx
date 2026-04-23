/**
 * Inspector-over-dense-grid fixture for vitest-browser spatial-nav tests.
 *
 * ## Purpose
 *
 * Reproduces the reported regression described in kanban
 * `01KPS22R2T4Q5QT9A71E7ZWAAP`: inspector spatial navigation (j/k between
 * field rows) works when the background view is a sparse board but breaks
 * when the background view is a dense grid — field rows get skipped and
 * `j` can leap to footer actions over middle fields.
 *
 * The canonical `spatial-inspector-fixture.tsx` only puts a single card in
 * the window layer behind the inspector, so it cannot exercise the
 * "dense background layer" code path. This fixture wires the shared
 * `FixtureShell` (window layer) around a full tag grid containing many
 * rows plus header/selector FocusScopes, and mounts the inspector panel
 * on top as its own `FocusLayer`.
 *
 * ## Shape
 *
 * ```
 * <FocusLayer name="window">               // FixtureShell
 *   <dense grid: N rows × (selector + 2 cells) + header row>
 *   <card FocusScope>                      // opens the inspector on dblclick
 *   ui.inspect → setInspectorOpen(true)
 * </FocusLayer>
 *
 * <FocusLayer name="inspector">            // rendered when inspectorOpen
 *   <FocusScope moniker={entity.moniker}>  // mirrors production bridge
 *     <4 vertically stacked field rows>
 *   </FocusScope>
 * </FocusLayer>
 * ```
 *
 * The outer entity-level `FocusScope` inside the inspector layer matches
 * the production wrapper at
 * `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — important
 * because hypothesis #1 in the task description is that this outer scope's
 * large rect shadows its own field children during beam-test scoring.
 *
 * Row heights, widths, and positions are chosen so
 * `getBoundingClientRect()` returns predictable rects in the headless
 * browser: cells are 32×32, rows stack with no gaps, inspector panel is a
 * 300px right-side column, fields are 40px tall.
 *
 * ## What the fixture does NOT do
 *
 * - No real schema, no real `EntityInspector` component — we exercise the
 *   layer/focus/spatial-scope contract, not the inspector's editor stack.
 * - No edit mode — `app.dismiss` only closes the inspector.
 * - No production grid component — a hand-rolled flex table mirrors the
 *   production grid's `FocusScope` registration shape (row selector +
 *   per-cell scopes + header cells) with tunable row count.
 */

import { useEffect, useRef, useState } from "react";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import {
  columnHeaderMoniker,
  fieldMoniker,
  moniker,
  ROW_SELECTOR_FIELD,
} from "@/lib/moniker";
import { FixtureShell } from "./spatial-fixture-shell";

/** Canonical entity type + id for the card the inspector opens on. */
export const FIXTURE_CARD_TYPE = "task";
export const FIXTURE_CARD_ID = "card-1-1";
export const FIXTURE_CARD_MONIKER = moniker(FIXTURE_CARD_TYPE, FIXTURE_CARD_ID);

/**
 * The four field names rendered by the inspector, in display order.
 *
 * `title` is in the `header` section; the remaining three are in the
 * `body` section. Four rows is the minimum that lets tests distinguish
 * "j moves one step", "j at last clamps", and — critically for this
 * regression — "j at the first header field reaches the first body field
 * instead of skipping past it".
 */
export const FIXTURE_FIELD_NAMES = ["title", "status", "body", "tags"] as const;

/**
 * Section assignment per field. Mirrors production's inspector sections
 * so a header-to-body transition is exercised, which is where the
 * from-grid skipping was reported.
 */
const FIXTURE_FIELD_SECTIONS: Record<
  (typeof FIXTURE_FIELD_NAMES)[number],
  "header" | "body"
> = {
  title: "header",
  status: "body",
  body: "body",
  tags: "body",
};

/** Pre-computed field monikers so tests can reference them by index. */
export const FIXTURE_FIELD_MONIKERS: readonly string[] =
  FIXTURE_FIELD_NAMES.map((name) =>
    fieldMoniker(FIXTURE_CARD_TYPE, FIXTURE_CARD_ID, name),
  );

/**
 * Number of grid rows the fixture renders by default.
 *
 * 50 is dense enough to populate the background layer with significantly
 * more scopes than the inspector has (4), so if layer isolation or beam
 * scoring misbehaves the grid scopes will dominate the candidate pool.
 * Tests can override via `gridRowCount` prop.
 */
export const DEFAULT_GRID_ROW_COUNT = 50;

/** The two field columns rendered by the grid. Mirrors the tag-grid fixture. */
const GRID_COLUMNS = ["tag_name", "color"] as const;

/**
 * One inspector field row. Wraps a plain label div in a `FocusScope` so
 * each field is its own spatial entry — matches production's
 * `EntityInspector` where every field row is wrapped in a `FocusScope`
 * keyed by `fieldMoniker(type, id, fieldName)`.
 *
 * Row height is hard-coded to mirror the prod row height so the beam
 * test geometry reflects real usage.
 */
function FixtureFieldRow({
  fieldName,
  section,
}: {
  fieldName: string;
  section: "header" | "body";
}) {
  const mk = fieldMoniker(FIXTURE_CARD_TYPE, FIXTURE_CARD_ID, fieldName);
  return (
    <FocusScope
      moniker={mk}
      commands={[]}
      data-testid={`field-row-${fieldName}`}
      data-section={section}
      style={{
        height: "40px",
        padding: "8px",
        borderBottom: "1px solid #ccc",
      }}
    >
      <span>{fieldName}</span>
    </FocusScope>
  );
}

/**
 * Focus the first field on mount.
 *
 * Mirrors `useFirstFieldFocus` in `entity-inspector.tsx` so the fixture
 * exercises the same "inspector opens → first field claims focus" flow
 * production uses when arriving from the grid.
 */
function useFirstFieldFocusOnMount(firstFieldMoniker: string): void {
  const { setFocus } = useEntityFocus();
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;

  useEffect(() => {
    setFocusRef.current(firstFieldMoniker);
  }, [firstFieldMoniker]);
}

interface InspectorBodyProps {
  onClose: () => void;
}

/**
 * The inspector panel wrapped in a `FocusLayer` and a matching
 * `FocusScope` for the entity itself — this last scope mirrors the
 * production `InspectorFocusBridge` wrapper and is the one hypothesis #1
 * suspects of attracting beam-test scoring with its large rect.
 *
 * Registering `app.dismiss` in a nested `CommandScopeProvider` closes
 * the panel when Escape fires.
 */
function InspectorBody({ onClose }: InspectorBodyProps) {
  const firstFieldMoniker = FIXTURE_FIELD_MONIKERS[0];
  useFirstFieldFocusOnMount(firstFieldMoniker);

  const inspectorCommands: CommandDef[] = [
    {
      id: "app.dismiss",
      name: "Close Inspector",
      keys: { vim: "Escape", cua: "Escape" },
      execute: onClose,
    },
  ];

  return (
    <FocusLayer name="inspector">
      {/*
       * `spatial={false}` — mirrors production `InspectorFocusBridge`.
       * The entity scope is a container for commands; the field rows
       * are the real spatial targets. Without this opt-out, the outer
       * rect swallows `j` from the last field and focus lands on the
       * entity scope instead of clamping.
       */}
      <FocusScope
        moniker={FIXTURE_CARD_MONIKER}
        commands={[]}
        showFocusBar={false}
        spatial={false}
        data-testid="inspector-entity-scope"
      >
        <CommandScopeProvider commands={inspectorCommands}>
          <div
            data-testid="inspector-body"
            style={{
              position: "fixed",
              top: 0,
              right: 0,
              width: "300px",
              height: "100vh",
              background: "#fff",
              borderLeft: "1px solid #ccc",
              display: "flex",
              flexDirection: "column",
            }}
          >
            {FIXTURE_FIELD_NAMES.map((name) => (
              <FixtureFieldRow
                key={name}
                fieldName={name}
                section={FIXTURE_FIELD_SECTIONS[name]}
              />
            ))}
          </div>
        </CommandScopeProvider>
      </FocusScope>
    </FocusLayer>
  );
}

/**
 * One row in the background grid. Mirrors `FixtureRow` in
 * `spatial-grid-fixture.tsx` exactly — `spatial={false}` on the row
 * container so only the selector and cells register rects.
 */
function DenseGridRow({ rowIndex }: { rowIndex: number }) {
  const rowMoniker = moniker("tag", `tag-${rowIndex}`);
  const selectorMoniker = fieldMoniker(
    "tag",
    `tag-${rowIndex}`,
    ROW_SELECTOR_FIELD,
  );
  return (
    <FocusScope
      moniker={rowMoniker}
      commands={[]}
      spatial={false}
      style={{
        display: "flex",
        height: "32px",
        borderBottom: "1px solid #eee",
      }}
    >
      <FocusScope
        moniker={selectorMoniker}
        commands={[]}
        data-testid={`data-moniker:${selectorMoniker}`}
        style={{ width: "32px", padding: "4px", textAlign: "center" }}
      >
        <span>{rowIndex + 1}</span>
      </FocusScope>
      {GRID_COLUMNS.map((col) => {
        const mk = fieldMoniker("tag", `tag-${rowIndex}`, col);
        return (
          <FocusScope
            key={mk}
            moniker={mk}
            commands={[]}
            data-testid={`data-moniker:${mk}`}
            style={{ flex: 1, padding: "4px" }}
          >
            <span>
              {col} r{rowIndex}
            </span>
          </FocusScope>
        );
      })}
    </FocusScope>
  );
}

/** Column header row for the background grid. */
function DenseGridHeader() {
  return (
    <div
      style={{
        display: "flex",
        height: "32px",
        borderBottom: "1px solid #888",
        fontWeight: "bold",
      }}
    >
      <div style={{ width: "32px" }} />
      {GRID_COLUMNS.map((col) => {
        const mk = columnHeaderMoniker(col);
        return (
          <FocusScope
            key={mk}
            moniker={mk}
            commands={[]}
            data-testid={`data-moniker:${mk}`}
            style={{ flex: 1, padding: "4px" }}
          >
            <span>{col}</span>
          </FocusScope>
        );
      })}
    </div>
  );
}

/**
 * The full-width dense grid sitting in the window layer, behind the
 * inspector. Rendered as a single container so the overlay inspector
 * sits on top visually; spatially, layer isolation is what keeps the
 * inspector's navigation from reaching these rows.
 */
function DenseGrid({ rowCount }: { rowCount: number }) {
  return (
    <div
      data-testid="dense-grid-root"
      style={{
        width: "400px",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <DenseGridHeader />
      {Array.from({ length: rowCount }, (_, r) => (
        <DenseGridRow key={r} rowIndex={r} />
      ))}
    </div>
  );
}

/**
 * Props for `AppWithInspectorOverGridFixture`. Tests override
 * `gridRowCount` to scale the background layer up or down; the default
 * of 50 matches the acceptance criterion "dense grid (50+ rows)".
 */
export interface InspectorOverGridFixtureProps {
  /** Number of grid rows in the background window layer. Defaults to 50. */
  gridRowCount?: number;
}

/**
 * Inspector-over-dense-grid fixture.
 *
 * Renders a dense grid in the window layer plus a card that opens the
 * inspector. When the inspector mounts it is on its own `FocusLayer`
 * with an entity-level `FocusScope` and four field rows — matching the
 * shape of production's `InspectorFocusBridge`.
 *
 * Tests drive navigation via the shared shim-backed `FixtureShell` —
 * the window layer owns nav commands (`j`/`k`/`h`/`l` → `nav.*`) and
 * those commands broadcast into `useEntityFocus`, which invokes
 * `spatial_navigate` on the (shimmed) Rust core.
 */
export function AppWithInspectorOverGridFixture({
  gridRowCount = DEFAULT_GRID_ROW_COUNT,
}: InspectorOverGridFixtureProps = {}) {
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const openInspector = () => setInspectorOpen(true);
  const closeInspector = () => setInspectorOpen(false);

  // Keep the open callback stable across renders so `extraCommands`
  // doesn't re-register a new `ui.inspect` entry every render.
  const openInspectorRef = useRef(openInspector);
  openInspectorRef.current = openInspector;

  const extraCommands: CommandDef[] = [
    {
      id: "ui.inspect",
      name: "Open Inspector",
      execute: () => {
        openInspectorRef.current();
      },
    },
  ];

  return (
    <EntityFocusProvider>
      <FixtureShell
        extraCommands={extraCommands}
        navOverrides={{ navFirstVim: "g g", navLastVim: "Shift+G" }}
      >
        <div
          data-testid="inspector-over-grid-root"
          style={{
            display: "flex",
            flexDirection: "column",
            padding: "16px",
          }}
        >
          <DenseGrid rowCount={gridRowCount} />
          {/*
           * Card scope used to open the inspector. Placed below the
           * grid so its rect is visually distinct from any grid row
           * rect — avoids accidental rect collisions in the beam test.
           */}
          <FocusScope
            moniker={FIXTURE_CARD_MONIKER}
            commands={[]}
            data-testid="fixture-card"
            style={{
              marginTop: "8px",
              width: "200px",
              height: "60px",
              padding: "8px",
              border: "1px solid #ccc",
              cursor: "pointer",
            }}
          >
            <span>Card 1-1</span>
          </FocusScope>
        </div>
        {inspectorOpen && <InspectorBody onClose={closeInspector} />}
      </FixtureShell>
    </EntityFocusProvider>
  );
}
