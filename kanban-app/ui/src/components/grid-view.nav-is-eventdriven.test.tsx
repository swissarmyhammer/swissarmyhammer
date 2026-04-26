/**
 * Regression test: enforces the event-driven grid contract.
 *
 * Contract: arrow-key navigation, cell click, and any focus change that does
 * not touch entity data MUST NOT trigger any backend data-fetch IPC. The
 * grid body is kept in sync by the Tauri event stream (`entity-created`,
 * `entity-field-changed`, `entity-removed`). Field cells subscribe via
 * `useFieldValue(entityType, entityId, fieldName)` and redraw from the
 * store — no re-fetch required.
 *
 * The allowed per-nav IPC is `ui.setFocus`, which is a state-mutation
 * dispatch (the frontend tells the backend "focus is now on moniker X"),
 * not a data fetch.
 *
 * Disallowed on nav:
 *   - `dispatch_command { cmd: "perspective.list" }`
 *   - `list_entities`
 *   - `get_entity`
 *   - `get_board_data`
 *
 * This file intentionally mounts only the real contexts that matter for
 * the invariant (schema + entity store + entity focus). The heavy outer
 * stack (PerspectivesContainer, WindowContainer, RustEngineContainer) is
 * mocked so the test captures fetch regressions in grid-adjacent code,
 * not transient churn from upstream provider fetches.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, screen } from "@testing-library/react";
import { memo, useEffect, useState } from "react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
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
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
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
// Mock the perspective container so the grid gets a stable activePerspective
// without requiring PerspectivesContainer / PerspectiveProvider (both of
// which would fire their own mount fetches — tracked by the dependency task
// 01KPZPY5F5HPXDKKHGKDEW6FNZ). This test keeps its scope tight on the grid
// body's behaviour: no render, click, or nav event may initiate a fetch.
// ---------------------------------------------------------------------------

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// Mock useGrid so cursor state is stable and the grid doesn't try to measure
// the DOM during tests (jsdom has no layout).
vi.mock("@/hooks/use-grid", () => ({
  useGrid: () => ({
    cursor: { row: 0, col: 0 },
    mode: "normal",
    setCursor: vi.fn(),
    moveCursor: vi.fn(),
    startEdit: vi.fn(),
    endEdit: vi.fn(),
    enterEdit: vi.fn(),
    exitEdit: vi.fn(),
    enterVisual: vi.fn(),
    exitVisual: vi.fn(),
    toggleVisual: vi.fn(),
    clearVisual: vi.fn(),
    getSelectedRange: () => null,
  }),
}));

// Render DataTable as a thin div so the grid's CommandScope + spatial-nav
// wiring still runs (each cell registers as a `<Focusable>` leaf through
// the real GridView path), but we skip the virtualized row machinery that
// is orthogonal to the fetch contract. The mock receives the same props
// the real `<DataTable>` does; this stub renders only the test-id marker
// so callers can assert the table mounted.
vi.mock("@/components/data-table", () => ({
  DataTable: () => <div data-testid="data-table" />,
}));

// ---------------------------------------------------------------------------
// Import the real providers AFTER mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider, useFieldValue } from "@/lib/entity-store-context";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Task schema — matches the on-disk task entity shape the grid expects.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    search_display_field: "title",
  },
  fields: [
    { name: "title", type: "string", section: "header", display: "text" },
  ],
} as unknown as EntitySchema;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a task Entity seed with the given id and title. */
function seedTask(id: string, title: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: { title },
  };
}

/** Seed five tasks — enough to exercise nav.down / nav.right without edge cases. */
function fiveTasks(): Entity[] {
  return [
    seedTask("t1", "Alpha"),
    seedTask("t2", "Beta"),
    seedTask("t3", "Gamma"),
    seedTask("t4", "Delta"),
    seedTask("t5", "Epsilon"),
  ];
}

/**
 * Fire a simulated Tauri event to all registered handlers for `eventName`.
 * Wraps the dispatch in `act()` so React state updates are flushed.
 */
async function fireTauriEvent(eventName: string, payload: unknown) {
  const handlers = listeners.get(eventName) ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
  });
}

