/**
 * Spatial-nav test: Enter on the focused `ui:column.add-task:{columnId}`
 * leaf invokes `onAddTask(columnId)` exactly once and seeds focus to
 * the column FQM — matching today's pointer-click behavior.
 *
 * Source of truth for the column-view half of the reopen-scope
 * expansion on card `01KQM9BGN0HFQSC168YD9G82Z2`. Pre-migration, the
 * "+" Add-task button in column headers was a bare `<button>` with no
 * `<FocusScope>` — keyboard users could not focus it at all. The
 * Pressable migration adds keyboard reachability AND Enter / Space
 * activation that did not exist before.
 *
 * Both paths must produce the same effect:
 *
 *   1. Keyboard: focus `ui:column.add-task:{columnId}`, press Enter.
 *   2. Pointer: click the rendered "+" button.
 *
 * Both must call `onAddTask(columnId)` exactly once and `setFocus(columnFq)`
 * (via the spatial-focus context's `setFocus`) exactly once with the
 * column's FQM.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports. Mirrors
// `app-shell.test.tsx`'s kernel simulator so `spatial_focus` emits
// `focus-changed` and the entity-focus bridge populates the focused
// scope.
// ---------------------------------------------------------------------------

const monikerToKey = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };
const listenCallbacks: Record<string, (event: unknown) => void> = {};

const COLUMN_SCHEMA = {
  entity: {
    name: "column",
    fields: ["name"],
    commands: [],
  },
  fields: [
    {
      id: "name",
      name: "name",
      type: { kind: "text" },
      section: "header",
      display: "text",
      editor: "text",
    },
  ],
};

function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
  if (cmd === "list_entity_types")
    return Promise.resolve(["column", "task"]);
  if (cmd === "get_entity_schema") return Promise.resolve(COLUMN_SCHEMA);
  if (cmd === "list_commands_for_scope") return Promise.resolve([]);
  if (cmd === "get_undo_state")
    return Promise.resolve({ can_undo: false, can_redo: false });
  if (cmd === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_zone") {
    const a = (args ?? {}) as { fq?: string; segment?: string };
    if (a.fq && a.segment) monikerToKey.set(a.segment, a.fq);
    return Promise.resolve(null);
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { fq?: string };
    if (a.fq) {
      for (const [m, k] of monikerToKey.entries()) {
        if (k === a.fq) {
          monikerToKey.delete(m);
          break;
        }
      }
    }
    return Promise.resolve(null);
  }
  if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
    const a = (args ?? {}) as { focusedFq?: string };
    return Promise.resolve(a.focusedFq ?? null);
  }
  if (cmd === "spatial_focus") {
    const a = (args ?? {}) as { fq?: string };
    const fq = a.fq ?? null;
    let moniker: string | null = null;
    for (const [s, k] of monikerToKey.entries()) {
      if (k === fq) {
        moniker = s;
        break;
      }
    }
    if (fq) {
      const prev = currentFocusKey.key;
      currentFocusKey.key = fq;
      queueMicrotask(() => {
        const cb = listenCallbacks["focus-changed"];
        if (cb) {
          cb({
            payload: {
              window_label: "main",
              prev_fq: prev,
              next_fq: fq,
              next_segment: moniker,
            },
          });
        }
      });
    }
    return Promise.resolve(null);
  }
  return Promise.resolve(null);
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: unknown) => defaultInvoke(cmd, args)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn((eventName: string, cb: (event: unknown) => void) => {
    listenCallbacks[eventName] = cb;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
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
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { ColumnView } from "./column-view";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { FocusZone } from "./focus-zone";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
import { invoke } from "@tauri-apps/api/core";

const WINDOW_LAYER_NAME = asSegment("window");

function makeColumn(id = "col-doing", name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Render a `<ColumnView>` inside the production-shaped provider stack so
 * Pressable's CommandDef binds through the global keymap handler.
 */
