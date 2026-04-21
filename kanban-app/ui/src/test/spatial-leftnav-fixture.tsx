/**
 * Deterministic LeftNav + board/grid fixture for vitest-browser
 * spatial-nav tests.
 *
 * ## Purpose
 *
 * Composes the real production `<LeftNav />` alongside the existing
 * 3x3 board and grid fixtures so the shared `h`/`j`/`k`/`l` contract
 * between a view body and the left-edge nav strip is exercised
 * end-to-end. The spatial engine, `EntityFocusProvider`, `FocusLayer`,
 * and `FixtureShell` keybinding wiring are all identical to
 * `spatial-board-fixture.tsx` and `spatial-grid-fixture.tsx` — the
 * only thing this fixture adds is the LeftNav strip flanking the
 * view body on the left edge.
 *
 * The LeftNav `useViews()` hook is replaced with a fixed list of two
 * views — `board` (active) and `grid` — so the strip has two buttons
 * for `j`/`k` navigation between views. `useViews` is mocked at the
 * test-file level with `vi.mock(...)`; this fixture only defines the
 * fixed view list and the layout.
 *
 * ## Shape
 *
 * ```
 *   ┌──LeftNav──┐   ┌──────── view body (board or grid) ────────┐
 *   │ view:board│   │ column:col-1   column:col-2   column:col-3│
 *   │ view:grid │   │ ...                                       │
 *   └───────────┘   └───────────────────────────────────────────┘
 * ```
 *
 * The LeftNav sits in a flex row to the left of the view body. Real
 * production composition (`ViewsContainer`) uses the same arrangement,
 * so the relative rects reported to the spatial engine model the real
 * geometry.
 */

import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { LeftNav } from "@/components/left-nav";
import { moniker } from "@/lib/moniker";
import { TooltipProvider } from "@/components/ui/tooltip";
import { FixtureShell } from "./spatial-fixture-shell";
import type { ViewDef } from "@/types/kanban";

/**
 * Fixed list of views the fixture renders in the LeftNav strip.
 *
 * Two entries are the minimum needed to test `j`/`k` between view
 * buttons; both have real `lucide-react` icon names so the icon
 * resolver inside `LeftNav` finds a match (instead of falling back
 * to the default `LayoutGrid`).
 */
export const FIXTURE_VIEWS: readonly ViewDef[] = [
  { id: "board", name: "Board", kind: "board", icon: "kanban" },
  { id: "grid", name: "Grid", kind: "grid", icon: "table" },
];

/** The id of the view rendered as active when the fixture mounts. */
export const FIXTURE_ACTIVE_VIEW_ID = FIXTURE_VIEWS[0].id;

/**
 * Pre-computed view-button monikers, one per entry in {@link FIXTURE_VIEWS}.
 *
 * Uses the same `moniker("view", id)` shape the production `LeftNav`
 * assigns to each `FocusScope`. Tests assert against these monikers
 * via `getByTestId("data-moniker:" + moniker)` or by matching the
 * focused moniker's prefix (`/^view:/`).
 */
export const FIXTURE_VIEW_MONIKERS: readonly string[] = FIXTURE_VIEWS.map((v) =>
  moniker("view", v.id),
);

/**
 * Reusable props shape so test fixtures that wrap a different body
 * (board vs grid) can share this shell.
 */
interface LeftNavFixtureShellProps {
  /** The view body rendered to the right of the LeftNav strip. */
  children: React.ReactNode;
  /** `data-testid` for the outer flex wrapper — helps tests find the root. */
  rootTestId: string;
}

/**
 * Flex-row layout composing `<LeftNav />` on the left edge with a
 * caller-supplied view body on the right.
 *
 * Owns the `EntityFocusProvider`, `FixtureShell` (which owns the
 * `FocusLayer` and nav commands), and the flex layout — matches the
 * production `ViewsContainer` shape from `views-container.tsx`.
 */
