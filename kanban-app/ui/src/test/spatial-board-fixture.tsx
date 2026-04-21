/**
 * Deterministic 3-column × 3-card board fixture for vitest-browser
 * spatial-nav tests.
 *
 * ## Purpose
 *
 * Mirrors the shape of the production board view — columns laid out
 * horizontally, cards stacked vertically within each column — without
 * dragging in the full `BoardView` machinery (schema, entity stores,
 * dnd-kit, drag session, perspectives). Tests render
 * `<AppWithBoardFixture />` once and drive h/j/k/l navigation through
 * real DOM clicks and keyboard events; the mocked `invoke()` routes
 * every spatial command into the `SpatialStateShim` via
 * `setupSpatialShim()`.
 *
 * ## Shape
 *
 * 3 columns × 3 cards per column, laid out as a flex row of columns,
 * each column a flex column of cards:
 *
 * ```
 *   column:col-1   column:col-2   column:col-3
 *   ┌──────────┐   ┌──────────┐   ┌──────────┐
 *   │ card-1-1 │   │ card-2-1 │   │ card-3-1 │
 *   │ card-1-2 │   │ card-2-2 │   │ card-3-2 │
 *   │ card-1-3 │   │ card-2-3 │   │ card-3-3 │
 *   └──────────┘   └──────────┘   └──────────┘
 * ```
 *
 * Monikers follow production conventions: `task:<id>` for cards,
 * `column:<id>` for columns. Both 1-indexed so test references like
 * `card-1-1` read naturally as "column 1, row 1".
 *
 * Each card and column is a `FocusScope` — matches production where
 * `EntityCard` and `ColumnView` both register scopes with the spatial
 * layer.
 *
 * ## What the fixture does NOT do
 *
 * No virtualization, no drag-and-drop, no entity context, no schema.
 * The spatial nav engine only needs rects + monikers; this fixture
 * provides exactly those, in the same flex layout production uses so
 * `getBoundingClientRect()` reports geometry that models a real board.
 */

import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { FocusScope, useFocusScopeElementRef } from "@/components/focus-scope";
import { moniker } from "@/lib/moniker";
import { FixtureShell } from "./spatial-fixture-shell";

/** Number of columns in the fixture. */
export const BOARD_COLUMNS = 3;

/** Number of cards per column in the fixture. */
const ROWS = 3;

// Alias retained for backwards compatibility inside this module.
const COLUMNS = BOARD_COLUMNS;

/**
 * Pixel width pinned on every fixture column.
 *
 * Tuned empirically so the column rect does not dominate adjacent card
 * rects in the spatial engine's `13 * major² + minor²` scoring — see
 * `FixtureColumn` below for why that matters. Tests depend on this value:
 * changing it without re-verifying the beam-test behaviour may flip
 * `h`/`l` cross-column nav from picking the adjacent card to picking the
 * adjacent column.
 *
 * The actual number (111px) is the width at which the three-column layout
 * fits inside the fixture viewport while keeping each column wide enough
 * for real card contents to render on a single line. The scoring contract
 * is what locks it in; the pixel count is the value that happens to
 * satisfy it.
 */
const COLUMN_WIDTH_PX = 111;

/**
 * Build the column id for a 1-indexed column number.
 *
 * Kept as a helper so tests share the same id convention as the fixture.
 */
export function fixtureColumnId(col: number): string {
  return `col-${col}`;
}

/**
 * Build the card id for 1-indexed (column, row) coordinates.
 *
 * Example: `fixtureCardId(1, 2)` → `"card-1-2"`. Tests reference card
 * ids directly in assertions like
 * `screen.getByTestId("data-moniker:task:card-1-2")`.
 */
export function fixtureCardId(col: number, row: number): string {
  return `card-${col}-${row}`;
}

