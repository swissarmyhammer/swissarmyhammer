/**
 * Kernel-state shape-snapshot test for the inspector entity-zone
 * barrier.
 *
 * Originally authored for card `01KQCTJY1QZ710A05SE975GHNR` (layer
 * simplification — field zones at the layer root with
 * `parentZone === null`). Updated for card `01KQFCQ9QMQKCDYVWGTXSVK5PZ`:
 * each open inspector wraps its body in an entity-keyed
 * `<FocusScope moniker={asSegment(\`${entityType}:${entityId}\`)}>`,
 * so the spatial structure is now:
 *
 *   - One shared `<FocusLayer name="inspector">` for the whole panel
 *     stack (unchanged).
 *   - One `<FocusScope moniker="task:T1">` per open inspector, keyed by
 *     the entity moniker itself (NOT a `panel:` prefix). The zone
 *     registers at the inspector layer root with `parentZone === null`.
 *   - Field zones register with `parentZone === <entity zone FQM>` —
 *     NOT `null`. This confines iter 0 of the kernel cascade to peers
 *     within the same entity, fixing the multi-inspector bleed bug
 *     (`field zone in inspector A` cascaded to field zones in
 *     inspector B because they all shared `parentZone === null`).
 *   - `<InspectorFocusBridge>` stays deleted (per
 *     `01KQCTJY1QZ710A05SE975GHNR`), so no `<FocusScope>` registers
 *     for the entity moniker as a leaf — the entity moniker is a
 *     ZONE under the new contract.
 *
 * The test uses `installKernelSimulator` to capture every spatial-nav
 * IPC the production tree fires on mount, then asserts the registered
 * shape matches the new contract.
 *
 * Cross-references:
 *   - `01KQAW97R9XTCNR1PJAWYSKBC7` — drill-out at layer root contract.
 *   - `01KQAXS8QKWCKFK8ENEMN7WHR1` — field-zone registration shape.
 *   - `01KQCKVN140DGBCK8NF8RZM4R5` — global nav unification.
 *   - `01KQ9X3A9NMRYK50GWP4S4ZMJ4` — `field.edit` drill-in/edit registration.
 *   - `01KQFCQ9QMQKCDYVWGTXSVK5PZ` — entity-zone barrier.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri-API spy triple — hoisted so the `vi.mock` factories can capture
// the spies before the module bodies run.
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
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

// ---------------------------------------------------------------------------
// Fixture — task with three fields so the assertions cover several field
// zones at once.
// ---------------------------------------------------------------------------

const TASK_ENTITY = {
  entity_type: "task",
  id: "T1",
  moniker: "task:T1",
  fields: {
    title: "Hello",
    status: "todo",
    body: "Some description",
  },
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

/** Per-test mutable UIState seeded with one inspector panel pre-pushed. */
const backendState = {
  inspector_stack: [] as string[],
  palette_open: false,
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
        palette_open: backendState.palette_open,
        palette_mode: "command" as const,
      },
    },
    recent_boards: [],
  };
}

