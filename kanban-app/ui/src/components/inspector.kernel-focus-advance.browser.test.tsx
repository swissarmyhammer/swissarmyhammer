/**
 * Kernel-focus-advance test pinning that opening an inspector advances the
 * Rust kernel's `focused_key` to the inspector's first field — not just the
 * React-side entity-focus store.
 *
 * Source of truth for card `01KQD0WK54G0FRD7SZVZASA9ST`. The bug:
 * `setFocus(moniker)` on the entity-focus context historically updated only
 * the React store and dispatched `ui.setFocus` for scope-chain bookkeeping,
 * but did **not** call `spatial_focus` on the kernel. The kernel and the
 * React side then drifted: the kernel still believed focus sat on the
 * originating card, while the React store reported the inspector field.
 *
 * Concrete consequence: `nav.down`'s execute closure in `app-shell.tsx`
 * reads `actions.focusedKey()` (the kernel's focus mirror) and threads it
 * into `spatial_navigate`. With the kernel out of sync, ArrowDown from an
 * open inspector dispatched `spatial_navigate(cardKey, "down")` — which
 * cascades on the *board*, not the inspector — and focus visually
 * "escaped" the inspector layer.
 *
 * The user direction:
 * > "I expect the state for the focus to be in the Rust kernel and the UI
 * > to just render it. That was kinda the whole point to avoid two sets of
 * > state."
 *
 * After this card lands, `setFocus(moniker)` becomes a pure dispatch:
 * the React side asks the kernel to move focus by moniker, the kernel
 * advances, the kernel emits `focus-changed`, and the React store updates
 * via the existing `subscribeFocusChanged` bridge. The kernel's focused
 * key stays in lockstep with whatever the user is looking at.
 *
 * This test exercises the bug reproduction path end-to-end: render the
 * inspector with a kernel simulator that tracks `currentFocus.key` like
 * the real Rust kernel does, drive the first-field auto-focus on mount,
 * assert the simulator's focused key is the inspector field (not the
 * originating card), and dispatch ArrowDown to verify `spatial_navigate`
 * is called with the field's key — not the card's.
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
import { asLayerName } from "@/types/spatial";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

// ---------------------------------------------------------------------------
// Schema + entity fixtures.
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

const WINDOW_LAYER_NAME = asLayerName("window");

/**
 * Reads `useFocusedMoniker()` and exposes it as text for assertions.
 */
