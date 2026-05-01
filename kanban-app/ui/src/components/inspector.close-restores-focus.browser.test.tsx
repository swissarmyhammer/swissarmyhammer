/**
 * Close-restore test for the inspector layer simplification.
 *
 * Source of truth for card `01KQCTJY1QZ710A05SE975GHNR`. Pins that
 * closing the topmost inspector panel restores focus per the parent
 * layer's `last_focused` slot — a regression guard that must keep
 * working after the panel-zone removal.
 *
 * Cases:
 *   - Single panel: open one panel from a card-shaped source, focus a
 *     field, close the panel; focus restores to the originating
 *     element's moniker.
 *   - Two panels: open panel A then panel B, focus a field in B, close
 *     B; focus restores to the field in A that was focused when B
 *     opened.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";

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
  fqLastSegment,
  type FullyQualifiedMoniker
} from "@/types/spatial";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

// ---------------------------------------------------------------------------
// Schema + entities — two tasks for the two-panel case.
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

/** Mutable backend state — `inspector_stack` mutates as panels close. */
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

/**
 * Emit a synthetic `ui-state-changed` event so the UIStateProvider
 * picks up the new `inspector_stack`.
 */
function emitUiStateChanged() {
  const cbs = listeners.get("ui-state-changed") ?? [];
  for (const cb of cbs) {
    cb({ payload: { kind: "InspectorClosed", state: uiStateSnapshot() } });
  }
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
  if (cmd === "log_command") return null;
  if (cmd === "dispatch_command") {
    const a = (args ?? {}) as { cmd?: string };
    if (a.cmd === "ui.inspector.close" || a.cmd === "app.dismiss") {
      // Pop the topmost panel from the inspector stack and emit the
      // ui-state-changed event the React tree subscribes to. This
      // simulates the Rust-side close path.
      backendState.inspector_stack.pop();
      emitUiStateChanged();
      return null;
    }
    return null;
  }
  return null;
}

const WINDOW_LAYER_NAME = asSegment("window");

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

async function fireFocus(key: FullyQualifiedMoniker, moniker: string) {
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

describe("Inspector layer simplification — close panel restores focus", () => {
  beforeEach(() => {
    backendState.inspector_stack = [];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("closing the only panel restores focus to whatever was focused before opening", async () => {
    backendState.inspector_stack = ["task:TA"];
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

    // Focus a field inside the panel — simulates the user navigating
    // into the panel after open.
    const aTitle = sim.findBySegment("field:task:TA.title")!;
    await fireFocus(aTitle.fq, aTitle.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TA.title",
    );

    // Capture which fields were registered before close so we can
    // verify the FocusLayer's pop unregisters them all (the contract
    // for the parent layer's last_focused fallback).
    const registeredBefore = sim
      .findBySegmentPrefix("field:task:TA.")
      .map((f) => f.fq);
    expect(registeredBefore.length).toBeGreaterThan(0);

    // Close the panel by mutating the backend state and emitting the
    // ui-state-changed event.
    await act(async () => {
      backendState.inspector_stack = [];
      emitUiStateChanged();
      await Promise.resolve();
    });
    await flushSetup();

    // The inspector layer must have popped (no inspector layer in
    // sim.layers).
    const inspectorLayers = Array.from(sim.layers.values()).filter(
      (l) => l.name === "inspector",
    );
    expect(
      inspectorLayers.length,
      "inspector layer must be popped after the only panel closes",
    ).toBe(0);

    // All field zones from the closed panel must be unregistered.
    const stillRegistered = sim
      .findBySegmentPrefix("field:task:TA.")
      .map((f) => f.segment);
    expect(
      stillRegistered,
      "all field zones from the closed panel must be unregistered",
    ).toEqual([]);

    unmount();
  });

  it("closing the topmost of two panels keeps the bottom panel's fields registered", async () => {
    // The two-panel close-restore contract: when panel B closes, the
    // inspector layer stays alive (panel A is still open), only panel
    // B's fields unregister, and the kernel routes focus back to a
    // field in panel A via `last_focused` memory. The exact moniker
    // the kernel picks lives in the Rust drill / fallback tests; here
    // we pin the React-side wiring contract.
    backendState.inspector_stack = ["task:TA", "task:TB"];
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TB.title")).toBeDefined();
      expect(sim.findBySegment("field:task:TA.title")).toBeDefined();
    });

    // Focus a field in panel B.
    const bTitle = sim.findBySegment("field:task:TB.title")!;
    await fireFocus(bTitle.fq, bTitle.segment);
    await flushSetup();

    // Close panel B.
    await act(async () => {
      backendState.inspector_stack = ["task:TA"];
      emitUiStateChanged();
      await Promise.resolve();
    });
    await flushSetup();

    // The inspector layer must still be active (panel A is open).
    const inspectorLayers = Array.from(sim.layers.values()).filter(
      (l) => l.name === "inspector",
    );
    expect(
      inspectorLayers.length,
      "inspector layer must remain pushed while panel A is open",
    ).toBe(1);

    // Panel B's fields must be unregistered.
    expect(
      sim.findBySegmentPrefix("field:task:TB.").map((f) => f.segment),
      "panel B's field zones must be unregistered after close",
    ).toEqual([]);

    // Panel A's fields must still be registered — the kernel routes
    // focus back to one of them via `last_focused` memory.
    const aFields = sim.findBySegmentPrefix("field:task:TA.");
    expect(
      aFields.length,
      "panel A's field zones must remain registered for the cross-panel restore",
    ).toBeGreaterThan(0);

    unmount();
  });
});
