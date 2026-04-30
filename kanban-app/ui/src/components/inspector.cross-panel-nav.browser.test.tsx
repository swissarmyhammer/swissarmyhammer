/**
 * Cross-panel nav test pinning that the inspector layer simplification
 * supports nav between fields in different panels.
 *
 * Source of truth for card `01KQCTJY1QZ710A05SE975GHNR`. With one
 * shared `<FocusLayer name="inspector">` wrapping the entire panel
 * stack and field zones registered at `parentZone === null`, all field
 * zones across all open panels are siblings in the kernel's iter 0
 * cascade. ArrowLeft / ArrowRight thus moves focus between adjacent
 * panels without any cross-zone fallback rule — the kernel's beam
 * search picks the spatially-nearest field.
 *
 * The user direction:
 * > "One layer for the whole panel stack allowing navigation between
 * > inspectors — which you should test."
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API spy triple.
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
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import {
  asSegment
} from "@/types/spatial";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

// ---------------------------------------------------------------------------
// Schema + entities — two tasks (TA, TB) so two panels open.
// ---------------------------------------------------------------------------

const TASK_A = {
  entity_type: "task",
  id: "TA",
  moniker: "task:TA",
  fields: { title: "Alpha", status: "todo", body: "A body" },
};

const TASK_B = {
  entity_type: "task",
  id: "TB",
  moniker: "task:TB",
  fields: { title: "Bravo", status: "doing", body: "B body" },
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

const backendState = { inspector_stack: [] as string[] };

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
    const id = a.id ?? "TA";
    const fields =
      id === "TA" ? TASK_A.fields : id === "TB" ? TASK_B.fields : {};
    return {
      entity_type: "task",
      id,
      moniker: `task:${id}`,
      fields,
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

function FocusedMonikerProbe() {
  const { focusedFq } = useEntityFocus();
  return (
    <span data-testid="focused-moniker-probe">{focusedFq ?? "null"}</span>
  );
}

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
                    <EntityStoreProvider entities={{ task: [TASK_A, TASK_B] }}>
                      <FieldUpdateProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <AppShell>
                            <FocusedMonikerProbe />
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

/**
 * Stamp realistic rects on every field zone so beam search has geometry
 * to score. The rect layout puts panel A on the left (x = 0) and panel
 * B on the right (x = 500), with three fields stacked vertically per
 * panel.
 */
function stampRects(
  sim: ReturnType<typeof installKernelSimulator>,
  taskIds: string[],
  fieldNames: string[],
) {
  taskIds.forEach((tid, panelIdx) => {
    const xBase = panelIdx * 500;
    fieldNames.forEach((name, fieldIdx) => {
      const f = sim.findBySegment(`field:task:${tid}.${name}`);
      if (f)
        f.rect = {
          x: xBase,
          y: fieldIdx * 30,
          width: 400,
          height: 28,
        };
    });
  });
}

async function fireFocus(
  key: import("@/types/spatial").FullyQualifiedMoniker,
  moniker: string,
) {
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const h of handlers) {
      h({
        payload: {
          window_label: "main",
          prev_fq: null,
          next_fq: key,
          next_segment: moniker,
        },
      });
    }
    await Promise.resolve();
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Inspector layer simplification — cross-panel navigation", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:TA", "task:TB"];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("ArrowRight from a field in panel A lands on a field in panel B", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TA.title")).toBeDefined();
      expect(sim.findBySegment("field:task:TB.title")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const aTitle = sim.findBySegment("field:task:TA.title")!;
    await fireFocus(aTitle.fq, aTitle.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TA.title",
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowRight", code: "ArrowRight" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    expect(
      getByTestId("focused-moniker-probe").textContent,
      "ArrowRight from a field in panel A must land on the spatially-nearest field in panel B",
    ).toBe("field:task:TB.title");
    unmount();
  });

  it("ArrowLeft from a field in panel B lands on a field in panel A", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TB.status")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const bStatus = sim.findBySegment("field:task:TB.status")!;
    await fireFocus(bStatus.fq, bStatus.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TB.status",
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowLeft", code: "ArrowLeft" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    expect(
      getByTestId("focused-moniker-probe").textContent,
      "ArrowLeft from a field in panel B must land on the spatially-nearest field in panel A",
    ).toBe("field:task:TA.status");
    unmount();
  });

  it("cross-panel nav respects rect y-coordinate (picks the same-row field)", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TA.status")).toBeDefined();
      expect(sim.findBySegment("field:task:TB.body")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    // Focus the middle field in panel A. ArrowRight must land on the
    // middle field in panel B because beam search prefers the candidate
    // closest to the source's y-center.
    const aStatus = sim.findBySegment("field:task:TA.status")!;
    await fireFocus(aStatus.fq, aStatus.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TA.status",
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowRight", code: "ArrowRight" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    expect(
      getByTestId("focused-moniker-probe").textContent,
      "ArrowRight from panel A's middle field must pick panel B's middle field (same y)",
    ).toBe("field:task:TB.status");
    unmount();
  });

  it("during cross-panel nav, no non-inspector moniker leaks into focused-scope", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TA.title")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const aTitle = sim.findBySegment("field:task:TA.title")!;
    await fireFocus(aTitle.fq, aTitle.segment);
    await flushSetup();

    const observations: string[] = [];
    const observe = () =>
      observations.push(getByTestId("focused-moniker-probe").textContent ?? "");
    observe();

    for (const dir of ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"]) {
      await act(async () => {
        fireEvent.keyDown(document, { key: dir, code: dir });
        await new Promise((r) => setTimeout(r, 50));
      });
      await flushSetup();
      observe();
    }

    const leaks = observations.filter(
      (m) =>
        m !== "null" &&
        !m.startsWith("field:task:TA.") &&
        !m.startsWith("field:task:TB."),
    );
    expect(
      leaks,
      "cross-panel nav must keep focus inside the inspector layer (no board / column / card moniker leaks)",
    ).toEqual([]);
    unmount();
  });
});
