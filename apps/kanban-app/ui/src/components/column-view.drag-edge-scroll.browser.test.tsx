/**
 * Browser-mode regression test for the drag-edge auto-scroll fight-the-user bug.
 *
 * Pins kanban task `01KRK6HR174QVN2TAH9AH4XZJB` — the second root cause.
 *
 * The bug:
 * `useColumnDragScroll` runs a `requestAnimationFrame` loop that calls
 * `scrollBy` every frame while the drag pointer is near the column's
 * top or bottom edge. `dragover` fires `start(-1)` or `start(1)` which
 * sets `dirRef.current` and kicks off the rAF loop. The loop is only
 * stopped from inside `handleDragOver` (when the pointer is in the
 * middle) or on component unmount.
 *
 * When the user drops a card with the pointer still near a column edge,
 * `dragover` stops firing but the rAF loop keeps running because
 * `dirRef.current` is still `-1` or `1`. Every subsequent frame calls
 * `scrollBy`, yanking the user's scroll back in whichever direction the
 * pointer was last near. That matches the user-visible behaviour: "after
 * dragging and dropping a card in a column, I can briefly scroll, but it
 * auto-scrolls back up."
 *
 * The fix: subscribe to the existing global `drag-ended` event (emitted
 * by `useTaskDragHandlers` in `board-view.tsx` after every task drag
 * completes) inside `useColumnDragScroll`, and call `stop()` on receipt.
 * Also stop on `dragleave` for the case where the pointer leaves the
 * column scroller without a drop.
 *
 * Test strategy: spy on `Element.prototype.scrollBy`. The rAF loop is
 * the only code path that calls `scrollBy` on the column scroller, so
 * any post-drop call is by definition the bug. Dispatch `dragover` near
 * the top edge to kick off the loop (assert `scrollBy` IS called),
 * dispatch `drag-ended` (the production "drag complete" signal), then
 * let several rAF frames pass and assert NO further `scrollBy` calls.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, mockEmit, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  const mockEmit = vi.fn(
    async (eventName: string, payload: unknown): Promise<void> => {
      const cbs = listeners.get(eventName) ?? [];
      // Dispatch synchronously so test code that awaits the emit observes
      // the resulting state changes deterministically.
      for (const cb of cbs) cb({ payload });
    },
  );
  return { mockInvoke, mockListen, mockEmit, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  emit: (...a: Parameters<typeof mockEmit>) => mockEmit(...a),
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

import "@/components/fields/registrations";
import { ColumnView } from "./column-view";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

const COLUMN_ID = "01ABCDEFGHJKMNPQRSTVWXYZ02";

function makeColumn(id: string = COLUMN_ID, name: string = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

function makeTask(index: number): Entity {
  const id = `01TASK${String(index).padStart(20, "0")}`;
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${index}`,
      position_column: COLUMN_ID,
      position_ordinal: `a${String(index).padStart(4, "0")}`,
    },
  };
}

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    fields: ["title"],
    sections: [{ id: "header", on_card: true }],
  },
  fields: [
    {
      id: "f-title",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
} as unknown as import("@/types/kanban").EntitySchema;

// ---------------------------------------------------------------------------
// Default invoke responses
// ---------------------------------------------------------------------------

async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    if (entityType === "task") return TASK_SCHEMA;
    return null;
  }
  if (cmd === "get_ui_state") {
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  }
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "show_context_menu") return undefined;
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

async function flushFrame() {
  await act(async () => {
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => resolve()),
    );
    await Promise.resolve();
  });
}

/** Find the inner column scroller — the element with `onDragOver` wired. */
function findColumnScroller(container: HTMLElement): HTMLElement {
  const candidates = Array.from(
    container.querySelectorAll("div"),
  ) as HTMLDivElement[];
  const found = candidates.find((el) =>
    el.className.includes("overflow-y-auto"),
  );
  if (!found) {
    throw new Error("expected to find an overflow-y-auto column scroller");
  }
  return found;
}