/** Default invoke responses for every IPC the simulator does not own. */
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
 * Mount the production-shaped inspector chain via `InspectorsContainer`,
 * which reads `inspector_stack` from `UIStateProvider`.
 */
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Inspector entity-zone barrier — kernel-state shape", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:T1"];
    backendState.palette_open = false;
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("pushes exactly one inspector layer with a non-null parent (the window layer)", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(
        sim.findBySegmentPrefix("field:task:T1.").length,
        "fields must register before assertions run",
      ).toBeGreaterThan(0);
    });

    const inspectorPushes = sim.history
      .filter((h) => h.type === "push_layer")
      .map((h) => h.record)
      .filter((l) => "name" in l && l.name === "inspector");

    expect(
      inspectorPushes.length,
      "exactly one inspector layer must be pushed when at least one panel is open",
    ).toBe(1);
    expect(
      inspectorPushes[0].parent,
      "the inspector layer must be a child of the window layer (non-null parent)",
    ).not.toBeNull();
    unmount();
  });

  it("does not register any zone with the panel:* moniker prefix", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegmentPrefix("field:task:T1.").length).toBeGreaterThan(
        0,
      );
    });

    const panelZones = sim.findBySegmentPrefix("panel:");
    expect(
      panelZones.map((z) => z.segment),
      "no zone should register with a panel:* moniker — the panel zone is gone",
    ).toEqual([]);
    unmount();
  });

  it("registers a ZONE for the entity moniker (task:T1) — not a scope, no InspectorFocusBridge", async () => {
    // The entity moniker IS a zone now, registered via
    // `<FocusScope moniker={asSegment(\`${type}:${id}\`)}>` in
    // `<InspectorPanel>` (card `01KQFCQ9QMQKCDYVWGTXSVK5PZ`). The
    // deleted `<InspectorFocusBridge>` would have registered the
    // entity moniker as a leaf scope (`<FocusScope>`); the bridge
    // stays deleted, so the only registration matching `task:T1` is
    // the new entity zone.
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegmentPrefix("field:task:T1.").length).toBeGreaterThan(
        0,
      );
    });

    const entityRegistration = sim.findBySegment("task:T1");
    expect(
      entityRegistration,
      "the entity moniker must register — it is the per-inspector zone wrap",
    ).toBeDefined();
    expect(
      entityRegistration!.kind,
      "the entity moniker registers as a zone (FocusScope), not a scope (FocusScope)",
    ).toBe("zone");
    expect(
      entityRegistration!.parentZone,
      "the entity zone registers at the inspector layer root (parentZone === null)",
    ).toBeNull();
    unmount();
  });

  it("every field zone registers with parentZone === <entity zone FQM> under the inspector layer", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegmentPrefix("field:task:T1.").length).toBeGreaterThan(
        0,
      );
    });

    const inspectorPush = sim.history
      .filter((h) => h.type === "push_layer")
      .map((h) => h.record)
      .find((l) => "name" in l && l.name === "inspector");
    expect(inspectorPush, "inspector layer must be pushed").toBeDefined();

    const entityZone = sim.findBySegment("task:T1");
    expect(
      entityZone,
      "the per-entity zone must register before field-parent assertions run",
    ).toBeDefined();
    expect(
      entityZone!.kind,
      "the entity moniker registers as a zone, not a leaf scope",
    ).toBe("zone");

    const fields = sim.findBySegmentPrefix("field:task:T1.");
    expect(
      fields.length,
      "at least one field zone must register",
    ).toBeGreaterThan(0);

    const wrongLayer = fields.filter((f) => f.layerFq !== inspectorPush!.fq);
    expect(
      wrongLayer.map((f) => ({ moniker: f.segment, layerKey: f.layerFq })),
      "every field zone must register under the inspector layer's key",
    ).toEqual([]);

    const wrongParent = fields.filter((f) => f.parentZone !== entityZone!.fq);
    expect(
      wrongParent.map((f) => ({
        moniker: f.segment,
        parentZone: f.parentZone,
      })),
      "every field zone must register with parentZone === <entity zone FQM> (entity-zone barrier from card 01KQFCQ9QMQKCDYVWGTXSVK5PZ)",
    ).toEqual([]);
    unmount();
  });

  it("inspector layer push fires before any field zone registers", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegmentPrefix("field:task:T1.").length).toBeGreaterThan(
        0,
      );
    });

    // Walk history. The first inspector layer push must arrive before
    // every field-zone registration.
    const firstFieldRegisterIdx = sim.history.findIndex(
      (h) =>
        h.type === "register" && h.record.segment.startsWith("field:task:T1."),
    );
    const firstInspectorPushIdx = sim.history.findIndex(
      (h) => h.type === "push_layer" && h.record.name === "inspector",
    );

    expect(
      firstInspectorPushIdx,
      "inspector layer push must appear in history",
    ).toBeGreaterThanOrEqual(0);
    expect(
      firstFieldRegisterIdx,
      "at least one field zone register must appear in history",
    ).toBeGreaterThanOrEqual(0);
    expect(
      firstInspectorPushIdx,
      "inspector layer push must arrive before the first field zone registration",
    ).toBeLessThan(firstFieldRegisterIdx);
    unmount();
  });
});
