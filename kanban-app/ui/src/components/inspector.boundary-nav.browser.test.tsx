/**
 * Boundary-nav test pinning that the inspector layer simplification
 * preserves the "echoed moniker at the layer edge" contract for
 * cardinal navigation keys.
 *
 * Source of truth for card `01KQCTJY1QZ710A05SE975GHNR`. With field
 * zones registered directly at the inspector layer root (no panel zone
 * between them), the kernel's beam-search cascade has these outcomes:
 *
 *   - Iter 0 (same-kind peers sharing `parentZone === null` in the
 *     inspector layer): finds another field zone in the same layer and
 *     advances focus to it.
 *   - Iter 0 + iter 1 + drill-out fallback: when there is no peer in
 *     the chosen direction (e.g. ArrowDown from the last field), the
 *     cascade returns the focused moniker echoed (the no-silent-dropout
 *     contract from `01KQAW97R9XTCNR1PJAWYSKBC7`).
 *
 * This test exercises the cardinal-direction stay-put path: ArrowDown
 * (`nav.down`) on the last field keeps focus on the same field;
 * ArrowUp (`nav.up`) on the first field does the same. The
 * Escape-driven `nav.drillOut` equality-→-`app.dismiss` fall-through
 * is a separate path and is covered elsewhere — this test only
 * verifies that boundary nav at the layer edge keeps focus on the same
 * field. During the entire interaction `useFocusedScope()` reports
 * only inspector-layer monikers (no board / column / card moniker
 * leaks into the focused-scope chain).
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
  asSegment,
  fqLastSegment
} from "@/types/spatial";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

// ---------------------------------------------------------------------------
// Schema + entity fixtures
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
  inspector_stack: [] as string[],
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

/**
 * Reads `useFocusedMoniker()` and exposes it as text for assertions.
 */
function FocusedMonikerProbe() {
  const { focusedFq } = useEntityFocus();
  const segment = focusedFq ? fqLastSegment(focusedFq) : null;
  return (
    <span data-testid="focused-moniker-probe">{segment ?? "null"}</span>
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
                    <EntityStoreProvider entities={{ task: [TASK_ENTITY] }}>
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Inspector layer simplification — boundary navigation", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:T1"];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("ArrowDown on the last field echoes the focused moniker (stays put)", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();

    // Stamp realistic rects so beam search has geometry to work with.
    // jsdom-mode `getBoundingClientRect` returns zeros; without rects,
    // the navigateInShadow port has nothing to score and would always
    // return null. We post an `update_rect` for each field zone in
    // top-down order so iter 0 has real geometry.
    const fields = sim.findBySegmentPrefix("field:task:T1.");
    expect(fields.length).toBeGreaterThanOrEqual(2);
    const orderedNames = ["title", "status", "body"];
    orderedNames.forEach((name, idx) => {
      const f = sim.findBySegment(`field:task:T1.${name}`);
      if (f) f.rect = { x: 0, y: idx * 30, width: 400, height: 28 };
    });

    const last = sim.findBySegment("field:task:T1.body");
    expect(last, "body field zone must register").toBeDefined();

    // Seed focus on the last field via a focus-changed event.
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: last!.fq,
            next_segment: last!.segment,
          },
        });
      }
      await Promise.resolve();
    });
    await flushSetup();

    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:T1.body",
    );

    // Drive ArrowDown. The beam-search cascade (iter 0 + iter 1) finds
    // no down-peer below the last field → the React adapter resolves to
    // the echoed moniker (stays put).
    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    expect(
      getByTestId("focused-moniker-probe").textContent,
      "ArrowDown at the last field must keep focus on the same moniker (echoed)",
    ).toBe("field:task:T1.body");
    unmount();
  });

  it("ArrowUp on the first field echoes the focused moniker (stays put)", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();

    const orderedNames = ["title", "status", "body"];
    orderedNames.forEach((name, idx) => {
      const f = sim.findBySegment(`field:task:T1.${name}`);
      if (f) f.rect = { x: 0, y: idx * 30, width: 400, height: 28 };
    });

    const first = sim.findBySegment("field:task:T1.title");
    expect(first, "title field zone must register").toBeDefined();

    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: first!.fq,
            next_segment: first!.segment,
          },
        });
      }
      await Promise.resolve();
    });
    await flushSetup();

    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:T1.title",
    );

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    expect(
      getByTestId("focused-moniker-probe").textContent,
      "ArrowUp at the first field must keep focus on the same moniker (echoed)",
    ).toBe("field:task:T1.title");
    unmount();
  });

  it("during boundary nav, no non-inspector moniker leaks into focused-scope", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();

    const orderedNames = ["title", "status", "body"];
    orderedNames.forEach((name, idx) => {
      const f = sim.findBySegment(`field:task:T1.${name}`);
      if (f) f.rect = { x: 0, y: idx * 30, width: 400, height: 28 };
    });

    const last = sim.findBySegment("field:task:T1.body");
    expect(last).toBeDefined();
    const handlers = listeners.get("focus-changed") ?? [];
    await act(async () => {
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: null,
            next_fq: last!.fq,
            next_segment: last!.segment,
          },
        });
      }
      await Promise.resolve();
    });
    await flushSetup();

    // Capture every focused-moniker observation across a sequence of
    // arrow presses.
    const observations: string[] = [];
    const observe = () =>
      observations.push(getByTestId("focused-moniker-probe").textContent ?? "");
    observe();

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();
    observe();

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();
    observe();

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();
    observe();

    // Every observation must be a `field:` moniker. Board / column /
    // card / panel monikers leaking in would mean the inspector layer's
    // boundary failed.
    const leaks = observations.filter(
      (m) => m !== "null" && !m.startsWith("field:task:T1."),
    );
    expect(
      leaks,
      "no non-inspector moniker may appear in useFocusedScope() during boundary nav",
    ).toEqual([]);
    unmount();
  });
});

/** Suppress vitest warning about unused waitFor import. */
void waitFor;
