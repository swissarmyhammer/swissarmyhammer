/**
 * Cross-panel nav test pinning that cardinal nav between adjacent
 * inspector panels lands on the spatially-nearest peer entity zone via
 * the kernel's iter-1 escalation.
 *
 * Originally authored for card `01KQCTJY1QZ710A05SE975GHNR` (the
 * inspector layer simplification, which removed the per-panel zone and
 * let cross-panel nav fall through to iter 0 across all field zones at
 * the layer root). Updated for card `01KQFCQ9QMQKCDYVWGTXSVK5PZ`: each
 * inspector body is now wrapped in an entity-keyed `<FocusZone>`, so
 * cardinal nav from a field looks like:
 *
 *   1. Iter 0 — peers within the same entity zone. ArrowLeft from the
 *      leftmost field in inspector B has no peer → fail.
 *   2. Escalate to the inspector-B entity zone.
 *   3. Iter 1 — zone-kind peers under the inspector layer root. The
 *      inspector-A entity zone qualifies; beam search picks it by rect.
 *
 * The cascade lands on the **entity zone** (e.g. `task:TA`), not a
 * leaf field. The kernel's same-kind filter at iter 1 restricts
 * candidates to zones (the parent is itself a zone, so iter 1 is the
 * sibling-zone beam — see `swissarmyhammer-focus/src/navigate.rs`
 * cascade docs and `beam_among_siblings`); fields are scopes and are
 * not eligible at iter 1. From the entity zone, the user can press
 * another arrow / Enter to descend into a specific field.
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
import { asSegment, fqLastSegment } from "@/types/spatial";
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
 * Stamp realistic rects on every field zone AND on each per-entity
 * zone so beam search has geometry to score at iter 0 (field zones
 * within an entity) and iter 1 (entity zones under the inspector
 * layer root). Panel A on the left (x = 0), panel B on the right
 * (x = 500), three fields stacked vertically per panel.
 *
 * Entity-zone rects bound their fields. Without rects on the entity
 * zones, `getBoundingClientRect` at mount time returns `(0, 0, 0, 0)`
 * in some browser-mode harness configurations, which collapses both
 * zones to the same point and lets iter-1 beam scoring pick the wrong
 * peer.
 */
