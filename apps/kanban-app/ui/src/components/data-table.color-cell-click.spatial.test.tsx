/**
 * Color cell click contract — single-click opens picker, double-click does
 * not inspect.
 *
 * Source of truth for kanban task `01KQZ77F629SHHJD5X470NZGJ0`.
 *
 * # The bugs being pinned
 *
 * Two bugs in `GridCellFocusable` for cells whose field def declares
 * `editor: "color-palette"`:
 *
 *   1. **Single-click failed to open the Radix popover.** The cell wraps
 *      its editing-mode children in an inner `<div className={innerClassName}
 *      onClick={handleCellClick} onDoubleClick={enterEdit}>`. That inner
 *      wrapper sits between the `<FocusScope>` host and the
 *      `<ColorPaletteEditor>`'s `<PopoverTrigger>`, so a click on the
 *      swatch fired the wrapper's `onClick` (move-cursor side effect)
 *      first; the popover toggled in unexpected ways and ended up closed.
 *      Fix: when the field's editor is `color-palette`, render the
 *      editing-mode child directly inside the `<FocusScope>` — no inner
 *      wrapper, no extra `onClick` on the path between the trigger and
 *      its `<Popover>` root.
 *
 *   2. **Double-click on a color cell dispatched `ui.inspect`.** The
 *      surrounding `<EntityRow>` attaches `useInspectOnDoubleClick(rowMk)`
 *      to its `<tr>`. The cell's own `onDoubleClick` calls `enterEdit()`
 *      but does not stop propagation, so the gesture also reaches the
 *      row and fires inspect. Color is a leaf editor — double-clicking it
 *      should drill into editing, not pop the inspector for the row
 *      entity. Fix: when the field's editor is `color-palette`, the
 *      cell's `onDoubleClick` calls `enterEdit()` AND `e.stopPropagation()`
 *      so the row never sees the gesture.
 *
 * # Test design
 *
 * Mounts `<DataTable>` inside the production-shaped spatial-nav stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">` +
 * `<EntityFocusProvider>`) with one column whose field declares
 * `editor: "color-palette"`. A custom `renderEditor` returns the real
 * `<ColorPaletteEditor>` so a click on the cell's swatch exercises the
 * Radix popover end to end.
 *
 * The grid mock is mutable: `enterEdit()` flips `mode` from `"normal"` to
 * `"edit"` and forces a re-render so a single click on the cell
 * transitions the cell from non-editing → editing in one gesture, the
 * same way the production grid behaves once `enterEdit` is wired to fire
 * on a single click for color-palette fields.
 *
 * Mock pattern matches the existing `data-table.cell-click-region.spatial.test.tsx`
 * file — `vi.hoisted` `mockInvoke`, narrow `@tauri-apps/api/*` mocks. The
 * suite runs under the browser project (real Chromium via Playwright) so
 * Radix's pointer-event handling on the popover content fires for real.
 *
 * # Assertions
 *
 * 1. **Click test**: click on a color cell → cell enters edit mode and
 *    Radix renders a `[data-radix-popper-content-wrapper]` element (the
 *    popover content wrapper) somewhere in the document.
 *
 * 2. **No-inspect test**: double-click on a color cell → no
 *    `dispatch_command` IPC fires with `cmd === "ui.inspect"`. The
 *    `enterEdit()` call still happens (so the picker can open).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";
import { useState, useCallback } from "react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn(async (_cmd: string, _args?: unknown): Promise<unknown> => undefined),
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { DataTable, type DataTableColumn } from "./data-table";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { ColorPaletteEditor } from "@/components/fields/editors/color-palette-editor";
import { asSegment } from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";
import type { UseGridReturn, GridMode } from "@/hooks/use-grid";

// ---------------------------------------------------------------------------
// Fixtures — single color column so the cell at (0, 0) is a color cell.
// ---------------------------------------------------------------------------

const COLOR_FIELD: FieldDef = {
  id: "f-color",
  name: "color",
  type: { kind: "color" },
  section: "body",
  display: "color-swatch",
  editor: "color-palette",
};

const COLUMNS: DataTableColumn[] = [{ field: COLOR_FIELD }];

const ENTITIES: Entity[] = [
  {
    entity_type: "tag",
    id: "tag-a",
    moniker: "tag:tag-a",
    fields: { color: "ff0000" },
  },
];

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/**
 * Hook that yields a UseGridReturn whose `mode` flips to `"edit"` when
 * `enterEdit()` is called and back to `"normal"` when `exitEdit()` runs.
 *
 * The cursor is pinned at (0, 0) — the test fixtures only ever render one
 * data row and one column, so cell (0, 0) is the color cell under test.
 *
 * `enterEditSpy` lets the assertions count `enterEdit` calls without
 * coupling to internal `useState` calls.
 */
type EnterEditSpy = ReturnType<typeof vi.fn<() => void>>;

