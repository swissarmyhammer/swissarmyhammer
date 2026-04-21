/**
 * Deterministic 3x3 tags-grid fixture for vitest-browser spatial-nav tests.
 *
 * ## Purpose
 *
 * Provides a self-contained React tree that exercises the full
 * `EntityFocusProvider` + `FocusLayer` + `KeybindingHandler` stack without
 * requiring the AppShell's UIState/AppMode/Schema providers. Tests render
 * `<AppWithGridFixture />` once, then drive navigation through real DOM
 * clicks and keyboard events; the mocked `invoke()` routes every spatial
 * command into the `SpatialStateShim` via `setupSpatialShim()`.
 *
 * ## Shape
 *
 * The fixture mirrors the shape of production's tag grid:
 * - Each row represents a tag entity (`tag-0`, `tag-1`, `tag-2`).
 * - Each row has 2 cells: `tag_name`, `color`.
 * - Cell monikers follow `fieldMoniker(type, id, field)` →
 *   `field:tag:tag-N.<field>`.
 * - Cells carry `data-moniker` and `data-testid="data-moniker:<moniker>"`
 *   so tests can assert focus via `getByTestId()` without coupling to
 *   implementation-internal selectors.
 *
 * ## What the fixture does NOT do
 *
 * Cells are plain `<div>`s, not `FocusScope`s — this mirrors production's
 * current state (row-level FocusScopes only) and is the point of the
 * canonical failing test: pressing `j` should move cell focus row-to-row,
 * but without per-cell FocusScopes the spatial nav engine has no cell
 * entries to navigate between. When a sibling task wraps each cell in a
 * `FocusScope`, the canonical `j` test flips from red to green; the
 * fixture itself doesn't need to change.
 */

import { type ReactNode } from "react";
import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedMoniker,
} from "@/lib/entity-focus-context";
import { FocusScope, useFocusScopeElementRef } from "@/components/focus-scope";
import { fieldMoniker, moniker, ROW_SELECTOR_FIELD } from "@/lib/moniker";
import { FixtureShell } from "./spatial-fixture-shell";

/** Number of rows and columns in the canonical 3x3 fixture. */
export const GRID_ROWS = 3;

// Alias retained for module-local expressions.
const ROWS = GRID_ROWS;

/** The two field columns rendered by the fixture, in display order. */
const COLUMNS = ["tag_name", "color"] as const;

/** Pre-computed row monikers so tests can reference them by index. */
export const FIXTURE_ROW_MONIKERS: readonly string[] = Array.from(
  { length: ROWS },
  (_, i) => moniker("tag", `tag-${i}`),
);

/** Pre-computed cell monikers, row-major. */
export const FIXTURE_CELL_MONIKERS: ReadonlyArray<readonly string[]> =
  FIXTURE_ROW_MONIKERS.map((_, r) =>
    COLUMNS.map((c) => fieldMoniker("tag", `tag-${r}`, c)),
  );

/**
 * Pre-computed row-selector monikers, one per row.
 *
 * Shape: `field:tag:tag-<N>.__rowselector`. Tests use these to target
 * the selector cell via `getByTestId("data-moniker:" + moniker)`.
 *
 * The `ROW_SELECTOR_FIELD` constant is shared with production's
 * `DataTable` — imported from `@/lib/moniker` so both sides of the
 * fixture/prod split cannot drift.
 */
export const FIXTURE_ROW_SELECTOR_MONIKERS: readonly string[] =
  FIXTURE_ROW_MONIKERS.map((_, r) =>
    fieldMoniker("tag", `tag-${r}`, ROW_SELECTOR_FIELD),
  );

/**
 * One row in the tag grid. Wrapped in a `FocusScope` so the row itself
 * is a spatial entry — matches production's DataTableRow behavior
 * exactly (cells are plain divs, not FocusScopes).
 *
 * Row height and width are set inline so `getBoundingClientRect()`
 * produces predictable rects in the headless browser.
 */