/**
 * Build the tag id for a 1-indexed pill index on a given card.
 *
 * The shape `tag-<col>-<row>-<idx>` keeps tag ids unique across the whole
 * fixture so the spatial engine never confuses two pills that happen to
 * share the same column/row.
 */
export function fixtureTagId(col: number, row: number, idx: number): string {
  return `tag-${col}-${row}-${idx}`;
}

/**
 * Map of `(col, row) → tag pill count`, keyed by the card id.
 *
 * Only entries listed here render tag pills inside their card; cards
 * absent from the map render no pills (and therefore register no
 * sub-part spatial entries). Keeping the default board shape pill-free
 * preserves the original `spatial-nav-board.test.tsx` scenarios — the
 * card-subparts tests explicitly depend on `card-1-1` having exactly
 * two pills.
 */
const CARD_TAG_COUNTS: Readonly<Record<string, number>> = Object.freeze({
  [fixtureCardId(1, 1)]: 2,
});

/**
 * Number of tag pills to render inside a given card.
 *
 * Tests import this to compute pill counts without re-deriving them
 * from the private `CARD_TAG_COUNTS` map.
 */
export function fixtureTagCount(col: number, row: number): number {
  return CARD_TAG_COUNTS[fixtureCardId(col, row)] ?? 0;
}

/** Pre-computed column monikers, 1-indexed by column number. */
export const FIXTURE_COLUMN_MONIKERS: readonly string[] = Array.from(
  { length: COLUMNS },
  (_, i) => moniker("column", fixtureColumnId(i + 1)),
);

/**
 * Pre-computed card monikers as a 2D array indexed by `[col - 1][row - 1]`.
 *
 * `FIXTURE_CARD_MONIKERS[0][1]` is the moniker for `card-1-2` (column 1,
 * row 2). Tests use these handles for `getByTestId` and assertion
 * comparisons.
 */
export const FIXTURE_CARD_MONIKERS: ReadonlyArray<readonly string[]> =
  Array.from({ length: COLUMNS }, (_, c) =>
    Array.from({ length: ROWS }, (_, r) =>
      moniker("task", fixtureCardId(c + 1, r + 1)),
    ),
  );

/** Props for a single card inside a column. */
interface FixtureCardProps {
  col: number;
  row: number;
}

/**
 * One kanban-style card rendered as a `FocusScope`.
 *
 * Matches production: `EntityCard` registers a `FocusScope` keyed on
 * `entity.moniker` (e.g. `task:<id>`). The fixture card uses the same
 * moniker shape so the spatial layer indexes cards identically.
 *
 * Inline styles pin a deterministic size so `getBoundingClientRect()`
 * gives stable rects inside the headless browser regardless of CSS
 * resets.
 *
 * When `fixtureTagCount(col, row)` is non-zero, the card renders a row
 * of nested `FocusScope` tag pills. Each pill's `FocusScope` resolves
 * its `parent_scope` through `FocusScopeContext` to the enclosing card
 * moniker — the wiring under test in `spatial-nav-card-subparts.test.tsx`.
 */
function FixtureCard({ col, row }: FixtureCardProps) {
  const mk = FIXTURE_CARD_MONIKERS[col - 1][row - 1];
  const tagCount = fixtureTagCount(col, row);
  if (tagCount > 0) {
    return <FixtureCardWithPills col={col} row={row} tagCount={tagCount} />;
  }
  return (
    <FocusScope
      moniker={mk}
      commands={[]}
      data-testid={`data-moniker:${mk}`}
      style={{
        // Fixed height + marginBottom for vertical stacking with stable rects.
        height: "60px",
        marginBottom: "8px",
        // Stretches to the column's content width by default (flex child);
        // combined with the column's zero horizontal padding, card rect
        // and column rect share the same left/right edges.
        boxSizing: "border-box",
        padding: "8px",
        border: "1px solid #ccc",
        borderRadius: "4px",
        background: "white",
      }}
    >
      {mk}
    </FocusScope>
  );
}