/**
 * Build a synthetic `DragEvent` with the fields `useColumnDragScroll`
 * inspects: `clientY`, `dataTransfer.types`, and `dataTransfer.dropEffect`.
 */
function makeDragEvent(type: string, clientY: number): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperty(event, "clientY", { value: clientY });
  Object.defineProperty(event, "dataTransfer", {
    value: {
      types: [] as string[],
      dropEffect: "move",
      setData: () => {},
      getData: () => "",
    },
  });
  return event;
}

// ---------------------------------------------------------------------------
// Render helper
// ---------------------------------------------------------------------------

function renderColumn(column: Entity, tasks: Entity[]) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: tasks }}>
              <FieldUpdateProvider>
                <UIStateProvider>
                  <TooltipProvider>
                    <ActiveBoardPathProvider value="/test/board">
                      <div
                        data-testid="board-shell"
                        style={{
                          height: "400px",
                          width: "400px",
                          overflowY: "auto",
                          overflowX: "hidden",
                        }}
                      >
                        <FocusScope moniker={asSegment("ui:board")}>
                          <ColumnView column={column} tasks={tasks} />
                        </FocusScope>
                      </div>
                    </ActiveBoardPathProvider>
                  </TooltipProvider>
                </UIStateProvider>
              </FieldUpdateProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<ColumnView> — edge drag auto-scroll stops on drop", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    mockEmit.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("rAF auto-scroll loop stops when the global drag-ended signal fires", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 35 }, (_, i) => makeTask(i));
    const { container, unmount } = renderColumn(column, tasks);
    await flushSetup();
    await flushFrame();

    const scroller = findColumnScroller(container);
    // The bug presents as `scrollBy` being called on the column
    // scroller every frame after a drop. Spy on the prototype so we
    // capture every call regardless of CSS layout details — the test
    // need not rely on `scrollTop` actually changing.
    const scrollBySpy = vi.spyOn(
      Element.prototype as Element & {
        scrollBy: (opts: ScrollToOptions) => void;
      },
      "scrollBy",
    );

    // Position the scroller's bounding rect at the top of the viewport
    // so `clientY=10` lands inside SCROLL_ZONE=40 of `rect.top`. This
    // is what `handleDragOver` reads — the actual layout of the
    // scroller is irrelevant to the bug.
    const rect = {
      top: 0,
      bottom: 400,
      left: 0,
      right: 400,
      width: 400,
      height: 400,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    } as DOMRect;
    scroller.getBoundingClientRect = () => rect;

    // Phase 1: kick off the rAF auto-scroll loop by firing `dragover`
    // events with the pointer in the top edge zone. After a couple of
    // frames the loop must have called `scrollBy` at least once.
    await act(async () => {
      scroller.dispatchEvent(makeDragEvent("dragover", 10));
    });
    await flushFrame();
    await flushFrame();
    await flushFrame();
    const callsAfterDragover = scrollBySpy.mock.calls.length;
    expect(callsAfterDragover).toBeGreaterThan(0);

    // Phase 2: drop. In production `useTaskDragHandlers` emits
    // `drag-ended` the moment a task drag completes. The fix
    // subscribes the scroll loop to this signal and stops the rAF
    // loop on receipt.
    await act(async () => {
      await mockEmit("drag-ended", {});
    });
    // Give the loop one frame to either stop (fix in place) or fire
    // one more `scrollBy` before settling (without the fix).
    await flushFrame();
    const callsRightAfterEmit = scrollBySpy.mock.calls.length;

    // Phase 3: let several frames pass. If the rAF loop is still
    // running, each frame will add another `scrollBy` call. With the
    // fix, no further calls should accumulate.
    await flushFrame();
    await flushFrame();
    await flushFrame();
    await flushFrame();
    await flushFrame();
    const callsAfterSettle = scrollBySpy.mock.calls.length;

    expect(callsAfterSettle).toBe(callsRightAfterEmit);

    scrollBySpy.mockRestore();
    unmount();
  });
});