function LeftNavFixtureShell({
  children,
  rootTestId,
}: LeftNavFixtureShellProps) {
  // `TooltipProvider` wraps the entire shell because `LeftNav` renders
  // `Tooltip` around every view button — production mounts the
  // provider high in `window-container.tsx`, so tests must reproduce
  // that ancestor to avoid "`Tooltip` must be used within
  // `TooltipProvider`" at render time.
  return (
    <TooltipProvider delayDuration={0}>
      <EntityFocusProvider>
        <FixtureShell>
          <div
            data-testid={rootTestId}
            style={{
              display: "flex",
              flexDirection: "row",
              alignItems: "stretch",
              minHeight: "400px",
            }}
          >
            <LeftNav />
            {children}
          </div>
        </FixtureShell>
      </EntityFocusProvider>
    </TooltipProvider>
  );
}

/**
 * Board + LeftNav fixture.
 *
 * Imports the same 3x3 board column tree used by
 * `spatial-board-fixture.tsx`, but re-rooted under this shell so the
 * LeftNav strip is rendered at the same flex-row level as the columns
 * — just as `ViewsContainer` composes `<LeftNav />` with the view
 * body in production.
 */
export function AppWithBoardAndLeftNavFixture() {
  // Inline the board body here rather than reusing AppWithBoardFixture,
  // because AppWithBoardFixture mounts its own EntityFocusProvider +
  // FixtureShell. Composing fixtures by wrapping would produce two
  // nested shells and double-register the `window` FocusLayer.
  return (
    <LeftNavFixtureShell rootTestId="board-and-leftnav-fixture-root">
      <FixtureBoardBody />
    </LeftNavFixtureShell>
  );
}

/**
 * Grid + LeftNav fixture.
 *
 * Same pattern as {@link AppWithBoardAndLeftNavFixture}: inline the
 * grid body rather than nesting fixtures.
 */
export function AppWithGridAndLeftNavFixture() {
  return (
    <LeftNavFixtureShell rootTestId="grid-and-leftnav-fixture-root">
      <FixtureGridBody />
    </LeftNavFixtureShell>
  );
}

// ---------------------------------------------------------------------------
// Inline board / grid body components
//
// These mirror the shape of spatial-board-fixture.tsx and
// spatial-grid-fixture.tsx respectively, but without the outer
// `EntityFocusProvider` + `FixtureShell` — those are owned by the
// LeftNav fixture shell above.
// ---------------------------------------------------------------------------

import { FocusScope, useFocusScopeElementRef } from "@/components/focus-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { fieldMoniker, moniker as mk, ROW_SELECTOR_FIELD } from "@/lib/moniker";
import type { ReactNode } from "react";

const BOARD_COLUMNS = 3;
const BOARD_ROWS = 3;
const BOARD_COLUMN_WIDTH_PX = 111;

/** Column id in the fixture, 1-indexed. */
function fixtureColumnId(col: number): string {
  return `col-${col}`;
}

/** Card id in the fixture, 1-indexed. */
function fixtureCardId(col: number, row: number): string {
  return `card-${col}-${row}`;
}

/** Pre-computed board card monikers: `[col-1][row-1]`. */
export const FIXTURE_CARD_MONIKERS: ReadonlyArray<readonly string[]> =
  Array.from({ length: BOARD_COLUMNS }, (_, c) =>
    Array.from({ length: BOARD_ROWS }, (_, r) =>
      mk("task", fixtureCardId(c + 1, r + 1)),
    ),
  );

/** One kanban-style card rendered as a `FocusScope`. */
function FixtureCard({ col, row }: { col: number; row: number }) {
  const m = FIXTURE_CARD_MONIKERS[col - 1][row - 1];
  return (
    <FocusScope
      moniker={m}
      commands={[]}
      data-testid={`data-moniker:${m}`}
      style={{
        height: "60px",
        marginBottom: "8px",
        boxSizing: "border-box",
        padding: "8px",
        border: "1px solid #ccc",
        borderRadius: "4px",
        background: "white",
      }}
    >
      {m}
    </FocusScope>
  );
}