export function FixtureRow({ rowIndex }: { rowIndex: number }) {
  const rowMoniker = FIXTURE_ROW_MONIKERS[rowIndex];
  const cellMonikers = FIXTURE_CELL_MONIKERS[rowIndex];
  const selectorMoniker = FIXTURE_ROW_SELECTOR_MONIKERS[rowIndex];

  // `spatial={false}` removes the row from the Rust beam-test graph —
  // the row still participates in focus/commands, but its rect does
  // not shadow the cell rects during cardinal-direction searches.
  //
  // `renderContainer={true}` keeps a real `<div>` for the row layout;
  // the row selector and cells each render their own `FocusScope` with
  // `renderContainer={false}` so there is exactly one DOM element per
  // spatial entry.
  return (
    <FocusScope
      moniker={rowMoniker}
      commands={[]}
      spatial={false}
      style={{
        display: "flex",
        height: "40px",
        borderBottom: "1px solid #ccc",
      }}
    >
      <FocusScope
        moniker={selectorMoniker}
        commands={[]}
        renderContainer={false}
      >
        <FixtureCellDiv
          dataMoniker={selectorMoniker}
          style={{ width: "40px", padding: "8px", textAlign: "center" }}
        >
          {rowIndex + 1}
        </FixtureCellDiv>
      </FocusScope>
      {cellMonikers.map((cellMk, col) => (
        <FocusScope
          key={cellMk}
          moniker={cellMk}
          commands={[]}
          renderContainer={false}
        >
          <FixtureCellDiv
            dataMoniker={cellMk}
            style={{ flex: 1, padding: "8px" }}
          >
            {COLUMNS[col]} r{rowIndex}
          </FixtureCellDiv>
        </FocusScope>
      ))}
    </FocusScope>
  );
}

/**
 * A `<div>` that plays the role of a `<td>` in the fixture:
 *
 * - Reads `elementRef` from the enclosing non-container `FocusScope` so
 *   `ResizeObserver` can measure this element's rect.
 * - Wires a click handler that focuses the scope via `setFocus`. This
 *   mirrors production's `RowSelector` / `DataTableCell`: the cell owns
 *   the click because the `FocusScope` has `renderContainer={false}`
 *   and cannot attach its own handler.
 * - Forwards `data-moniker` and `data-testid` so tests can query the
 *   element by moniker.
 */
interface FixtureCellDivProps {
  dataMoniker: string;
  style: React.CSSProperties;
  children: ReactNode;
}

function FixtureCellDiv({ dataMoniker, style, children }: FixtureCellDivProps) {
  const elementRef = useFocusScopeElementRef();
  const { setFocus } = useEntityFocus();
  const focusedMoniker = useFocusedMoniker();
  const isFocused = focusedMoniker === dataMoniker;

  return (
    <div
      ref={elementRef as React.RefObject<HTMLDivElement>}
      data-moniker={dataMoniker}
      data-testid={`data-moniker:${dataMoniker}`}
      data-focused={isFocused || undefined}
      style={style}
      onClick={(e) => {
        e.stopPropagation();
        setFocus(dataMoniker);
      }}
    >
      {children}
    </div>
  );
}

/**
 * 3x3 tag-grid fixture ready for rendering in vitest-browser tests.
 *
 * Usage:
 * ```tsx
 * const { shim } = setupSpatialShim();
 * const screen = await render(<AppWithGridFixture />);
 * const cell = screen.getByTestId("data-moniker:field:tag:tag-0.tag_name");
 * ```
 *
 * All Tauri IPC goes through the shim — no real backend involvement.
 */
export function AppWithGridFixture() {
  return (
    <EntityFocusProvider>
      <FixtureShell>
        <div
          data-testid="grid-fixture-root"
          style={{
            width: "400px",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {Array.from({ length: ROWS }, (_, r) => (
            <FixtureRow key={r} rowIndex={r} />
          ))}
        </div>
      </FixtureShell>
    </EntityFocusProvider>
  );
}
