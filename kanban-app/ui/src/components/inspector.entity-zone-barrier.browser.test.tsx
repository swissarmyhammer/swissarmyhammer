/**
 * Entity-zone barrier test pinning the cardinal-nav contract for the
 * multi-inspector case.
 *
 * Source of truth for card `01KQFCQ9QMQKCDYVWGTXSVK5PZ`. With two
 * inspectors open and each inspector body wrapped in a
 * `<FocusZone moniker={asSegment(\`${entityType}:${entityId}\`)}>`,
 * cardinal nav at iter 0 of the kernel's beam-search cascade is
 * confined to peers under the same entity zone:
 *
 *   - ArrowDown from the last field in inspector A stays on that field
 *     (no peer below within the same entity zone). It MUST NOT cross
 *     into inspector B's fields.
 *   - ArrowUp from the first field in inspector A stays put — symmetric
 *     guard.
 *   - Cross-entity ArrowLeft/Right still works at iter 1 — beam search
 *     escalates to the entity-zone peers (the two inspector bodies) and
 *     finds the adjacent inspector's fields by rect.
 *
 * The fourth test snapshots the kernel-side registration shape: each
 * open inspector pushes a zone keyed by the entity moniker, and every
 * field zone underneath registers with `parentZone === <entity-zone FQM>`
 * (NOT `null`).
 *
 * This card explicitly does NOT bring back `<InspectorFocusBridge>`,
 * does NOT use a `panel:*` moniker shape (the entity moniker IS the
 * identity), and does NOT reintroduce `inspector.edit/editEnter/exitEdit`
 * commands — see `01KQCTJY1QZ710A05SE975GHNR` for the deletions that
 * stay deleted.
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

/** Mirrors the focused FQM's last segment via `useEntityFocus`. */
function FocusedMonikerProbe() {
  const { focusedFq } = useEntityFocus();
  const segment = focusedFq ? fqLastSegment(focusedFq) : null;
  return <span data-testid="focused-moniker-probe">{segment ?? "null"}</span>;
}

/**
 * Mirrors the focused FQM as the **full path string**. Tests need the full
 * path to detect "did focus cross into the other entity?", because the
 * leaf segment alone (e.g. `field:task:TA.body`) does not include the
 * inspector's entity-zone prefix.
 */