/** Props for a card that renders pills beside a narrow body. */
interface FixtureCardWithPillsProps {
  col: number;
  row: number;
  tagCount: number;
}

/**
 * Card variant used when the card renders tag pills.
 *
 * The card scope uses `renderContainer={false}` so the card's spatial
 * rect is bounded by the title element (the body region), not the
 * full outer flex container. That keeps pills — rendered to the right
 * of the body inside the same flex row — geometrically outside the
 * card's beam-test rect, so `nav.right` from the card reaches the
 * first pill and `nav.left` from the first pill falls through to the
 * card body.
 *
 * Rendering pills as descendants of the card's `FocusScope` is what
 * makes this test meaningful: `useParentFocusScope()` inside each pill
 * resolves to the card moniker, and the `parent_scope` value the
 * implementation forwards to `spatial_register` is the card moniker.
 * Container-first search in the Rust engine keeps h/j/k/l amongst
 * pills on the same card before falling through to the full layer.
 */
function FixtureCardWithPills({
  col,
  row,
  tagCount,
}: FixtureCardWithPillsProps) {
  const mk = FIXTURE_CARD_MONIKERS[col - 1][row - 1];
  return (
    <FocusScope moniker={mk} commands={[]} renderContainer={false}>
      <div
        data-testid={`card-outer:${fixtureCardId(col, row)}`}
        style={{
          display: "flex",
          flexDirection: "row",
          alignItems: "center",
          gap: "4px",
          height: "60px",
          marginBottom: "8px",
          boxSizing: "border-box",
          padding: "4px",
          border: "1px solid #ccc",
          borderRadius: "4px",
          background: "white",
          // Prevent pills from wrapping to a new visual row; the fixture
          // relies on a single row of [body | pills] so the beam-test
          // geometry is deterministic.
          flexWrap: "nowrap",
          overflow: "visible",
        }}
      >
        <FixtureCardBody moniker={mk} />
        <FixtureTagRow col={col} row={row} count={tagCount} />
      </div>
    </FocusScope>
  );
}

/**
 * Narrow body region of a pill-bearing card.
 *
 * Registers its own DOM node as the enclosing `FocusScope`'s
 * spatial element via `useFocusScopeElementRef()`. Pinned to a small
 * width so the card's rect ends well before the pill row, giving
 * beam-test `l` from the card a clear path into the first pill.
 *
 * Wires a click handler that sets entity focus to the card moniker —
 * `renderContainer={false}` means the scope has no onClick of its
 * own, so the body element takes that responsibility. Mirrors the
 * `FixtureCellDiv` pattern in `spatial-grid-fixture.tsx`.
 */
function FixtureCardBody({ moniker: cardMoniker }: { moniker: string }) {
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
      data-testid={`data-moniker:${cardMoniker}`}
      data-moniker={cardMoniker}
      onClick={(e) => {
        e.stopPropagation();
        setFocus(cardMoniker);
      }}
      style={{
        // Narrow body — the card's spatial rect is sized to this element.
        // Keeps the pill row (rendered to the right) outside the card's
        // beam-test rect. Fixed width + height so `getBoundingClientRect()`
        // is deterministic regardless of text content.
        flex: "0 0 auto",
        width: "24px",
        height: "24px",
        fontSize: "9px",
        lineHeight: "24px",
        textAlign: "center",
        overflow: "hidden",
        whiteSpace: "nowrap",
        cursor: "pointer",
        background: "#fafafa",
        border: "1px solid #ddd",
        borderRadius: "3px",
      }}
    >
      {cardMoniker}
    </div>
  );
}

/** Props for the tag-pill row rendered inside a single card. */
interface FixtureTagRowProps {
  col: number;
  row: number;
  count: number;
}

