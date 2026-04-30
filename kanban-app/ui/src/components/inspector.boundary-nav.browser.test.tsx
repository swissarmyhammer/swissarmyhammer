/**
 * Boundary-nav test pinning the cardinal-nav contract for a single
 * inspector when there's no peer in the chosen direction.
 *
 * Originally authored for card `01KQCTJY1QZ710A05SE975GHNR` (the
 * inspector layer simplification — field zones registered at
 * `parentZone === null` so iter 0 + iter 1 found nothing and the
 * cascade echoed the focused moniker). Updated for card
 * `01KQFCQ9QMQKCDYVWGTXSVK5PZ`: each inspector now wraps its body in
 * an entity-keyed `<FocusZone>`, so field zones register with
 * `parentZone === <entity zone FQM>`. The kernel cascade then runs:
 *
 *   - Iter 0 (same-kind peers sharing `parentZone === entity zone`):
 *     finds another field zone in the same entity and advances focus
 *     to it.
 *   - Iter 0 + iter 1 + drill-out fallback: when there is no peer in
 *     the chosen direction (e.g. ArrowDown from the last field of the
 *     ONLY open inspector), the cascade escalates to the entity zone,
 *     iter 1 (zone-kind peers under the inspector layer root) finds
 *     no other entity zone, and the drill-out fallback returns the
 *     parent zone's FQM. Focus moves from the field to the **entity
 *     zone**, NOT echoed on the field.
 *
 * Either outcome (echoed field or drill-out to entity zone) keeps the
 * user inside the same entity — neither leaks board / column / card
 * monikers into `useFocusedScope()`. This test pins the inside-entity
 * invariant rather than a single specific moniker.
 *
 * Cross-references:
 *   - `01KQAW97R9XTCNR1PJAWYSKBC7` — no-silent-dropout contract.
 *   - `01KQFCQ9QMQKCDYVWGTXSVK5PZ` — entity-zone barrier.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

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
import { asSegment, fqLastSegment } from "@/types/spatial";
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
  return <span data-testid="focused-moniker-probe">{segment ?? "null"}</span>;
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

describe("Inspector entity-zone barrier — boundary navigation", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:T1"];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  /**
   * Predicate: is `m` a moniker that keeps focus inside inspector T1?
   *
   * Two outcomes both satisfy "stay inside the entity":
   *
   *   - `field:task:T1.*` — the cascade echoed the focused field
   *     (no-silent-dropout contract from `01KQAW97R9XTCNR1PJAWYSKBC7`).
   *     Happens when the cascade returns null (no peer + parent
   *     fallback yields the same FQM).
   *   - `task:T1` — the cascade hit drill-out fallback and returned
   *     the entity zone's FQM. Happens when iter 1 finds no peer
   *     entity zone (single-inspector case).
   */
  const stayedInEntity = (m: string) =>
    m.startsWith("field:task:T1.") || m === "task:T1";

  /** Stamp rects on the entity zone AND every field zone. */
  function stampInspectorRects(sim: ReturnType<typeof installKernelSimulator>) {
    const orderedNames = ["title", "status", "body"];
    const entityZone = sim.findBySegment("task:T1");
    if (entityZone) {
      entityZone.rect = {
        x: 0,
        y: 0,
        width: 400,
        height: orderedNames.length * 30 - 2,
      };
    }
    orderedNames.forEach((name, idx) => {
      const f = sim.findBySegment(`field:task:T1.${name}`);
      if (f) f.rect = { x: 0, y: idx * 30, width: 400, height: 28 };
    });
  }

  it("ArrowDown on the last field stays inside the entity (drill-out to entity zone)", async () => {
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
    // return null.
    const fields = sim.findBySegmentPrefix("field:task:T1.");
    expect(fields.length).toBeGreaterThanOrEqual(2);
    stampInspectorRects(sim);

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

    // Drive ArrowDown. With the entity-zone barrier:
    //   - Iter 0 (peers within entity T1): no down-peer below the last
    //     field. Iter 0 fails.
    //   - Escalate to entity zone T1.
    //   - Iter 1 (zone-kind peers under inspector layer root): no other
    //     entity zone is open. Iter 1 fails.
    //   - Drill-out fallback returns the parent zone's FQM
    //     (`task:T1`).
    await act(async () => {
      fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
      await new Promise((r) => setTimeout(r, 50));
    });
    await flushSetup();

    const focused = getByTestId("focused-moniker-probe").textContent ?? "";
    expect(
      stayedInEntity(focused),
      `ArrowDown at the last field must keep focus inside the entity (echoed field or drill-out to entity zone), got ${focused}`,
    ).toBe(true);
    unmount();
  });

  it("ArrowUp on the first field stays inside the entity (drill-out to entity zone)", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();

    stampInspectorRects(sim);

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

    const focused = getByTestId("focused-moniker-probe").textContent ?? "";
    expect(
      stayedInEntity(focused),
      `ArrowUp at the first field must keep focus inside the entity (echoed field or drill-out to entity zone), got ${focused}`,
    ).toBe(true);
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

    stampInspectorRects(sim);

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

    // Every observation must be either a `field:task:T1.*` moniker or
    // the entity-zone moniker `task:T1`. Board / column / card monikers
    // leaking in would mean the inspector layer's boundary failed.
    const leaks = observations.filter(
      (m) => m !== "null" && !stayedInEntity(m),
    );
    expect(
      leaks,
      "no non-inspector moniker may appear in useFocusedScope() during boundary nav (entity zone or field-only)",
    ).toEqual([]);
    unmount();
  });
});