function FocusedFqProbe() {
  const { focusedFq } = useEntityFocus();
  return <span data-testid="focused-fq-probe">{focusedFq ?? "null"}</span>;
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
                            <FocusedFqProbe />
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
 * zone so beam search has geometry to score at both iter 0 (field
 * zones inside an entity) and iter 1 (entity zones under the inspector
 * layer root). Panel A on the left (x = 0), panel B on the right
 * (x = 500), three fields stacked vertically per panel.
 *
 * The entity zones get a rect that bounds their fields — rect(x,
 * 0, 400, fields*30 - margin). Without this, the entity zones
 * register in browser-mode tests with whatever rect
 * `getBoundingClientRect` returns at mount time, which is sometimes
 * `(0, 0, 0, 0)` and breaks iter-1 beam scoring.
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

describe("Inspector entity-zone barrier — multi-inspector cardinal nav", () => {
  beforeEach(() => {
    backendState.inspector_stack = ["task:TA", "task:TB"];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("ArrowDown at the last field of inspector A stays put — does not enter inspector B's fields", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TA.body")).toBeDefined();
      expect(sim.findBySegment("field:task:TB.body")).toBeDefined();
      expect(sim.findBySegment("task:TA")).toBeDefined();
      expect(sim.findBySegment("task:TB")).toBeDefined();
    });

    // Realistic side-by-side geometry: panel A on the left, panel B on
    // the right, both stacked at the same y. Without an entity-zone
    // barrier, all fields share `parentZone === null` and ArrowDown's
    // beam search would still consider TB's fields at iter 0; with
    // pathological field geometry (e.g. TB's title row offset down a
    // few px) that lets cross-entity bleed happen on a single
    // ArrowDown. With the entity-zone wrap, iter 0 of the cascade is
    // confined to A's own field zones (`parentZone === entity zone A`)
    // and there is no peer below A's last field within the same
    // entity. Escalation to entity zone A then runs iter 1 over the
    // zone-kind peers (TA-zone, TB-zone) under the inspector layer
    // root. With same-y, side-by-side rects neither zone is "below"
    // the other and iter 1 fails — the drill-out fallback returns the
    // parent zone (entity zone A) itself. Focus stays inside A.
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const aBody = sim.findBySegment("field:task:TA.body")!;
    await fireFocus(aBody.fq, aBody.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TA.body",
    );

    // Re-stamp rects right before the key event. Real `ResizeObserver`s
    // attached to entity-zone divs may have fired `updateRect` IPCs
    // during the focus-changed flush, overwriting our stamped rects
    // with browser-mode `getBoundingClientRect()` values that are
    // sometimes `(0, 0, 0, 0)`. Stamping again immediately before nav
    // guarantees the simulator's beam search runs against deterministic
    // geometry.
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    fireEvent.keyDown(document, { key: "ArrowDown", code: "ArrowDown" });
    // Wait for the simulator's microtask focus-changed emit to flush.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Focus must still be inside inspector A — either echoed on the
    // same field (no-silent-dropout) or drill-out to entity zone A
    // when iter 1 finds no peer below. Both outcomes satisfy "stays
    // in A — does NOT enter inspector B's fields".
    const focused = getByTestId("focused-moniker-probe").textContent ?? "";
    const focusedFq = getByTestId("focused-fq-probe").textContent ?? "";
    expect(
      focused === "field:task:TA.body" || focused === "task:TA",
      `ArrowDown at A's last field must stay in inspector A (echoed field or drill-out to entity zone), got ${focused}`,
    ).toBe(true);
    // And the FQM must NOT route through inspector B's entity zone.
    expect(
      focusedFq,
      "ArrowDown must NOT cross the entity-zone barrier into inspector B",
    ).not.toMatch(/task:TB/);
    unmount();
  });

  it("ArrowUp at the first field of inspector A stays put — does not enter inspector B's fields", async () => {
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

    // Realistic side-by-side geometry — see ArrowDown test above for the
    // cascade-trace rationale. ArrowUp from A's first field has no peer
    // above within entity A; iter 1 (zone-kind sibling beam) finds TB
    // at the same y so beam search filters TB out as not above; the
    // drill-out fallback returns entity zone A. Focus stays inside A.
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    const aTitle = sim.findBySegment("field:task:TA.title")!;
    await fireFocus(aTitle.fq, aTitle.segment);
    await flushSetup();
    expect(getByTestId("focused-moniker-probe").textContent).toBe(
      "field:task:TA.title",
    );

    // Re-stamp rects right before nav — see ArrowDown test for why.
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    fireEvent.keyDown(document, { key: "ArrowUp", code: "ArrowUp" });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const focused = getByTestId("focused-moniker-probe").textContent ?? "";
    const focusedFq = getByTestId("focused-fq-probe").textContent ?? "";
    expect(
      focused === "field:task:TA.title" || focused === "task:TA",
      `ArrowUp at A's first field must stay in inspector A (echoed field or drill-out to entity zone), got ${focused}`,
    ).toBe(true);
    expect(
      focusedFq,
      "ArrowUp must NOT cross the entity-zone barrier into inspector B",
    ).not.toMatch(/task:TB/);
    unmount();
  });

  it("cross-entity ArrowLeft from B's leftmost field lands on entity zone A (iter-1 escalation through entity-zone peers)", async () => {
    // The kernel cascade at iter 1 is restricted to zone-kind peers
    // (the parent is itself a zone, so iter 1 is the sibling-zone
    // beam — see `swissarmyhammer-focus/src/navigate.rs` cascade
    // docs and `beam_among_siblings`). With the entity-zone wrap, the
    // cascade from `field:task:TB.status` goes:
    //
    //   field zone (iter 0 — peers within entity B; no left peer)
    //     → escalate to entity zone B
    //     → iter 1 — zone-kind peers of TB-zone sharing the inspector
    //       layer root; entity zone A qualifies and wins by rect.
    //
    // Focus lands on **entity zone A** (`task:TA`), NOT a leaf field
    // in A. The user descends into a specific field with another
    // arrow / Enter.
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { getByTestId, unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegment("field:task:TB.status")).toBeDefined();
      expect(sim.findBySegment("field:task:TA.status")).toBeDefined();
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

    // Re-stamp rects right before the key event. Real `ResizeObserver`s
    // attached to the entity-zone divs may have fired `updateRect` IPCs
    // during the focus-changed flush, overwriting our stamped rects with
    // browser-mode `getBoundingClientRect()` values that are sometimes
    // `(0, 0, 0, 0)`. Stamping again immediately before nav guarantees
    // the simulator's beam search runs against deterministic geometry.
    stampRects(sim, ["TA", "TB"], ["title", "status", "body"]);

    fireEvent.keyDown(document, { key: "ArrowLeft", code: "ArrowLeft" });
    await waitFor(() =>
      expect(
        getByTestId("focused-moniker-probe").textContent,
        "ArrowLeft from inspector B must land on inspector A's entity zone via iter-1 escalation",
      ).toBe("task:TA"),
    );
    unmount();
  });

  it("kernel-state shape: each open inspector registers an entity-keyed zone, field zones list the entity-zone FQM as parentZone", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      defaultInvokeImpl,
    );
    const { unmount } = renderInspectorChain();
    await flushSetup();
    await waitFor(() => {
      expect(sim.findBySegmentPrefix("field:task:TA.").length).toBeGreaterThan(
        0,
      );
      expect(sim.findBySegmentPrefix("field:task:TB.").length).toBeGreaterThan(
        0,
      );
      expect(sim.findBySegment("task:TA")).toBeDefined();
      expect(sim.findBySegment("task:TB")).toBeDefined();
    });

    // Find the per-entity zones by their declared segment moniker —
    // the entity moniker itself, NOT a `panel:*` prefix.
    const entityZoneA = sim.findBySegment("task:TA");
    const entityZoneB = sim.findBySegment("task:TB");
    expect(
      entityZoneA,
      "inspector A must register a zone keyed by entity moniker `task:TA`",
    ).toBeDefined();
    expect(
      entityZoneB,
      "inspector B must register a zone keyed by entity moniker `task:TB`",
    ).toBeDefined();
    expect(entityZoneA!.kind).toBe("zone");
    expect(entityZoneB!.kind).toBe("zone");

    // Both per-entity zones live at the inspector layer root, so their
    // own parentZone is null.
    expect(
      entityZoneA!.parentZone,
      "entity zone A registers at the inspector layer root (parentZone === null)",
    ).toBeNull();
    expect(
      entityZoneB!.parentZone,
      "entity zone B registers at the inspector layer root (parentZone === null)",
    ).toBeNull();

    // No `panel:*` zone — the entity moniker IS the identity.
    expect(
      sim.findBySegmentPrefix("panel:").map((z) => z.segment),
      "no zone should register with a panel:* moniker — this card uses the entity moniker directly",
    ).toEqual([]);

    // Every field zone in inspector A lists entity zone A's FQM as parentZone.
    const fieldsA = sim.findBySegmentPrefix("field:task:TA.");
    expect(
      fieldsA.length,
      "inspector A must register at least one field zone",
    ).toBeGreaterThan(0);
    const wrongParentA = fieldsA.filter(
      (f) => f.parentZone !== entityZoneA!.fq,
    );
    expect(
      wrongParentA.map((f) => ({
        segment: f.segment,
        parentZone: f.parentZone,
      })),
      "every field zone under inspector A must list entity zone A's FQM as parentZone",
    ).toEqual([]);

    // Every field zone in inspector B lists entity zone B's FQM as parentZone.
    const fieldsB = sim.findBySegmentPrefix("field:task:TB.");
    expect(
      fieldsB.length,
      "inspector B must register at least one field zone",
    ).toBeGreaterThan(0);
    const wrongParentB = fieldsB.filter(
      (f) => f.parentZone !== entityZoneB!.fq,
    );
    expect(
      wrongParentB.map((f) => ({
        segment: f.segment,
        parentZone: f.parentZone,
      })),
      "every field zone under inspector B must list entity zone B's FQM as parentZone",
    ).toEqual([]);
    unmount();
  });
});