function FocusedMonikerProbe() {
  const { focusedMoniker } = useEntityFocus();
  return (
    <span data-testid="focused-moniker-probe">{focusedMoniker ?? "null"}</span>
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

/**
 * Stamp realistic rects so beam search has geometry to score. jsdom-mode
 * `getBoundingClientRect` returns zeros; without rects, the
 * `navigateInShadow` port has nothing to score and would always return
 * null. We post update rects for each field zone in top-down order so
 * iter 0 has real geometry.
 */
function stampFieldRects(
  sim: ReturnType<typeof installKernelSimulator>,
  taskId: string,
  fieldNames: string[],
): void {
  fieldNames.forEach((name, idx) => {
    const f = sim.findByMoniker(`field:task:${taskId}.${name}`);
    if (f) f.rect = { x: 0, y: idx * 30, width: 400, height: 28 };
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Inspector kernel-focus advance — kernel is the source of truth", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:T1"];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("first-field auto-focus advances the kernel's focused key (NOT the originating card)", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();

    await waitFor(() => {
      expect(sim.findByMoniker("field:task:T1.title")).toBeDefined();
    });
    stampFieldRects(sim, "T1", ["title", "status", "body"]);
    await flushSetup();

    const titleField = sim.findByMoniker("field:task:T1.title");
    expect(titleField, "title field zone must register").toBeDefined();

    // The first-field auto-focus runs from `useFirstFieldFocus` on mount.
    // Under the new contract, that hook calls `setFocus(firstFieldMoniker)`,
    // which dispatches a kernel command (`spatial_focus_by_moniker` or
    // equivalent) — the kernel resolves the moniker to a SpatialKey,
    // updates `focus_by_window`, and emits `focus-changed`. The
    // simulator's `currentFocus.key` mirrors the real kernel's focused
    // slot, so it must equal the title field's key.
    await waitFor(
      () => {
        expect(
          sim.currentFocus.key,
          "kernel's focused key must advance to the inspector's first field after mount",
        ).toBe(titleField!.key);
      },
      { timeout: 200 },
    );

    // The React-side store is downstream of the kernel — it must report
    // the same moniker after the focus-changed event flows through the
    // bridge.
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:T1.title",
    );
    unmount();
  });

  it("ArrowDown from the inspector dispatches spatial_navigate with the inspector field's key", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();

    await waitFor(() => {
      expect(sim.findByMoniker("field:task:T1.title")).toBeDefined();
    });
    stampFieldRects(sim, "T1", ["title", "status", "body"]);
    await flushSetup();

    const titleField = sim.findByMoniker("field:task:T1.title")!;
    const statusField = sim.findByMoniker("field:task:T1.status")!;

    // Wait for the first-field auto-focus to settle the kernel.
    await waitFor(
      () => {
        expect(sim.currentFocus.key).toBe(titleField.key);
      },
      { timeout: 200 },
    );

    // Reset the spy so we can pin the exact `spatial_navigate` call shape.
    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    // Find every `spatial_navigate` call in the IPC trace; assert at
    // least one fired with `key === titleField.key`. Under the bug,
    // `actions.focusedKey()` returned null (or worse: a board card's
    // key) because `setFocus` never advanced the kernel — so the
    // dispatched key would be wrong (or no dispatch would fire at all).
    const navigateCalls = mockInvoke.mock.calls.filter(
      (call) => call[0] === "spatial_navigate",
    );
    expect(
      navigateCalls.length,
      "ArrowDown must dispatch spatial_navigate at least once",
    ).toBeGreaterThanOrEqual(1);
    const navigateArgs = navigateCalls.map((call) => call[1]) as Array<{
      key: string;
      direction: string;
    }>;
    const dispatchedFromTitle = navigateArgs.find(
      (a) => a.key === titleField.key && a.direction === "down",
    );
    expect(
      dispatchedFromTitle,
      "spatial_navigate must be invoked with the inspector field's key (not a board card's key)",
    ).toBeDefined();

    // Sanity: kernel landed on the next field (status) after the cascade.
    await waitFor(() => {
      expect(sim.currentFocus.key).toBe(statusField.key);
    });
    unmount();
  });

  it("during arrow-key nav inside the inspector, no board moniker leaks into useFocusedScope()", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();

    await waitFor(() => {
      expect(sim.findByMoniker("field:task:T1.title")).toBeDefined();
    });
    stampFieldRects(sim, "T1", ["title", "status", "body"]);
    await flushSetup();

    const titleField = sim.findByMoniker("field:task:T1.title")!;
    await waitFor(
      () => {
        expect(sim.currentFocus.key).toBe(titleField.key);
      },
      { timeout: 200 },
    );

    const observations: string[] = [];
    const observe = () =>
      observations.push(getByTestId("focused-moniker-probe").textContent ?? "");
    observe();

    for (const dir of ["ArrowDown", "ArrowDown", "ArrowUp", "ArrowDown"]) {
      await act(async () => {
        fireEvent.keyDown(document, { key: dir, code: dir });
        await new Promise((r) => setTimeout(r, 50));
      });
      await flushSetup();
      observe();
    }

    // Every observation must be a `field:` or `panel:` moniker. A
    // `card:*` or `column:*` moniker would mean focus crossed the
    // inspector layer boundary — the bug.
    const leaks = observations.filter(
      (m) => m !== "null" && !m.startsWith("field:") && !m.startsWith("panel:"),
    );
    expect(
      leaks,
      "no board / column / card moniker may appear in useFocusedScope() while the inspector is open",
    ).toEqual([]);
    unmount();
  });

  it("ArrowDown from the last field stays put, kernel's focused key remains the last field", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();

    await waitFor(() => {
      expect(sim.findByMoniker("field:task:T1.body")).toBeDefined();
    });
    stampFieldRects(sim, "T1", ["title", "status", "body"]);
    await flushSetup();

    const lastField = sim.findByMoniker("field:task:T1.body")!;

    // Drive the kernel to the last field via the entity-focus setter — the
    // user-visible flow that matters most here. After the new contract,
    // `setFocus("field:task:T1.body")` calls the kernel; the kernel
    // emits `focus-changed`; the React store mirrors it.
    const focusedKeyBefore = sim.currentFocus.key;
    expect(focusedKeyBefore).not.toBe(lastField.key); // first field auto-focused, not last
    // Move via spatial_focus from inside React: invoke setFocus directly
    // by firing a keystroke loop until we land on body. Cleaner than
    // poking React internals — beam search drives the move.
    for (let i = 0; i < 5; i++) {
      if (sim.currentFocus.key === lastField.key) break;
      await act(async () => {
        fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
        await new Promise((r) => setTimeout(r, 50));
      });
      await flushSetup();
    }
    await waitFor(() => {
      expect(sim.currentFocus.key).toBe(lastField.key);
    });
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:T1.body",
    );

    // ArrowDown at the last field — beam search has no down-peer, so
    // the cascade echoes the focused moniker (no-silent-dropout).
    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    expect(sim.currentFocus.key).toBe(lastField.key);
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:T1.body",
    );
    unmount();
  });
});
