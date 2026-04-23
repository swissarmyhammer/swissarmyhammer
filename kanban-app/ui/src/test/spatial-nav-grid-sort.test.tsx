/**
 * Grid column-header Enter-toggles-sort contract
 * (task 01KPXQR7CM8GEQAEM5HB8X130B).
 *
 * ## What this pins
 *
 * In the grid (data-table) view, pressing **Enter** on a focused column
 * header must toggle sort on that column — identical to clicking the
 * header with the mouse. Mouse-click sort works in the live app; the
 * user report is that **keyboard Enter does nothing**.
 *
 * ## Why the previous round of tests missed the bug
 *
 * The first pass mounted a `FocusScope`+`<th>` fixture and drove focus
 * with `userEvent.click(header)`. That click fires the `<th>`'s
 * capture-phase `onClickCapture` → `setFocus(headerMoniker)`
 * synchronously. The React state update happens *before* the bubble
 * phase dispatches sort. Enter then fires against a focused scope, and
 * the assertion is satisfied.
 *
 * But a real user navigates by keyboard: `ArrowUp`/`k` dispatches
 * `nav.up` to Rust, Rust picks the header's key and emits
 * `focus-changed`, and the frontend listener in
 * `useFocusChangedEffect` calls `setFocusedMoniker(newMoniker)`. The
 * `onClickCapture` path is never exercised. **That** is the path the
 * user report covers.
 *
 * ## What this file tests
 *
 * Two cases, both mounted against the **real** production
 * `DataTableHeader` (no fixture replica — the reviewer called that out
 * as dead weight that doesn't catch regressions in the real binding):
 *
 * - **Case A** — keyboard-only focus acquisition. Focus starts on a
 *   sibling body-cell `FocusScope`. A scripted `focus-changed` event
 *   moves focus to the header without any click — mirroring what Rust
 *   does in response to `nav.up`. Enter is pressed and the test asserts
 *   `perspective.sort.toggle` was dispatched exactly once **and the
 *   dispatched scope chain includes the header's own moniker**.
 * - **Case B** — real `DataTableHeader` + click-to-focus baseline, with
 *   the grid-level Enter bindings stacked as parent scope. Pins the
 *   scope-chain shadow resolution contract: the header scope must win
 *   over the parent `grid.edit`/`grid.editEnter` bindings. The post-
 *   Enter assertion also requires the header moniker in the dispatched
 *   chain.
 *
 * ## Red-on-regression
 *
 * Two independent axes pin the production binding:
 *
 * 1. **Binding presence** — if the production `column-header.sort.*`
 *    CommandDef is removed from `data-table.tsx`, both cases go red
 *    (Enter never dispatches). Verified by temporarily deleting the
 *    production `commands` array.
 * 2. **Stale-closure regression** — if the production binding reverts
 *    to closing over the render-time `handleClick` (instead of reading
 *    `handleClickRef.current` at execute time), both cases go red on
 *    the "chain must contain header moniker" assertion. Verified by
 *    temporarily reverting the `handleClickRef` indirection in
 *    `HeaderCell`.
 *
 * The two assertions together catch both classes of regression the
 * reviewer called out in round-1 review — binding deletion (blocker 3)
 * and silent identity-churn where the execute closure decouples from
 * the freshest `dispatchSortToggle` (task hypothesis 4).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";
import {
  useContext,
  useEffect,
  useRef,
  useCallback,
  useMemo,
  type ReactNode,
} from "react";

vi.mock("@tauri-apps/api/core", async () => {
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  return tauriCoreMock();
});
vi.mock("@tauri-apps/api/event", async () => {
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return tauriEventMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import {
  EntityFocusProvider,
  useFocusedScope,
} from "@/lib/entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import {
  CommandScopeContext,
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import {
  createKeyHandler,
  extractScopeBindings,
  type KeymapMode,
} from "@/lib/keybindings";
import { columnHeaderMoniker, fieldMoniker } from "@/lib/moniker";
import { DataTableHeader_forTestingOnly as DataTableHeader } from "@/components/data-table";
import {
  useReactTable,
  getCoreRowModel,
  type ColumnDef,
} from "@tanstack/react-table";
import type { Entity } from "@/types/kanban";

/**
 * Inline replica of `KeybindingHandler` — identical to the one in
 * `spatial-nav-column-drill.test.tsx`. Attaches the production
 * `createKeyHandler` to `document` via the production
 * `extractScopeBindings` lookup, so Enter resolves through the focused
 * scope's command chain exactly as it would in the live app.
 *
 * This mirrors `AppShell.KeybindingHandler` verbatim — if the
 * production shape ever diverges from what this replica assumes, we
 * want the test suite to fail loudly, not silently pass with a stale
 * replica. The inline copy is the same pattern used by
 * `spatial-nav-column-drill.test.tsx` and `spatial-fixture-shell.tsx`;
 * a shared helper is a follow-up refactor flagged in the reviewer's
 * nits for that whole family of tests.
 */