/**
 * Horizontal row of tag-pill `FocusScope`s rendered inside a card.
 *
 * Each pill is its own spatial entry with a `tag:` moniker. Because
 * pills mount inside the card's `FocusScope`, `useParentFocusScope()`
 * (read by `FocusScope` itself) resolves to the enclosing card moniker
 * — that value is what the implementation under test threads through to
 * the Rust `spatial_register` call as `parent_scope`.
 */
function FixtureTagRow({ col, row, count }: FixtureTagRowProps) {
  return (
    <div
      data-testid={`tag-row:${fixtureCardId(col, row)}`}
      style={{
        display: "flex",
        flexDirection: "row",
        gap: "4px",
        flex: "0 0 auto",
      }}
    >
      {Array.from({ length: count }, (_, i) => {
        const pillMoniker = moniker("tag", fixtureTagId(col, row, i + 1));
        return (
          <FocusScope
            key={i}
            moniker={pillMoniker}
            commands={[]}
            data-testid={`data-moniker:${pillMoniker}`}
            style={{
              // Fixed-size pills so beam-test geometry is deterministic.
              // Content is clipped — the rect is what matters for the
              // spatial engine, not the visible text.
              display: "inline-block",
              flex: "0 0 auto",
              width: "24px",
              height: "20px",
              fontSize: "8px",
              lineHeight: "20px",
              textAlign: "center",
              overflow: "hidden",
              whiteSpace: "nowrap",
              border: "1px solid #888",
              borderRadius: "10px",
              background: "#eee",
            }}
          >
            {pillMoniker}
          </FocusScope>
        );
      })}
    </div>
  );
}

/**
 * One column containing three cards, laid out vertically.
 *
 * Matches production's `ColumnView` — the column is itself a
 * `FocusScope` (moniker `column:<id>`) and cards stack inside a flex
 * column. Width is pinned so columns sit side-by-side horizontally
 * without shrink-wrapping.
 *
 * The column's horizontal rect is sized to match the cards it holds
 * (no internal padding on the x-axis). Cards and the column share the
 * same left/right edges, so the spatial engine can't pick the column
 * over the next column's card just because the column's edge is
 * slightly closer — a single-pixel padding can tip the
 * `13 * major² + minor²` score in the wrong direction. Horizontal
 * spacing between columns is expressed via `marginRight`, which sits
 * outside the column's rect.
 */
export function FixtureColumn({ col }: { col: number }) {
  const columnMoniker = FIXTURE_COLUMN_MONIKERS[col - 1];
  return (
    <FocusScope
      moniker={columnMoniker}
      commands={[]}
      data-testid={`data-moniker:${columnMoniker}`}
      style={{
        display: "flex",
        flexDirection: "column",
        width: `${COLUMN_WIDTH_PX}px`,
        marginRight: "16px",
        paddingTop: "8px",
        paddingBottom: "8px",
        background: "#f5f5f5",
        borderRadius: "6px",
      }}
    >
      {Array.from({ length: ROWS }, (_, r) => (
        <FixtureCard key={r} col={col} row={r + 1} />
      ))}
    </FocusScope>
  );
}

/**
 * 3x3 kanban-board fixture ready for rendering in vitest-browser tests.
 *
 * Usage:
 * ```tsx
 * const { shim } = setupSpatialShim();
 * const screen = await render(<AppWithBoardFixture />);
 * const card = screen.getByTestId("data-moniker:task:card-1-1");
 * await userEvent.click(card);
 * await userEvent.keyboard("j");
 * ```
 *
 * All Tauri IPC goes through the shim — no real backend involvement.
 */
export function AppWithBoardFixture() {
  return (
    <EntityFocusProvider>
      <FixtureShell>
        <div
          data-testid="board-fixture-root"
          style={{
            display: "flex",
            flexDirection: "row",
            padding: "16px",
            alignItems: "flex-start",
          }}
        >
          {Array.from({ length: COLUMNS }, (_, c) => (
            <FixtureColumn key={c} col={c + 1} />
          ))}
        </div>
      </FixtureShell>
    </EntityFocusProvider>
  );
}