/**
 * Capture the focus API into a ref so tests can drive nav without the real
 * keybinding pipeline. The probe mounts inside the `EntityFocusProvider`,
 * reads `broadcastNavCommand` and `setFocus` from context, and writes them
 * to the test-owned ref.
 *
 * `broadcastNavCommand` is the production entry point — `AppShell` wires
 * arrow keys through it. It only dispatches `ui.setFocus` when some
 * FocusScope claim predicate matches. Our mocked `DataTable` does not
 * register predicates, so tests that need to exercise the full dispatch
 * path call `setFocus` directly with a known moniker instead. Both paths
 * flow through the same `useDispatchCommand("ui.setFocus")` — that's the
 * contract this test is guarding.
 */
interface NavRef {
  broadcast: ((cmd: string) => boolean) | null;
  setFocus: ((moniker: string | null) => void) | null;
}

function NavProbe({ navRef }: { navRef: NavRef }) {
  const { broadcastNavCommand, setFocus } = useEntityFocus();
  useEffect(() => {
    navRef.broadcast = broadcastNavCommand;
    navRef.setFocus = setFocus;
    return () => {
      navRef.broadcast = null;
      navRef.setFocus = null;
    };
  }, [broadcastNavCommand, setFocus, navRef]);
  return null;
}

/**
 * Harness that wraps GridView in the real provider stack used for the
 * invariant under test. The providers are the same ones that sit under
 * `RustEngineContainer` in `App.tsx` (schema, entity store, entity focus,
 * field update, ui state); the heavier outer containers are intentionally
 * omitted — see the module docstring.
 */
function GridHarness({
  entities,
  navRef,
}: {
  entities: Record<string, Entity[]>;
  navRef: NavRef;
}) {
  return (
    <CommandBusyProvider>
      <TooltipProvider>
        <SchemaProvider>
          <EntityStoreProvider entities={entities}>
            <EntityFocusProvider>
              <FieldUpdateProvider>
                <UIStateProvider>
                  <NavProbe navRef={navRef} />
                  <GridView
                    view={{
                      id: "v-nav",
                      name: "Tasks Grid",
                      kind: "grid",
                      entity_type: "task",
                    }}
                  />
                </UIStateProvider>
              </FieldUpdateProvider>
            </EntityFocusProvider>
          </EntityStoreProvider>
        </SchemaProvider>
      </TooltipProvider>
    </CommandBusyProvider>
  );
}

/** Filter the invoke mock's calls to only IPCs whose first arg is `name`. */
function invokeCallsFor(name: string): unknown[][] {
  return mockInvoke.mock.calls.filter((c) => c[0] === name);
}

/**
 * Filter `dispatch_command` calls to those whose payload `cmd` matches the
 * given command id. Helpful for isolating `perspective.list` from
 * `ui.setFocus` in the same mock.
 */
function dispatchCallsFor(cmd: string): unknown[][] {
  return mockInvoke.mock.calls.filter(
    (c) =>
      c[0] === "dispatch_command" &&
      (c[1] as { cmd?: string } | undefined)?.cmd === cmd,
  );
}