function stampRects(
  sim: ReturnType<typeof installKernelSimulator>,
  taskIds: string[],
  fieldNames: string[],
) {
  taskIds.forEach((tid, panelIdx) => {
    const xBase = panelIdx * 500;
    const entityZone = sim.findBySegment(`task:${tid}`);
    if (entityZone) {
      entityZone.rect = {
        x: xBase,
        y: 0,
        width: 400,
        height: fieldNames.length * 30 - 2,
      };
    }
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

describe("Inspector entity-zone barrier — cross-panel navigation", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:TA", "task:TB"];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("ArrowRight from a field in panel A lands on panel B's entity zone", async () => {
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
      expect(sim.findBySegment("task:TA")).toBeDefined();
      expect(sim.findBySegment("task:TB")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const aTitle = sim.findBySegment("field:task:TA.title")!;
    await fireFocus(aTitle.fq, aTitle.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TA.title",
    );

    // Re-stamp rects right before nav — see entity-zone-barrier test
    // for the rationale (real ResizeObservers can overwrite our stamped
    // rects mid-flush).
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    fireEvent.keyDown(document, { key: "ArrowRight", code: "ArrowRight" });
    await waitFor(() =>
      expect(
        getByTestId("focused-moniker-probe").textContent,
        "ArrowRight from a field in panel A must escalate via iter-1 to panel B's entity zone",
      ).toBe("task:TB"),
    );
    unmount();
  });

  it("ArrowLeft from a field in panel B lands on panel A's entity zone", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();
    // Wait for both entity zones AND the focused field to register so
    // `stampRects` updates rects on every relevant entry. Without this,
    // stamping can race ahead of the entity-zone registrations and the
    // iter-1 beam search runs against `(0, 0, 0, 0)` rects that
    // collapse the candidate set.
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TB.status")).toBeDefined();
      expect(sim.findBySegment("task:TA")).toBeDefined();
      expect(sim.findBySegment("task:TB")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const bStatus = sim.findBySegment("field:task:TB.status")!;
    await fireFocus(bStatus.fq, bStatus.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TB.status",
    );

    // Re-stamp rects right before nav — see entity-zone-barrier test
    // for the rationale.
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    fireEvent.keyDown(document, { key: "ArrowLeft", code: "ArrowLeft" });
    await waitFor(() =>
      expect(
        getByTestId("focused-moniker-probe").textContent,
        "ArrowLeft from a field in panel B must escalate via iter-1 to panel A's entity zone",
      ).toBe("task:TA"),
    );
    unmount();
  });

  it("cross-panel nav escalates to the spatially-nearest entity-zone peer (rect-driven)", async () => {
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
      expect(sim.findBySegment("task:TA")).toBeDefined();
      expect(sim.findBySegment("task:TB")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    // Focus the middle field in panel A. ArrowRight escalates to entity
    // zone A → iter 1 picks entity zone B (the only other zone-kind
    // sibling under the inspector layer root) by rect.
    const aStatus = sim.findBySegment("field:task:TA.status")!;
    await fireFocus(aStatus.fq, aStatus.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TA.status",
    );

    // Re-stamp rects right before nav — see entity-zone-barrier test
    // for the rationale.
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    fireEvent.keyDown(document, { key: "ArrowRight", code: "ArrowRight" });
    await waitFor(() =>
      expect(
        getByTestId("focused-moniker-probe").textContent,
        "ArrowRight from panel A's middle field must escalate to panel B's entity zone",
      ).toBe("task:TB"),
    );
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
      expect(sim.findBySegment("task:TA")).toBeDefined();
      expect(sim.findBySegment("task:TB")).toBeDefined();
    });

    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const aTitle = sim.findBySegment("field:task:TA.title")!;
    await fireFocus(aTitle.fq, aTitle.segment);
    await flushSetup();

    const observations: string[] = [];
    const observe = () =>
      observations.push(getByTestId("focused-moniker-probe").textContent ?? "");
    observe();

    /**
     * Predicate: is the leaf segment an inspector-layer moniker? After
     * the entity-zone wrap, valid inspector targets are:
     *   - `field:task:TA.*` / `field:task:TB.*` — leaf field zones.
     *   - `task:TA` / `task:TB` — entity zones (cardinal nav lands
     *     here at iter 1 escalation).
     */
    const isInspectorMoniker = (m: string) =>
      m.startsWith("field:task:TA.") ||
      m.startsWith("field:task:TB.") ||
      m === "task:TA" ||
      m === "task:TB";

    for (const dir of ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"]) {
      // Re-stamp rects each iteration — see entity-zone-barrier test
      // for the rationale (real ResizeObservers can overwrite our
      // stamped rects mid-flush).
      stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);
      fireEvent.keyDown(document, { key: dir, code: dir });
      // Wait for React state to settle on an inspector-layer moniker
      // before observing. The contract under test is that focus stays
      // inside the inspector layer, so polling for an inspector moniker
      // (field zone OR entity zone) converges on the post-nav state
      // without racing the simulator's microtask `focus-changed` emit.
      await waitFor(() => {
        const text = getByTestId("focused-moniker-probe").textContent ?? "";
        expect(
          isInspectorMoniker(text),
          `focused moniker after ${dir} should be an inspector field or entity zone, got ${text}`,
        ).toBe(true);
      });
      observe();
    }

    const leaks = observations.filter(
      (m) => m !== "null" && !isInspectorMoniker(m),
    );
    expect(
      leaks,
      "cross-panel nav must keep focus inside the inspector layer (no board / column / card moniker leaks)",
    ).toEqual([]);
    unmount();
  });
});