/** One column containing three cards, laid out vertically. */
function FixtureColumn({ col }: { col: number }) {
  const columnMoniker = mk("column", fixtureColumnId(col));
  return (
    <FocusScope
      moniker={columnMoniker}
      commands={[]}
      data-testid={`data-moniker:${columnMoniker}`}
      style={{
        display: "flex",
        flexDirection: "column",
        width: `${BOARD_COLUMN_WIDTH_PX}px`,
        marginRight: "16px",
        paddingTop: "8px",
        paddingBottom: "8px",
        background: "#f5f5f5",
        borderRadius: "6px",
      }}
    >
      {Array.from({ length: BOARD_ROWS }, (_, r) => (
        <FixtureCard key={r} col={col} row={r + 1} />
      ))}
    </FocusScope>
  );
}

/** 3x3 board body, sans provider/shell. */
function FixtureBoardBody() {
  return (
    <div
      data-testid="board-body"
      style={{
        display: "flex",
        flexDirection: "row",
        padding: "16px",
        alignItems: "flex-start",
        flex: 1,
      }}
    >
      {Array.from({ length: BOARD_COLUMNS }, (_, c) => (
        <FixtureColumn key={c} col={c + 1} />
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Grid body
// ---------------------------------------------------------------------------

const GRID_ROWS = 3;
const GRID_COLUMNS = ["tag_name", "color"] as const;

/** Pre-computed grid row monikers, 1-indexed. */
export const FIXTURE_ROW_MONIKERS: readonly string[] = Array.from(
  { length: GRID_ROWS },
  (_, i) => mk("tag", `tag-${i}`),
);

/** Pre-computed grid cell monikers (`[row][col]`). */
export const FIXTURE_CELL_MONIKERS: ReadonlyArray<readonly string[]> =
  FIXTURE_ROW_MONIKERS.map((_, r) =>
    GRID_COLUMNS.map((c) => fieldMoniker("tag", `tag-${r}`, c)),
  );

/** Pre-computed row-selector monikers, one per row. */
export const FIXTURE_ROW_SELECTOR_MONIKERS: readonly string[] =
  FIXTURE_ROW_MONIKERS.map((_, r) =>
    fieldMoniker("tag", `tag-${r}`, ROW_SELECTOR_FIELD),
  );

/** A `<div>` that plays the role of a grid cell — reads the scope elementRef. */
function FixtureCellDiv({
  dataMoniker,
  style,
  children,
}: {
  dataMoniker: string;
  style: React.CSSProperties;
  children: ReactNode;
}) {
  const elementRef = useFocusScopeElementRef();
  const { setFocus } = useEntityFocus();

  // `data-focused` is written by the enclosing `FocusScope`'s
  // `useFocusDecoration` hook onto this same element (via the forwarded
  // `elementRef`). The fixture no longer re-implements the
  // `useFocusedMoniker` compare dance — it mirrors production's
  // centralized decoration exactly.
  return (
    <div
      ref={elementRef as React.RefObject<HTMLDivElement>}
      data-moniker={dataMoniker}
      data-testid={`data-moniker:${dataMoniker}`}
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

/** One row in the tag grid. */
function FixtureRow({ rowIndex }: { rowIndex: number }) {
  const rowMoniker = FIXTURE_ROW_MONIKERS[rowIndex];
  const cellMonikers = FIXTURE_CELL_MONIKERS[rowIndex];
  const selectorMoniker = FIXTURE_ROW_SELECTOR_MONIKERS[rowIndex];

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
            {GRID_COLUMNS[col]} r{rowIndex}
          </FixtureCellDiv>
        </FocusScope>
      ))}
    </FocusScope>
  );
}

/** 3x3 tag-grid body, sans provider/shell. */
function FixtureGridBody() {
  return (
    <div
      data-testid="grid-body"
      style={{
        width: "400px",
        display: "flex",
        flexDirection: "column",
        flex: 1,
      }}
    >
      {Array.from({ length: GRID_ROWS }, (_, r) => (
        <FixtureRow key={r} rowIndex={r} />
      ))}
    </div>
  );
}