function KeybindingHandler({ mode }: { mode: KeymapMode }) {
  const dispatch = useDispatchCommand();
  const focusedScope = useFocusedScope();
  const treeScope = useContext(CommandScopeContext);

  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;
  const focusedScopeRef = useRef(focusedScope);
  focusedScopeRef.current = focusedScope;
  const treeScopeRef = useRef(treeScope);
  treeScopeRef.current = treeScope;

  const executeCommand = useCallback(async (id: string): Promise<boolean> => {
    await dispatchRef.current(id);
    return true;
  }, []);

  useEffect(() => {
    const handler = createKeyHandler(mode, executeCommand, () =>
      extractScopeBindings(
        focusedScopeRef.current ?? treeScopeRef.current,
        mode,
      ),
    );
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [mode, executeCommand]);

  return null;
}

// ---------------------------------------------------------------------------
// Topology wrapper shared by all cases.
// ---------------------------------------------------------------------------

/**
 * Navigation commands — verbatim copies of `NAV_COMMAND_DEFS` from
 * `app-shell.tsx`. Registered at the root scope so keyboard nav keys
 * (`ArrowUp`/`k`, `ArrowDown`/`j`, etc.) resolve to the matching
 * `nav.*` command id. These commands carry no local `execute` — the
 * dispatcher routes each nav keypress through `dispatch_command` to
 * the (stubbed) Rust nav handler, exactly like production.
 *
 * Without this registration, `userEvent.keyboard("{ArrowUp}")` lands
 * on no binding and silently does nothing, making a "keyboard path"
 * test vacuously pass.
 */
const NAV_COMMANDS: CommandDef[] = [
  { id: "nav.up", name: "Up", keys: { vim: "k", cua: "ArrowUp" } },
  { id: "nav.down", name: "Down", keys: { vim: "j", cua: "ArrowDown" } },
  { id: "nav.left", name: "Left", keys: { vim: "h", cua: "ArrowLeft" } },
  { id: "nav.right", name: "Right", keys: { vim: "l", cua: "ArrowRight" } },
];

/** Wrap a component in the minimum tree needed for keybindings to flow. */
function AppShell({ children }: { children: ReactNode }) {
  return (
    <EntityFocusProvider>
      <FocusLayer name="window">
        <CommandScopeProvider commands={NAV_COMMANDS}>
          <KeybindingHandler mode="cua" />
          {children}
        </CommandScopeProvider>
      </FocusLayer>
    </EntityFocusProvider>
  );
}

// ---------------------------------------------------------------------------
// Real-DataTableHeader slice — exercises production HeaderCell wiring.
// ---------------------------------------------------------------------------

/**
 * Minimal real-DataTable slice: set up a TanStack table with one
 * column, render the **actual** `DataTableHeader` from production, and
 * feed it the same dispatch hook `DataTable` would feed it.
 *
 * We render `DataTableHeader` directly (rather than the full
 * `<DataTable>`) because `DataTable`'s body renders `<Field>` for each
 * cell, which needs entity-store context and a registered display
 * component. The header path is entirely self-contained: column defs +
 * dispatch hook + perspective map. This slice exercises exactly the
 * `HeaderCell` + `HeaderCellTh` wiring that diverges from any hand-
 * copied fixture.
 *
 * `DataTableHeader` is exported from `data-table.tsx` purely as a
 * testing seam — see its JSDoc there. The seam lets these tests reach
 * the production header without standing up the full `<DataTable>`
 * body; production callers still use `<DataTable>`.
 */
function RealDataTableHeaderSlice({
  columnId,
  perspectiveId,
}: {
  columnId: string;
  perspectiveId: string;
}) {
  const dispatchSortToggle = useDispatchCommand("perspective.sort.toggle");
  const columns = useMemo<ColumnDef<Entity>[]>(
    () => [
      {
        id: columnId,
        accessorFn: (row: Entity) => row.fields[columnId],
        header: columnId,
      },
    ],
    [columnId],
  );
  // `table.getHeaderGroups()` walks `columnDef.header`/`id` — no row
  // data needed for header rendering. Empty row array is valid for
  // TanStack.
  const table = useReactTable<Entity>({
    data: [],
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <table>
      <DataTableHeader
        table={table}
        showRowSelector={false}
        perspectiveSortMap={new Map()}
        perspectiveId={perspectiveId}
        dispatchSortToggle={dispatchSortToggle}
      />
    </table>
  );
}

// ---------------------------------------------------------------------------
// Sibling body-cell fixture — gives Case A a keyboard-accessible starting
// focus target that is *not* the header.
// ---------------------------------------------------------------------------

/**
 * A minimal body-cell `FocusScope` using the same shape the real
 * `DataTableCell` uses: `renderContainer={false}` with the scope's
 * `elementRef` attached to a `<td>` via `useFocusScopeElementRef()`.
 * Rendered inside a separate `<table>` alongside the header slice so
 * it sits in the same layer tree but is not a child of the header
 * scope.
 *
 * The only purpose is to give Case A a real registered scope to start
 * focus on, so the `focus-changed` event it fires later can flip focus
 * *to* the header from a sibling scope — mirroring the live-app
 * transition where a user arrows up from a data cell to the header.
 */
function BodyCellFixture({ cellMoniker }: { cellMoniker: string }) {
  return (
    <FocusScope moniker={cellMoniker} commands={[]}>
      <div data-testid={`body-cell-${cellMoniker}`}>body</div>
    </FocusScope>
  );
}

// ---------------------------------------------------------------------------
// Grid-level parent commands — mirror GRID_EDIT_DESCRIPTORS in grid-view.tsx.
// ---------------------------------------------------------------------------

/**
 * Enter-binding commands at the grid scope — verbatim copies of the
 * entries the real `grid-view.tsx` registers via
 * `GRID_EDIT_DESCRIPTORS`. These compete with the header scope's Enter
 * binding during scope-chain resolution. If a regression ever shadowed
 * the header's `column-header.sort.*` with one of these, the test below
 * would flip red and pin the shadow.
 *
 * `grid.edit` (cua Enter) and `grid.editEnter` (vim Enter) are the two
 * that actually bind Enter in production — the rest of the edit
 * descriptors bind other keys (`i`, `Mod+Enter`, etc.) and are omitted
 * here because they add nothing to the shadow test.
 *
 * The ids/keys are intentionally duplicated from production. If
 * grid-view ever renames one of those command IDs, the shadow test
 * silently stops matching production. Reviewer flagged this as a nit —
 * a follow-up refactor could export `GRID_EDIT_DESCRIPTORS` from
 * `grid-view.tsx` (with the same "testing seam" caveat as
 * `DataTableHeader`). Not worth blocking on here.
 */
const GRID_PARENT_ENTER_COMMANDS: CommandDef[] = [
  {
    id: "grid.edit",
    name: "Edit Cell",
    keys: { vim: "i", cua: "Enter" },
    execute: () => {
      /* no-op — the binding's presence is what matters for shadow tests */
    },
  },
  {
    id: "grid.editEnter",
    name: "Edit Cell (Enter)",
    keys: { vim: "Enter" },
    execute: () => {
      /* no-op */
    },
  },
];

// ---------------------------------------------------------------------------
// Test suite.
// ---------------------------------------------------------------------------

const PERSPECTIVE_ID = "my-view";
const COLUMN_ID = "title";
const BODY_CELL_MONIKER = fieldMoniker("tag", "tag-0", COLUMN_ID);

describe("Grid column-header Enter toggles sort (01KPXQR7CM8GEQAEM5HB8X130B)", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * Case A — full keyboard path, starting from a body cell.
   *
   * Mimics the live-app scenario the user reports as verbatim as
   * possible while staying inside the Tauri-boundary stub:
   *
   * 1. Focus starts on a body cell (via click, as the initial-focus
   *    hook would do on mount).
   * 2. The user presses `ArrowUp` (cua `nav.up`). The CUA
   *    `KeybindingHandler` routes it through `dispatch_command:nav.up`
   *    to the stub.
   * 3. The stub's scripted handler returns a `focus-changed` payload
   *    that moves focus from the body cell to the header — mirroring
   *    what Rust's `SpatialState::navigate` computes against real
   *    rects. The frontend's `useFocusChangedEffect` listener updates
   *    the focus store.
   * 4. The user presses `Enter`. The header's
   *    `column-header.sort.<id>` binding must fire and dispatch
   *    `perspective.sort.toggle`.
   *
   * The `onClickCapture` path on the `<th>` is never exercised — focus
   * arrives via the `focus-changed` event, which is the live keyboard
   * path. If this test goes red against current HEAD, we've reproduced
   * the user's bug in an automated shape.
   */
  it("Case A: keyboard ArrowUp from body cell → Enter on header dispatches sort", async () => {
    const screen = await render(
      <AppShell>
        <CommandScopeProvider
          commands={[]}
          moniker={`perspective:${PERSPECTIVE_ID}`}
        >
          <CommandScopeProvider commands={GRID_PARENT_ENTER_COMMANDS}>
            <RealDataTableHeaderSlice
              columnId={COLUMN_ID}
              perspectiveId={PERSPECTIVE_ID}
            />
            <BodyCellFixture cellMoniker={BODY_CELL_MONIKER} />
          </CommandScopeProvider>
        </CommandScopeProvider>
      </AppShell>,
    );

    // Seed focus on the body cell — the starting point the live app
    // would be in after `useGridInitialFocus` places focus on the
    // first cell. Click is fine here because we only care that *focus
    // moves to the header* happens via `focus-changed`, not a click.
    const bodyCell = screen.getByTestId(`body-cell-${BODY_CELL_MONIKER}`);
    await userEvent.click(bodyCell.element());
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));

    // Snapshot dispatch count before the keyboard path runs. No
    // production code should have dispatched sort yet.
    const sortBefore = handles
      .dispatchedCommands()
      .filter((d) => d.cmd === "perspective.sort.toggle").length;
    expect(sortBefore).toBe(0);

    // Script Rust's response to `nav.up`: when the frontend dispatches
    // `nav.up`, the backend emits `focus-changed` moving focus from
    // BODY_CELL_MONIKER to the header. This mirrors what
    // `SpatialState::navigate` would compute against real rects.
    const headerMoniker = columnHeaderMoniker(COLUMN_ID);
    handles.scriptResponse("dispatch_command:nav.up", () =>
      handles.payloadForFocusMove(BODY_CELL_MONIKER, headerMoniker),
    );

    // Press ArrowUp — CUA binding for `nav.up`. The key handler
    // dispatches through the React dispatch chain to
    // `dispatch_command:nav.up`, which the stub handles by emitting
    // `focus-changed` for the header. The frontend listener updates
    // the focus store.
    await userEvent.keyboard("{ArrowUp}");

    // Wait for the focus store update to propagate — the
    // `EntityFocusProvider` re-renders via `useSyncExternalStore` and
    // re-publishes `FocusedScopeContext`, and the `KeybindingHandler`
    // re-renders to update `focusedScopeRef.current` to the header's
    // scope.
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));

    // Sanity-check: after the `nav.up` round-trip, the stub reports
    // focus on the header. Catches the class of bug where the focus
    // never arrived on the header in the first place (then the Enter
    // assertion would fail for the wrong reason).
    expect(handles.focusedMoniker()).toBe(headerMoniker);

    // Press Enter. The header is now the focused scope; the binding
    // `column-header.sort.<id>` must resolve and its execute must
    // dispatch `perspective.sort.toggle`.
    await userEvent.keyboard("{Enter}");

    // Enter must produce exactly one dispatch. The cleaner single-
    // count assertion (not `toBeGreaterThan(sortBefore)`) catches
    // silent-regression shapes where both paths disappear:
    // `baseline=0, after=0` would pass a `>` check with a zero baseline.
    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .filter((d) => d.cmd === "perspective.sort.toggle").length,
        { timeout: 500 },
      )
      .toBe(1);

    const sortCalls = handles
      .dispatchedCommands()
      .filter((d) => d.cmd === "perspective.sort.toggle");
    expect(sortCalls[0].args).toEqual({
      field: COLUMN_ID,
      perspective_id: PERSPECTIVE_ID,
    });

    // The dispatch must carry `perspective:<id>` in its scope chain so
    // the backend's `ToggleSortCmd.available(ctx)` returns true. Without
    // it, the dispatch reaches Rust but is silently rejected as "not
    // available" — which would be the exact live-app symptom (no sort
    // indicator change) the user reports. This is the live-app
    // regression anchor: the test's view of "did the dispatch fire?"
    // is not enough on its own to guarantee the backend actually
    // executes it.
    expect(sortCalls[0].scopeChain).toContain(`perspective:${PERSPECTIVE_ID}`);

    // The dispatched scope chain must also include the header's
    // moniker. This pins the stale-closure fix in `HeaderCell`:
    // without the `handleClickRef` indirection, the scope chain at
    // Enter dispatch time reflects whatever focus was BEFORE the
    // user arrowed up to the header — here, the body cell. With
    // the fix, every call to the Enter binding reads the freshest
    // `handleClick`, which in turn reads the freshest
    // `dispatchSortToggle` built while the header itself was focused.
    expect(sortCalls[0].scopeChain).toContain(columnHeaderMoniker(COLUMN_ID));
  });

  /**
   * Case B — real DataTableHeader slice, click-driven focus, with the
   * grid-level parent Enter bindings stacked as parent scope.
   *
   * Pins the scope-chain shadow-resolution contract: the header
   * scope's `column-header.sort.<id>` must win over the parent grid
   * scope's `grid.edit` / `grid.editEnter` Enter bindings. If the
   * shadow logic ever regressed (e.g. a future parent binding accidentally
   * shadows the header's binding), this test would go red.
   *
   * Uses the cleaner single-call assertion shape: after a baseline
   * click dispatches once, Enter must push the count to exactly 2.
   * Reviewer warning 3 noted the previous `toBeGreaterThan(sortBefore)`
   * pattern could silently mask a regression where *both* paths
   * disappear. Counting to an exact post-Enter value closes that hole.
   */
  it("Case B: real DataTableHeader slice — Enter on click-focused header dispatches sort (scope-chain shadow)", async () => {
    const screen = await render(
      <AppShell>
        <CommandScopeProvider
          commands={[]}
          moniker={`perspective:${PERSPECTIVE_ID}`}
        >
          <CommandScopeProvider commands={GRID_PARENT_ENTER_COMMANDS}>
            <RealDataTableHeaderSlice
              columnId={COLUMN_ID}
              perspectiveId={PERSPECTIVE_ID}
            />
          </CommandScopeProvider>
        </CommandScopeProvider>
      </AppShell>,
    );

    const header = screen.getByTestId(`column-header-${COLUMN_ID}`);

    // Click the header once. The `<th>`'s `onClick` dispatches sort
    // via the bubble phase, and `onClickCapture` sets focus. The
    // click's sort dispatch is the baseline — subsequent Enter must
    // produce one more dispatch, landing at exactly 2.
    await userEvent.click(header.element());
    await new Promise((r) => requestAnimationFrame(() => r(undefined)));

    const sortAfterClick = handles
      .dispatchedCommands()
      .filter((d) => d.cmd === "perspective.sort.toggle").length;
    expect(sortAfterClick).toBe(1);

    // Press Enter. The header scope's binding must fire and shadow
    // the parent's `grid.edit` / `grid.editEnter` Enter bindings.
    await userEvent.keyboard("{Enter}");

    await expect
      .poll(
        () =>
          handles
            .dispatchedCommands()
            .filter((d) => d.cmd === "perspective.sort.toggle").length,
        { timeout: 500 },
      )
      .toBe(2);

    const sortCalls = handles
      .dispatchedCommands()
      .filter((d) => d.cmd === "perspective.sort.toggle");
    expect(sortCalls[sortCalls.length - 1].args).toEqual({
      field: COLUMN_ID,
      perspective_id: PERSPECTIVE_ID,
    });

    // Both dispatches must carry `perspective:<id>` in the scope chain.
    // The backend's `ToggleSortCmd.available(ctx)` requires a
    // `perspective:*` moniker in the chain to actually execute the
    // sort; otherwise the dispatch is rejected as "not available" with
    // no user-visible effect. See
    // `perspective_commands.rs::ToggleSortCmd::available`.
    expect(sortCalls[0].scopeChain).toContain(`perspective:${PERSPECTIVE_ID}`);
    expect(sortCalls[1].scopeChain).toContain(`perspective:${PERSPECTIVE_ID}`);

    // The Enter dispatch (the second sort call) must also carry the
    // header's own moniker. Pins the stale-closure fix: without the
    // `handleClickRef` indirection in `HeaderCell`, the Enter binding's
    // execute would close over a `dispatchSortToggle` captured before
    // the click-driven focus change, and its chain would be rooted at
    // whatever scope was focused before the click (or the tree scope)
    // instead of the header. With the fix, the Enter dispatch's chain
    // reflects the header-focused state the user was in when they
    // pressed Enter — same as the click path.
    expect(sortCalls[1].scopeChain).toContain(columnHeaderMoniker(COLUMN_ID));
  });
});