function useStatefulGrid(enterEditSpy: EnterEditSpy): UseGridReturn {
  const [mode, setMode] = useState<GridMode>("normal");
  const enterEdit = useCallback(() => {
    enterEditSpy();
    setMode("edit");
  }, [enterEditSpy]);
  const exitEdit = useCallback(() => setMode("normal"), []);
  return {
    cursor: { row: 0, col: 0 },
    mode,
    enterEdit,
    exitEdit,
    enterVisual: () => {},
    exitVisual: () => {},
    selection: null,
    setCursor: () => {},
    expandSelection: () => {},
    getSelectedRange: () => null,
  };
}

/**
 * Test harness: mounts `<DataTable>` inside the spatial-nav provider
 * stack with a stateful grid mock. The custom `renderEditor` returns
 * the real `<ColorPaletteEditor>` so click → popover wires end to end.
 *
 * `enterEditSpy` is a vitest `vi.fn` the caller passes in; the harness
 * forwards every `enterEdit` call to it.
 */
function Harness({ enterEditSpy }: { enterEditSpy: EnterEditSpy }) {
  const grid = useStatefulGrid(enterEditSpy);
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <DataTable
            columns={COLUMNS}
            rows={ENTITIES}
            grid={grid}
            showRowSelector={false}
            renderEditor={(_entity, field, onCommit, onCancel) => (
              <ColorPaletteEditor
                field={field}
                mode="compact"
                value="ff0000"
                onCommit={onCommit}
                onCancel={onCancel}
              />
            )}
          />
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

/** Mount the harness, flushing initial spatial-nav register effects. */
async function renderHarness() {
  const enterEditSpy: EnterEditSpy = vi.fn();
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(<Harness enterEditSpy={enterEditSpy} />);
  });
  return { ...result, enterEditSpy };
}

/** Resolve the color cell focusable (the `<FocusScope>`'s rendered `<div>`). */
function findColorCellFocusable(container: HTMLElement): HTMLElement {
  const node = container.querySelector(
    `[data-segment="grid_cell:0:color"]`,
  ) as HTMLElement | null;
  if (!node) throw new Error("color cell focusable not found");
  return node;
}

/**
 * Resolve the click target inside the cell — the deepest visible
 * descendant the user actually clicks.
 *
 * In display mode the cell renders the inner click wrapper around the
 * field display (`<ColorSwatchDisplay>`); the swatch is a descendant of
 * that wrapper. Clicking the swatch bubbles through the wrapper to the
 * `<FocusScope>` host. We pick the deepest leaf so the bubble path
 * matches what a user gesture would do — clicking the focusable host
 * directly skips intermediate `onClick` handlers attached to
 * descendants.
 */
function findColorCellClickTarget(container: HTMLElement): HTMLElement {
  const focusable = findColorCellFocusable(container);
  let cursor: HTMLElement = focusable;
  while (cursor.firstElementChild) {
    cursor = cursor.firstElementChild as HTMLElement;
  }
  return cursor;
}

/** Collect every `dispatch_command` IPC call's arg bag. */
function dispatchCommandCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DataTable color cell — click opens picker, dblclick does not inspect", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("single click on a color cell enters edit mode and opens the Radix popover", async () => {
    const { container, baseElement, enterEditSpy } = await renderHarness();
    const target = findColorCellClickTarget(container);

    await act(async () => {
      fireEvent.click(target);
    });

    // The cell entered edit mode in response to the click — the leaf
    // contract for a leaf editor (color is a leaf) is that a click
    // drills in.
    expect(enterEditSpy).toHaveBeenCalled();

    // Radix renders the popover content into a portal — find it on the
    // document root, not just inside `container`. The presence of any
    // `[data-radix-popper-content-wrapper]` element confirms the popover
    // mounted and is open.
    await waitFor(() => {
      const popper = baseElement.querySelector(
        "[data-radix-popper-content-wrapper]",
      );
      expect(popper).not.toBeNull();
    });
  });

  it("double click on a color cell opens the picker and does NOT dispatch ui.inspect", async () => {
    const { container, baseElement, enterEditSpy } = await renderHarness();
    const target = findColorCellClickTarget(container);

    // Reset invoke before the gesture so we measure only the
    // double-click's IPC.
    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.doubleClick(target);
    });

    // The double-click drills into editing — same as a single click.
    expect(enterEditSpy).toHaveBeenCalled();

    // Radix popover content mounted.
    await waitFor(() => {
      const popper = baseElement.querySelector(
        "[data-radix-popper-content-wrapper]",
      );
      expect(popper).not.toBeNull();
    });

    // CRITICAL: no `ui.inspect` dispatch fired. Color is a leaf editor —
    // double-click drills in, it must NOT pop the inspector for the row
    // entity. Both the structured `dispatch_command` shape and a defensive
    // string match are checked so a future rename of the IPC name does
    // not silently let a regression through.
    const inspectCalls = dispatchCommandCalls().filter(
      (c) => c.cmd === "ui.inspect",
    );
    expect(inspectCalls).toHaveLength(0);

    const anyInspect = mockInvoke.mock.calls.find((c) => {
      const cmd = typeof c[0] === "string" ? c[0] : "";
      const payload = (c[1] as { cmd?: string } | undefined)?.cmd ?? "";
      return /inspect/i.test(cmd) || /inspect/i.test(payload);
    });
    expect(anyInspect).toBeUndefined();
  });
});