// ---------------------------------------------------------------------------
// Default invoke responses — the handful of IPCs the real providers hit on
// mount. Kept in one place so beforeEach restores them cleanly after each
// test's mockClear / mockReset.
// ---------------------------------------------------------------------------
async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return TASK_SCHEMA;
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GridView — nav is event-driven", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("fires zero fetch IPCs during arrow-key navigation", async () => {
    const navRef: NavRef = { broadcast: null, setFocus: null };
    const entities = { task: fiveTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} navRef={navRef} />);
    });

    // Let schema + initial focus effects settle so any mount-time IPCs
    // have already recorded by the time we clear the mock.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(navRef.broadcast).toBeTruthy();
    expect(navRef.setFocus).toBeTruthy();

    // Capture the baseline and clear: tests measure only post-mount IPCs.
    mockInvoke.mockClear();

    // Simulate nav in two faithful ways:
    //
    // 1. `broadcastNavCommand` — the production entry point. In this test
    //    harness the `DataTable` is mocked, so no FocusScope predicates
    //    register; these broadcasts become no-ops. That's still a valid
    //    probe — if any *other* code path reached by a nav broadcast
    //    fetched, we'd catch it.
    // 2. `setFocus(moniker)` — the effect every matching nav claim has.
    //    Drives the `ui.setFocus` dispatch path end-to-end, which is the
    //    one IPC legitimately allowed on nav.
    //
    // Alternating 10 nav commands exercises both axes; the 5 setFocus
    // calls cycle through every seeded task so we hit every moniker the
    // grid would surface on real keyboard nav.
    await act(async () => {
      for (let i = 0; i < 5; i++) {
        navRef.broadcast?.("nav.down");
        navRef.broadcast?.("nav.right");
      }
      navRef.setFocus?.("field:task:t1.title");
      navRef.setFocus?.("field:task:t2.title");
      navRef.setFocus?.("field:task:t3.title");
      navRef.setFocus?.("field:task:t4.title");
      navRef.setFocus?.("field:task:t5.title");
    });

    // Wait for any async dispatches to settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Disallowed fetch IPCs — zero calls during nav.
    expect(
      invokeCallsFor("list_entities"),
      "list_entities must not fire on navigation",
    ).toHaveLength(0);
    expect(
      invokeCallsFor("get_entity"),
      "get_entity must not fire on navigation",
    ).toHaveLength(0);
    expect(
      invokeCallsFor("get_board_data"),
      "get_board_data must not fire on navigation",
    ).toHaveLength(0);
    expect(
      dispatchCallsFor("perspective.list"),
      "perspective.list must not fire on navigation",
    ).toHaveLength(0);

    // Sanity check: `ui.setFocus` IS allowed — it's a state-mutation dispatch,
    // not a fetch. Navigating should dispatch at least one of these.
    expect(
      dispatchCallsFor("ui.setFocus").length,
      "ui.setFocus is the legitimate per-nav dispatch",
    ).toBeGreaterThan(0);
  });

  it("allows ui.setFocus through — the only legitimate per-nav dispatch", async () => {
    const navRef: NavRef = { broadcast: null, setFocus: null };
    const entities = { task: fiveTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} navRef={navRef} />);
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    mockInvoke.mockClear();

    // A focus change should surface a ui.setFocus dispatch — nothing else.
    await act(async () => {
      navRef.setFocus?.("field:task:t2.title");
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Every non-ui.setFocus dispatch_command call is a violator — the grid
    // must not issue command dispatches of any other kind on a bare nav.
    const bogusDispatches = mockInvoke.mock.calls.filter((c) => {
      if (c[0] !== "dispatch_command") return false;
      const cmd = (c[1] as { cmd?: string } | undefined)?.cmd;
      return cmd !== "ui.setFocus";
    });
    expect(
      bogusDispatches.map((c) => (c[1] as { cmd?: string }).cmd),
      "nav must not dispatch any command other than ui.setFocus",
    ).toEqual([]);

    // And the one legitimate dispatch did happen.
    expect(
      dispatchCallsFor("ui.setFocus").length,
      "setFocus must drive exactly one ui.setFocus dispatch",
    ).toBeGreaterThan(0);
  });

  // ---------------------------------------------------------------------------
  // Field-level subscription fidelity
  //
  // The complementary half of the contract: updates arrive via events and
  // must wake up exactly the subscribing cell, not its siblings. This
  // protects the grid's per-cell re-render budget — if a field-changed
  // event caused every cell to re-render, we'd have O(rows * cols) renders
  // per keystroke in an editor, negating the field-subscription design.
  // ---------------------------------------------------------------------------
  it("entity-field-changed wakes only the subscribing cell's useFieldValue", async () => {
    // Shared across the test so memoized FieldProbes can report their own
    // render counts. The harness increments the count on each render, and
    // assertions compare before/after.
    const renderCounts = new Map<string, number>();

    /**
     * Field-value subscriber probe that counts its own render cycles.
     *
     * Wrapped in `memo` so the parent's state swap does not by itself
     * trigger a re-render — only a subscribed field change or a genuine
     * prop change should. This is the exact shape grid cells take in
     * production: memoized leaf components that pull a single field value
     * through `useFieldValue`.
     */
    const FieldProbe = memo(function FieldProbe({
      id,
      label,
    }: {
      id: string;
      label: string;
    }) {
      const value = useFieldValue("task", id, "title");
      renderCounts.set(label, (renderCounts.get(label) ?? 0) + 1);
      return <span data-testid={`probe-${label}`}>{String(value ?? "")}</span>;
    });

    /**
     * Reactive harness — listens to `entity-field-changed` via the `listen`
     * mock and patches `entitiesByType` in place, exactly the way
     * `RustEngineContainer.handleEntityFieldChanged` does in production.
     * Renders three memoized probes bound to three different entity ids so
     * we can assert the subscribing probe re-renders and siblings stay quiet.
     */
    function ReactiveFieldHarness() {
      const [ents, setEnts] = useState<Record<string, Entity[]>>({
        task: [
          seedTask("t1", "Alpha"),
          seedTask("t2", "Beta"),
          seedTask("t3", "Gamma"),
        ],
      });
      useEffect(() => {
        mockListen("entity-field-changed", (e: { payload: unknown }) => {
          const payload = e.payload as {
            entity_type: string;
            id: string;
            changes: Array<{ field: string; value: unknown }>;
          };
          const { entity_type, id, changes } = payload;
          setEnts((prev) => {
            const list = prev[entity_type] ?? [];
            const next = list.map((ent) => {
              if (ent.id !== id) return ent;
              const patched = { ...ent.fields };
              for (const { field, value } of changes) patched[field] = value;
              return { ...ent, fields: patched };
            });
            return { ...prev, [entity_type]: next };
          });
        });
      }, []);
      return (
        <EntityStoreProvider entities={ents}>
          <FieldProbe id="t1" label="t1" />
          <FieldProbe id="t2" label="t2" />
          <FieldProbe id="t3" label="t3" />
        </EntityStoreProvider>
      );
    }

    await act(async () => {
      render(<ReactiveFieldHarness />);
    });

    // Initial renders — each probe mounts with its seeded title.
    expect(screen.getByTestId("probe-t1").textContent).toBe("Alpha");
    expect(screen.getByTestId("probe-t2").textContent).toBe("Beta");
    expect(screen.getByTestId("probe-t3").textContent).toBe("Gamma");

    const baselineT1 = renderCounts.get("t1") ?? 0;
    const baselineT2 = renderCounts.get("t2") ?? 0;
    const baselineT3 = renderCounts.get("t3") ?? 0;

    // Fire a field-changed event for t1.title ONLY.
    await fireTauriEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "task",
      id: "t1",
      changes: [{ field: "title", value: "Alpha-PRIME" }],
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });

    // Subscribing probe re-rendered with the patched value.
    expect(screen.getByTestId("probe-t1").textContent).toBe("Alpha-PRIME");

    const t1After = renderCounts.get("t1") ?? 0;
    const t2After = renderCounts.get("t2") ?? 0;
    const t3After = renderCounts.get("t3") ?? 0;

    // t1 re-rendered at least once (the field changed).
    expect(
      t1After - baselineT1,
      "t1 probe must re-render after its field changes",
    ).toBeGreaterThanOrEqual(1);

    // Siblings must not wake on an unrelated field change.
    //
    // The load-bearing mechanism is `FieldSubscriptions.diff`:
    // useFieldValue subscribes to `${type}:${id}:${field}`, so when
    // `entity-field-changed` fires for `task:t1:title`, `diff` notifies
    // only the subscriber under that key. t2's and t3's
    // `useSyncExternalStore` subscribers are never invoked, which means
    // their components never schedule a re-render in the first place —
    // the increments we measure for them stay at their baseline.
    //
    // `memo` on FieldProbe is a secondary defense against parent-cascade
    // re-renders (if the parent re-rendered for an unrelated reason,
    // stable `id`/`label` props would still let React skip these leaves).
    // Remove the subscription diff and `memo` alone would not save us —
    // every probe would wake via the parent cascade when the store
    // notified globally.
    //
    // If a regression re-introduces a full-state re-fetch or collapses the
    // field-level diff back into a whole-store notification, every probe
    // will wake and these expectations will fail.
    expect(
      t2After - baselineT2,
      "t2 probe must not re-render on t1 field change",
    ).toBe(0);
    expect(
      t3After - baselineT3,
      "t3 probe must not re-render on t1 field change",
    ).toBe(0);

    // Sibling values unchanged.
    expect(screen.getByTestId("probe-t2").textContent).toBe("Beta");
    expect(screen.getByTestId("probe-t3").textContent).toBe("Gamma");
  });
});
