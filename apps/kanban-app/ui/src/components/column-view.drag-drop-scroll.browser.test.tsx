/**
 * Browser-mode regression test for the drag-and-drop scroll-fight bug.
 *
 * Pins kanban task `01KRK6HR174QVN2TAH9AH4XZJB`:
 * after a drop focuses a card mid-list, the user must be able to scroll
 * inside that column WITHOUT being yanked back to the focused card. The
 * regression mode is that the virtualizer unmounts/remounts the focused
 * card during scroll, the focus-scope's `useEffect` fires
 * `scrollIntoView` on every remount, and the scroller's `scrollTop` is
 * reset to the focused card's position.
 *
 * The fix elsewhere makes `scrollIntoView` fire only on real focus
 * transitions; this test programmatically simulates the post-drop +
 * user-scroll sequence and asserts the user's scroll position survives.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
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
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  emit: vi.fn(() => Promise.resolve()),
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
import {
  SpatialFocusProvider,
  useSpatialFocusActions,
  type SpatialFocusActions,
} from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";
import { useEffect } from "react";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/** Identity-stable column id used by every fixture in this file. */
const COLUMN_ID = "01ABCDEFGHJKMNPQRSTVWXYZ01";

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
  if (cmd === "spatial_focus") {
    // Emit the kernel-style focus-changed event so the EntityFocusProvider
    // bridge updates the focus store. The bridge then flips
    // `useOptionalIsDirectFocus(fq)` for the targeted scope.
    const a = (args ?? {}) as { fq?: FullyQualifiedMoniker };
    const fq = a.fq ?? null;
    if (fq) {
      const handlers = listeners.get("focus-changed") ?? [];
      queueMicrotask(() => {
        for (const h of handlers) {
          h({
            payload: {
              window_label: "main",
              prev_fq: null,
              next_fq: fq,
              next_segment: null,
            },
          });
        }
      });
    }
  }
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

/** Find the outer scroll container the test installed. */
function findOuterScroller(container: HTMLElement): HTMLElement {
  const node = container.querySelector(
    "[data-testid='board-shell']",
  ) as HTMLElement | null;
  if (!node) {
    throw new Error(
      "expected to find the test wrapper [data-testid='board-shell']",
    );
  }
  return node;
}

/**
 * Capture the live `SpatialFocusActions` into a ref so the test can
 * imperatively call `focus(fq)` — mirroring what the drop handler does
 * after a card lands in a column.
 */
function ActionsCapture({
  actionsRef,
}: {
  actionsRef: { current: SpatialFocusActions | null };
}) {
  const actions = useSpatialFocusActions();
  useEffect(() => {
    actionsRef.current = actions;
  }, [actions, actionsRef]);
  return null;
}

// ---------------------------------------------------------------------------
// Render helpers
// ---------------------------------------------------------------------------

function renderColumn(
  column: Entity,
  tasks: Entity[],
  actionsRef: { current: SpatialFocusActions | null },
) {
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
                      <ActionsCapture actionsRef={actionsRef} />
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

describe("<ColumnView> — drag-drop scroll does not fight the user", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    // The focus-scroll latch lives on `FocusStore`; each test mounts a
    // fresh `EntityFocusProvider` (and therefore a fresh store), so the
    // latch resets implicitly.
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("user scroll inside the column is not yanked back to the focused (dropped) card", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 35 }, (_, i) => makeTask(i));
    const actionsRef: { current: SpatialFocusActions | null } = {
      current: null,
    };
    const { container, unmount } = renderColumn(column, tasks, actionsRef);
    await flushSetup();
    await flushFrame();

    const scroller = findOuterScroller(container);
    expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);

    // Pick a card mid-list and focus it — exactly what the drop handler
    // does after a card lands in a column.
    const cards = Array.from(
      container.querySelectorAll("[data-segment^='task:']"),
    ) as HTMLElement[];
    expect(cards.length).toBeGreaterThan(0);
    const middleCard = cards[Math.floor(cards.length / 2)];
    const focusedFq = middleCard.getAttribute(
      "data-moniker",
    ) as FullyQualifiedMoniker;
    expect(focusedFq).toBeTruthy();

    const actions = actionsRef.current!;
    expect(actions).not.toBeNull();
    await act(async () => {
      await actions.focus(focusedFq);
    });
    await flushFrame();
    await flushFrame();

    // The post-drop scroll-into-view may have moved the scroller. That
    // initial scroll is correct; the bug is what happens NEXT when the
    // user scrolls and the virtualizer recycles the focused row.
    //
    // Simulate the user scrolling down by a measurable amount. Then
    // wait for any rAF-driven side-effects (virtualizer remount of the
    // focused row, focus-scope effect, etc.) to settle. The scroller's
    // scrollTop after settle must equal the user's set value — i.e.
    // the focus-scope effect must NOT have yanked it back.
    const USER_SCROLL = scroller.scrollTop + 200;
    await act(async () => {
      scroller.scrollTop = USER_SCROLL;
      scroller.dispatchEvent(new Event("scroll"));
    });
    await flushFrame();
    await flushFrame();
    await flushFrame();

    expect(scroller.scrollTop).toBe(USER_SCROLL);

    unmount();
  });
});