async function renderColumn(onAddTask: (columnId: string) => void) {
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <EntityFocusProvider>
            <UIStateProvider>
              <AppModeProvider>
                <UndoProvider>
                  <AppShell>
                    <SchemaProvider>
                      <EntityStoreProvider entities={{}}>
                        <FieldUpdateProvider>
                          <TooltipProvider delayDuration={100}>
                            <ActiveBoardPathProvider value="/test/board">
                              <FocusZone moniker={asSegment("ui:board")}>
                                <ColumnView
                                  column={makeColumn()}
                                  tasks={[]}
                                  onAddTask={onAddTask}
                                />
                              </FocusZone>
                            </ActiveBoardPathProvider>
                          </TooltipProvider>
                        </FieldUpdateProvider>
                      </EntityStoreProvider>
                    </SchemaProvider>
                  </AppShell>
                </UndoProvider>
              </AppModeProvider>
            </UIStateProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await Promise.resolve();
  });
  return result;
}

function registerScopeArgs(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "spatial_register_scope")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

function registerZoneArgs(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "spatial_register_zone")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

function spatialFocusCalls(): Array<Record<string, unknown>> {
  const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
  return mockInvoke.mock.calls
    .filter((c: unknown[]) => c[0] === "spatial_focus")
    .map((c: unknown[]) => c[1] as Record<string, unknown>);
}

describe("ColumnView add-task button — Enter activates onAddTask via Pressable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("seeds focus on ui:column.add-task:{id} → Enter invokes onAddTask once and seeds column focus", async () => {
    const onAddTask = vi.fn();
    await renderColumn(onAddTask);
    await flushSetup();

    const leaf = registerScopeArgs().find(
      (a) => a.segment === "ui:column.add-task:col-doing",
    );
    expect(
      leaf,
      "ui:column.add-task:col-doing must register as a FocusScope leaf via Pressable",
    ).toBeDefined();

    const columnZone = registerZoneArgs().find(
      (a) => a.segment === "column:col-doing",
    );
    expect(columnZone, "column zone must register").toBeDefined();
    const columnFq = columnZone!.fq as string;

    // Drive a focus-changed event for the add-task leaf so the
    // entity-focus bridge populates the focused-scope chain.
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    await act(async () => {
      cb({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: leaf!.fq,
          next_segment: "ui:column.add-task:col-doing",
        },
      });
      currentFocusKey.key = leaf!.fq as string;
      await Promise.resolve();
    });

    // Clear prior IPC noise — we only care about post-Enter calls.
    const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
      await Promise.resolve();
    });

    expect(
      onAddTask,
      "Enter on the focused add-task leaf must invoke onAddTask exactly once",
    ).toHaveBeenCalledTimes(1);
    expect(onAddTask).toHaveBeenCalledWith("col-doing");

    const focusCalls = spatialFocusCalls().filter(
      (c) => c.fq === columnFq,
    );
    expect(
      focusCalls.length,
      "Enter must seed focus to the column FQM exactly once",
    ).toBe(1);
  });

  it("clicking the + button invokes onAddTask once and seeds column focus at least once (click also bubbles benignly to column zone)", async () => {
    const onAddTask = vi.fn();
    const { container } = await renderColumn(onAddTask);
    await flushSetup();

    const columnZone = registerZoneArgs().find(
      (a) => a.segment === "column:col-doing",
    );
    expect(columnZone).toBeDefined();
    const columnFq = columnZone!.fq as string;

    const mockInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
    mockInvoke.mockClear();

    const addButton = container.querySelector(
      'button[aria-label="Add task to To Do"]',
    ) as HTMLButtonElement | null;
    expect(addButton, "+ button must render with aria-label").toBeTruthy();

    await act(async () => {
      fireEvent.click(addButton!);
      await Promise.resolve();
    });

    expect(onAddTask).toHaveBeenCalledTimes(1);
    expect(onAddTask).toHaveBeenCalledWith("col-doing");

    // Click on "+" calls `setFocus(columnFq)` from the onPress closure
    // and also bubbles to the enclosing column `<FocusZone>`'s
    // own onClick, which fires a second `spatial_focus(columnFq)`.
    // Both pre- and post-migration the inner button has no
    // `e.stopPropagation()` (the click bubble to the column zone is
    // benign because both calls land on the same FQM), so we assert at
    // least one focus call rather than exactly one — mirroring today's
    // behavior. The Enter path above asserts exactly-once because there
    // is no native click bubble in the keyboard pathway.
    const focusCalls = spatialFocusCalls().filter(
      (c) => c.fq === columnFq,
    );
    expect(focusCalls.length).toBeGreaterThanOrEqual(1);
  });
});
