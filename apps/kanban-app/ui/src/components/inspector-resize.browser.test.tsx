/**
 * Drag-interaction test for the resizable inspector.
 *
 * Mounts `<InspectorsContainer>` with one panel open (via the same
 * production-shaped provider stack as `inspector.layer-shape.browser
 * .test.tsx`), simulates a `mousedown` on the panel's left-edge
 * resize handle, fires a `mousemove` shifting -120 px (wider) and a
 * `mouseup`. Asserts:
 *
 *   - The panel's `style.width` after `mousemove` reflects the new
 *     value (transient drag state, no backend round-trip).
 *   - `ui.inspector.set_width` was dispatched exactly once on
 *     `mouseup` with the final value.
 *
 * Pins the persistence cadence the task requires: live React state
 * during the drag, single dispatch on release — mirroring the
 * column-resize / window-geometry pattern.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API mocks so the production providers can call into them.
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
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
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
// Imports come after the mocks.
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { AppShell } from "./app-shell";
import { InspectorsContainer } from "./inspectors-container";
import { FocusLayer } from "./focus-layer";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------------

const TASK_ENTITY = {
  entity_type: "task",
  id: "T1",
  moniker: "task:T1",
  fields: { title: "Hello", status: "todo", body: "Some body" },
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "status", "body"],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "type",
      section: "header",
    },
    {
      id: "f2",
      name: "status",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "circle",
      section: "header",
    },
    {
      id: "f3",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      icon: "file-text",
      section: "body",
    },
  ],
};

const backendState = {
  inspector_stack: ["task:T1"] as string[],
  inspector_width: undefined as number | undefined,
};

function uiStateSnapshot() {
  return {
    keymap_mode: "cua" as const,
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    can_undo: false,
    can_redo: false,
    drag_session: null,
    windows: {
      main: {
        board_path: "/test",
        inspector_stack: [...backendState.inspector_stack],
        active_view_id: "",
        active_perspective_id: "",
        palette_open: false,
        palette_mode: "command" as const,
        ...(backendState.inspector_width !== undefined
          ? { inspector_width: backendState.inspector_width }
          : {}),
      },
    },
    recent_boards: [],
  };
}

async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "get_ui_state") return uiStateSnapshot();
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return TASK_SCHEMA;
  if (cmd === "get_entity") {
    const a = (args ?? {}) as { id?: string };
    return {
      entity_type: "task",
      id: a.id ?? "T1",
      moniker: `task:${a.id ?? "T1"}`,
      fields: TASK_ENTITY.fields,
    };
  }
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "list_views") return [];
  if (cmd === "list_perspectives") return [];
  if (cmd === "dispatch_command") return null;
  if (cmd === "log_command") return null;
  return null;
}

const WINDOW_LAYER_NAME = asSegment("window");

function renderInspectorChain() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <UIStateProvider>
          <EntityFocusProvider>
            <AppModeProvider>
              <UndoProvider>
                <TooltipProvider delayDuration={100}>
                  <SchemaProvider>
                    <EntityStoreProvider entities={{ task: [TASK_ENTITY] }}>
                      <FieldUpdateProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <AppShell>
                            <InspectorsContainer />
                          </AppShell>
                        </ActiveBoardPathProvider>
                      </FieldUpdateProvider>
                    </EntityStoreProvider>
                  </SchemaProvider>
                </TooltipProvider>
              </UndoProvider>
            </AppModeProvider>
          </EntityFocusProvider>
        </UIStateProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/** Force the viewport so clamp upper bound = min(800, 0.85*viewport). */
function setViewportWidth(px: number) {
  Object.defineProperty(window, "innerWidth", {
    configurable: true,
    writable: true,
    value: px,
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Inspector resize drag interaction", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:T1"];
    backendState.inspector_width = undefined;
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    mockListen.mockClear();
    listeners.clear();
    setViewportWidth(1600); // upper clamp = 800
  });

  /** Pull every `dispatch_command` invocation as `{ cmd, args }` records. */
  function dispatchedCommands() {
    return mockInvoke.mock.calls
      .filter((c) => c[0] === "dispatch_command")
      .map((c) => c[1] as { cmd: string; args?: { width?: number } });
  }

  it(
    "drag emits live width on mousemove and dispatches set_width once on mouseup",
    async () => {
      renderInspectorChain();
      await flushSetup();
      // Wait until the panel has rendered.
      const panel = await waitFor(() => {
        const p = document.querySelector(
          "[data-slide-panel]",
        ) as HTMLElement | null;
        if (!p) throw new Error("panel not yet rendered");
        return p;
      });
      const handle = document.querySelector(
        "[data-inspector-resize-handle]",
      ) as HTMLElement;
      expect(handle, "drag handle must exist").not.toBeNull();

      // Default width = 420 px. Drag the LEFT edge LEFT by 120 px to
      // grow the panel to 540 px. Choose startX so the math is
      // unambiguous regardless of the panel's actual screen position.
      const startX = 1180;
      const endX = startX - 120; // -120 deltaX → +120 width

      act(() => {
        fireEvent.mouseDown(handle, { clientX: startX, button: 0 });
      });
      act(() => {
        fireEvent.mouseMove(window, { clientX: endX });
      });

      // After mousemove the panel must already be at the new width —
      // transient state, no backend round-trip yet.
      expect(panel.style.width).toBe("540px");

      // No dispatch yet — the persistence cadence is "fire on mouseup
      // only".
      expect(
        dispatchedCommands().filter((c) => c.cmd === "ui.inspector.set_width"),
      ).toHaveLength(0);

      act(() => {
        fireEvent.mouseUp(window, { clientX: endX });
      });

      // Wait for the dispatch — useDispatchCommand defers via
      // useCallback closure but no microtask is needed; still a
      // waitFor keeps this resilient if a future change adds one.
      await waitFor(() => {
        const setWidthCalls = dispatchedCommands().filter(
          (c) => c.cmd === "ui.inspector.set_width",
        );
        expect(setWidthCalls).toHaveLength(1);
        expect(setWidthCalls[0]?.args?.width).toBe(540);
      });
    },
  );
});
