/**
 * Spatial-nav integration tests for `<GridView>` (browser-mode).
 *
 * Mounts the grid inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the conditional
 * `<GridSpatialZone>` lights up its `<FocusScope moniker={asSegment("ui:grid")}>`
 * branch, and the per-cell `<GridCellFocusable>` lights up its `<FocusScope>`
 * leaf branch (after the architecture-fix card collapsed the leaf primitive
 * onto `<FocusScope>`). The Tauri `invoke` and `listen` boundaries are mocked at
 * the module level so we can:
 *
 *   - Inspect every `spatial_register_scope` / `spatial_register_scope` call
 *     each primitive makes on mount.
 *   - Drive synthetic `focus-changed` payloads through the captured `listen`
 *     callback to simulate the Rust kernel asserting focus on a specific
 *     `FullyQualifiedMoniker`. The provider's listener fans out to per-key claim
 *     callbacks and broad `subscribeFocusChanged` subscribers (which the
 *     `EntityFocusProvider` bridge uses to mirror `next_segment` into the
 *     entity-focus store, driving the `data-cell-cursor` ring).
 *
 * Asserts the contract from kanban task `01KNQXZZ9VQBHFX091P0K4F4YC`:
 *
 *   1. Registration (zone) — exactly one `ui:grid` zone is registered
 *      with a layer key and (optional) parent zone.
 *   2. Cell registration (per cell) — every visible cell registers as a
 *      `<FocusScope>` leaf with `grid_cell:R:K` shape. Each cell focusable's
 *      `parentZone` is the `ui:grid` zone's key.
 *   3. Click cell → focus — clicking a cell triggers exactly one
 *      `spatial_focus` for THAT cell's key and does NOT also fire for the
 *      enclosing zone (leaf `stopPropagation` keeps the click local).
 *   4. Focus claim → no zone bar but cell has cursor ring — driving
 *      `focus-changed` to the grid zone flips its `data-focused` but mounts
 *      no `<FocusIndicator>` (zone-suppressed); driving it to a cell's key
 *      mounts the `<FocusIndicator>` inside that cell.
 *   5. Keystrokes → navigate — deferred per the card's AC #5 (owned by
 *      follow-up `01KNQY1GQ9...`); assertion below pins the precondition
 *      that each cell has a stable `FullyQualifiedMoniker` ready to be passed to
 *      `spatial_navigate` once arrow-key nav lands.
 *   6. Unmount — every registered zone / cell key reaches
 *      `spatial_unregister_scope` and the `focus-changed` listener slot
 *      empties on teardown.
 *   7. Legacy nav stripped — no `entity_focus_*`, `claim_when_*`, or
 *      `broadcast_nav_*` IPCs are dispatched at any point.
 *
 * Plus per-component additions:
 *
 *   - Cell-as-FocusScope — each cell carries both `[data-segment]` and the
 *     `data-focused` attribute slot (proves it's a `<FocusScope>` leaf, not
 *     a bare `<div>`).
 *   - Rect-update count — re-rendering with the same data does not produce
 *     duplicate `spatial_register_*` calls per cell.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks -- must come before component imports.
//
// The hoisted bag captures `mockInvoke` (every IPC the providers fire) and
// `mockListen` (every `listen("event", cb)` callback) plus a `listeners`
// map keyed by event name. Tests drive `focus-changed` events by reaching
// into `listeners.get("focus-changed")` and invoking each registered
// callback — the same shape `grid-view.nav-is-eventdriven.test.tsx` and
// `perspective-bar.spatial.test.tsx` use.
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

// Stub the perspective container so the grid gets a stable activePerspective
// without dragging in heavier providers.
vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// ---------------------------------------------------------------------------
// Imports after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import {
  asSegment,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type WindowLabel,
} from "@/types/spatial";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Task schema -- two columns so we exercise the grid_cell:R:K shape with
// distinct column keys.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    search_display_field: "title",
  },
  fields: [
    { name: "title", type: "string", section: "header", display: "text" },
    { name: "status", type: "string", section: "header", display: "text" },
  ],
} as unknown as EntitySchema;

function seedTask(id: string, title: string, status: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: { title, status },
  };
}

function threeTasks(): Entity[] {
  return [
    seedTask("t1", "Alpha", "todo"),
    seedTask("t2", "Beta", "doing"),
    seedTask("t3", "Gamma", "done"),
  ];
}

/**
 * Mount `GridView` inside the production-shaped provider stack with the
 * spatial-nav layer present so `<GridSpatialZone>` and `<GridCellFocusable>`
 * both light up.
 */
function GridHarness({ entities }: { entities: Record<string, Entity[]> }) {
  return (
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <TooltipProvider>
            <SchemaProvider>
              <EntityStoreProvider entities={entities}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <GridView
                        view={{
                          id: "v-spatial",
                          name: "Tasks",
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
        </FocusLayer>
      </SpatialFocusProvider>
    </CommandBusyProvider>
  );
}

/** Default invoke responses for the mount-time IPCs the providers fire. */
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
  if (cmd === "list_commands_for_scope") return [];
  return undefined;
}

/** Collect every `spatial_register_scope` call payload. */
function registerScopeCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_unregister_scope` call payload. */
function unregisterScopeCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_focus` call payload. */
function spatialFocusCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as Record<string, unknown>);
}

/**
 * Wait for register effects scheduled inside `useEffect` to flush. The
 * primitives mint their `FullyQualifiedMoniker` and invoke `spatial_register_*` from
 * a mount-effect, so the calls don't land on the mock until React has
 * committed and run effects. A `setTimeout(0)` round-trip is sufficient
 * — the providers don't await any further async chain after registration.
 */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Drive a `focus-changed` event into the React tree as if the Rust kernel
 * had emitted one for the current window.
 *
 * Wraps the dispatch in `act()` so React state updates (per-key claim
 * subscribers, broad `subscribeFocusChanged` subscribers) are flushed
 * before the caller asserts against post-update DOM.
 */
async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
  next_segment = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
  next_segment?: string | null;
}) {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment: next_segment === null ? null : asSegment(next_segment),
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GridView (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // 1. Registration (zone)
  // -------------------------------------------------------------------------

  it("registers exactly one ui:grid zone at the grid root", async () => {
    const entities = { task: threeTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const calls = registerScopeCalls();
    const gridZones = calls.filter((c) => c.segment === "ui:grid");
    expect(gridZones.length).toBe(1);

    // Zone must be inside a layer (production layer key) and carry a
    // minted FullyQualifiedMoniker suitable for use as the cells' `parentZone`.
    expect(gridZones[0].layerFq).toBeTruthy();
    expect(typeof gridZones[0].fq).toBe("string");
    expect((gridZones[0].fq as string).length).toBeGreaterThan(0);
  });

  it("emits a wrapper element with data-moniker='ui:grid'", async () => {
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const node = result.container.querySelector("[data-segment='ui:grid']");
    expect(node).not.toBeNull();
  });

  // -------------------------------------------------------------------------
  // 2. Cell registration (per cell)
  // -------------------------------------------------------------------------

  it("registers each cell as a FocusScope leaf with grid_cell:R:K moniker", async () => {
    const entities = { task: threeTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const focusableCalls = registerScopeCalls();
    const cellMonikers = focusableCalls
      .map((c) => c.segment)
      .filter(
        (m): m is string => typeof m === "string" && m.startsWith("grid_cell:"),
      );

    // 3 rows × 2 columns = 6 cell focusables. Use a Set-based assertion so
    // the test is not sensitive to registration order — what matters is the
    // identity of each cell, not the sequence.
    expect(new Set(cellMonikers)).toEqual(
      new Set([
        "grid_cell:0:title",
        "grid_cell:0:status",
        "grid_cell:1:title",
        "grid_cell:1:status",
        "grid_cell:2:title",
        "grid_cell:2:status",
      ]),
    );
  });

  it("registers cell focusables with parentZone = the row's FQM under the ui:grid zone", async () => {
    const entities = { task: threeTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const zoneCalls = registerScopeCalls();
    const gridZone = zoneCalls.find((c) => c.segment === "ui:grid");
    expect(gridZone).toBeTruthy();
    const gridZoneKey = gridZone!.fq as string;
    expect(gridZoneKey).toBeTruthy();

    const focusableCalls = registerScopeCalls();
    const cellFocusables = focusableCalls.filter(
      (c) =>
        typeof c.segment === "string" &&
        (c.segment as string).startsWith("grid_cell:"),
    );
    expect(cellFocusables.length).toBeGreaterThan(0);

    // Every cell focusable must point its parentZone at the row Zone
    // — the row mounts a `<FocusScope moniker={asSegment(entityMk)}
    // renderContainer={false}>` which publishes its FQM via
    // `FocusScopeContext.Provider`. Cell `useParentFocusScope()` therefore
    // resolves to the row, not the surrounding `ui:grid` zone. The
    // composed shape is `<gridZoneKey>/task:<id>`. The row Zone uses
    // `renderContainer={false}` and so does NOT register a rect with
    // the kernel itself — but its FQM is still the legal nearest-zone
    // ancestor for cell registrations under it (lifting the
    // scope-is-leaf restriction the row's previous `<FocusScope>`
    // wrapper would have triggered now that `<FocusScope
    // renderContainer={false}>` ALSO publishes its FQM through the
    // path-prefix branch).
    for (const cell of cellFocusables) {
      const parentZone = cell.parentZone as string;
      expect(parentZone).toBeTruthy();
      expect(parentZone.startsWith(`${gridZoneKey}/task:`)).toBe(true);
    }
  });

  // -------------------------------------------------------------------------
  // 3. Click cell → focus
  // -------------------------------------------------------------------------

  it("clicking a cell dispatches exactly one spatial_focus for THAT cell's key (not the zone's)", async () => {
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    // Capture the bar key + the target cell's key from the registration calls.
    const gridZone = registerScopeCalls().find((c) => c.segment === "ui:grid");
    expect(gridZone).toBeTruthy();
    const gridZoneKey = gridZone!.fq;

    const targetMoniker = "grid_cell:1:status";
    const targetCell = registerScopeCalls().find(
      (c) => c.segment === targetMoniker,
    );
    expect(targetCell).toBeTruthy();
    const targetCellKey = targetCell!.fq;

    // Reset invoke before the click so we measure only the click's IPC. The
    // `mockClear` does not affect the `listeners` map, so the SpatialFocusProvider's
    // `focus-changed` listener stays registered.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    // Locate the cell's `<FocusScope>` leaf. It carries the canonical
    // `[data-segment="grid_cell:R:K"]` selector.
    const cellNode = result.container.querySelector(
      `[data-segment='${targetMoniker}']`,
    ) as HTMLElement | null;
    expect(cellNode).not.toBeNull();

    // Click an element INSIDE the cell's subtree — that's where the inner
    // click bridge lives in the spatial path. React's bubble order: target
    // → inner div onClick (legacy entity-focus optimistic update) →
    // FocusScope's outer onClick (`spatial_focus` + `stopPropagation`).
    // The test asserts on the spatial path; the inner-div bridge is an
    // optimistic update that does not perturb the IPC count we measure.
    //
    // Wrap in `act` so React flushes the state updates the click triggers
    // (the CommandBusyProvider transitions on `dispatch_command`).
    await act(async () => {
      fireEvent.click(cellNode!.firstElementChild ?? cellNode!);
      await Promise.resolve();
    });

    // Exactly one `spatial_focus` call, addressed to the cell's key.
    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe(targetCellKey);

    // The grid zone key must NOT also receive a focus call — the leaf
    // stops propagation so the click does not bubble to the wrapping zone.
    expect(focusCalls.find((c) => c.fq === gridZoneKey)).toBeUndefined();
  });

  // -------------------------------------------------------------------------
  // 4. Focus claim → no zone bar but cell has cursor ring
  // -------------------------------------------------------------------------

  it("focus claim on the grid zone flips data-focused but renders no FocusIndicator", async () => {
    // The grid zone uses `showFocus={false}` because a focus bar around
    // the entire grid body would be visual noise — every cell already has
    // its own bar that drives the visible focus decoration. The
    // `data-focused` attribute still flips so e2e selectors and debugging
    // tooling can observe the claim, but no `<FocusIndicator>` mounts.
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const gridZone = registerScopeCalls().find((c) => c.segment === "ui:grid");
    expect(gridZone).toBeTruthy();
    const gridZoneKey = gridZone!.fq as FullyQualifiedMoniker;

    const gridNode = result.container.querySelector(
      "[data-segment='ui:grid']",
    ) as HTMLElement | null;
    expect(gridNode).not.toBeNull();

    // Drive a `focus-changed` payload claiming the grid zone's key.
    await fireFocusChanged({
      next_fq: gridZoneKey,
      next_segment: asSegment("ui:grid"),
    });

    // `data-focused` flips on the zone but no `<FocusIndicator>` is
    // mounted on it (zone-suppressed via `showFocus={false}`). The
    // grid's status bar / scroll container have no FocusIndicator either.
    await waitFor(() => {
      expect(gridNode!.getAttribute("data-focused")).not.toBeNull();
    });

    // No FocusIndicator anywhere in the grid — the indicator would only
    // appear once focus moves to a cell (next test).
    const indicators = result.container.querySelectorAll(
      "[data-testid='focus-indicator']",
    );
    expect(indicators.length).toBe(0);
  });

  it("focus claim on a cell mounts the FocusIndicator inside that cell", async () => {
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const targetMoniker = "grid_cell:1:status";
    const targetCell = registerScopeCalls().find(
      (c) => c.segment === targetMoniker,
    );
    expect(targetCell).toBeTruthy();
    const targetCellKey = targetCell!.fq as FullyQualifiedMoniker;

    // Drive `focus-changed` on the target cell. The provider's listener
    // fires the cell's `useFocusClaim` callback (flips `data-focused`)
    // AND the broad `subscribeFocusChanged` subscribers. The
    // `EntityFocusProvider` bridge is one such subscriber: it mirrors
    // `next_segment` into the entity-focus store so the cursor ring
    // (derived from `focusedMoniker`) updates to point at this cell.
    await fireFocusChanged({
      next_fq: targetCellKey,
      next_segment: targetMoniker,
    });

    // After the claim flips, the FocusIndicator renders inside the
    // matching cell — the cell's own React state observes the claim and
    // mounts the bar. Use `waitFor` because the React commit happens
    // asynchronously after the listener fires.
    await waitFor(() => {
      const indicators = result.container.querySelectorAll(
        "[data-testid='focus-indicator']",
      );
      expect(indicators.length).toBe(1);
    });

    const cellNode = result.container.querySelector(
      `[data-segment='${targetMoniker}']`,
    ) as HTMLElement | null;
    expect(cellNode).not.toBeNull();

    const indicator = result.container.querySelector(
      "[data-testid='focus-indicator']",
    )!;
    // The indicator's host is the focused cell — proves the bar mounts
    // inside the leaf, not on a sibling element.
    expect(cellNode!.contains(indicator)).toBe(true);
    expect(cellNode!.getAttribute("data-focused")).not.toBeNull();
  });

  it("the cursor ring (data-cell-cursor) tracks focused cell across spatial-focus events", async () => {
    // End-to-end of the bridge from spatial-focus events to entity-focus.
    // The `EntityFocusProvider` subscribes to `subscribeFocusChanged` and
    // mirrors `payload.next_segment` into the legacy entity-focus store.
    // The grid's `gridCellCursor` is derived from that store, and the
    // matching cell stamps `data-cell-cursor`. This test pins the
    // contract that focusing a cell via the kernel's `focus-changed`
    // event lights up the cursor ring without any direct entity-focus
    // mutation from the click handler.
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const targetMoniker = "grid_cell:2:title";
    const targetCell = registerScopeCalls().find(
      (c) => c.segment === targetMoniker,
    );
    expect(targetCell).toBeTruthy();
    const targetCellKey = targetCell!.fq as FullyQualifiedMoniker;

    // Drive focus to the target cell via the spatial event path only
    // (no click, no direct setFocus call).
    await fireFocusChanged({
      next_fq: targetCellKey,
      next_segment: targetMoniker,
    });

    // The matching cell stamps `data-cell-cursor` once the bridge
    // updates the entity-focus store. Use `waitFor` to allow React to
    // commit the derived state.
    await waitFor(() => {
      const ringedCells =
        result.container.querySelectorAll("[data-cell-cursor]");
      expect(ringedCells.length).toBe(1);
      expect(
        (ringedCells[0] as HTMLElement).getAttribute("data-cell-cursor"),
      ).toBe("2:title");
    });
  });

  // -------------------------------------------------------------------------
  // 5. Keystrokes → navigate
  //
  // Per the card's AC #5, arrow-key navigation in the grid is deferred to
  // the follow-up `01KNQY1GQ9...`. The grid view itself is forbidden from
  // owning a `keydown` listener (enforced by
  // `grid-spatial-nav.guards.node.test.ts`). The keystroke path is the
  // global keymap pipeline in `<AppShell>`: `nav.up` / `nav.down` /
  // `nav.left` / `nav.right` (mapped to `j/k/h/l`, arrows) plus
  // `nav.first` / `nav.last` (Home / End).
  //
  // The cell-side precondition the grid CAN guarantee — and that the
  // follow-up will rely on — is that each cell registers a stable
  // `FullyQualifiedMoniker` in the spatial graph that `spatial_navigate` can be
  // dispatched against. The assertion below pins that precondition by
  // checking the registration shape (key + moniker + parentZone) is
  // valid for every visible cell.
  // -------------------------------------------------------------------------

  it("each cell's FullyQualifiedMoniker is registered with a complete shape ready for spatial_navigate", async () => {
    const entities = { task: threeTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const gridZone = registerScopeCalls().find((c) => c.segment === "ui:grid")!;
    const gridZoneKey = gridZone.fq as FullyQualifiedMoniker;
    const cellRegistrations = registerScopeCalls().filter(
      (c) =>
        typeof c.segment === "string" &&
        (c.segment as string).startsWith("grid_cell:"),
    );
    expect(cellRegistrations.length).toBe(6);

    // Each cell must have:
    //   - a non-empty FullyQualifiedMoniker (the argument `spatial_navigate` would receive)
    //   - the canonical `grid_cell:R:K` moniker
    //   - `parentZone` pointing at the row Zone (a path-descendant of
    //     the `ui:grid` zone — the row's outer `<FocusScope
    //     renderContainer={false}>` publishes its FQM through
    //     `FocusScopeContext`, so `useParentFocusScope()` lands on the row
    //     entity rather than skipping past it to `ui:grid`). Beam
    //     search still stays inside the grid because the row is itself
    //     a path-descendant of `ui:grid`.
    //   - a layer key (so the kernel knows which modal layer the cell lives in)
    const gridZoneKeyStr = gridZoneKey as string;
    for (const cell of cellRegistrations) {
      expect(typeof cell.fq).toBe("string");
      expect((cell.fq as string).length).toBeGreaterThan(0);
      expect(cell.segment).toMatch(/^grid_cell:[0-9]+:[a-z_]+$/);
      const parentZone = cell.parentZone as string;
      expect(parentZone.startsWith(`${gridZoneKeyStr}/task:`)).toBe(true);
      expect(cell.layerFq).toBe(gridZone.layerFq);
    }
  });

  // -------------------------------------------------------------------------
  // 6. Unmount — no listener leaks
  // -------------------------------------------------------------------------

  it("unmounting unregisters the zone and every cell key (no listener leaks)", async () => {
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    // Snapshot the keys we expect to be unregistered.
    const gridZone = registerScopeCalls().find((c) => c.segment === "ui:grid");
    expect(gridZone).toBeTruthy();
    const gridZoneKey = gridZone!.fq;

    const cellRegistrations = registerScopeCalls().filter(
      (c) =>
        typeof c.segment === "string" &&
        (c.segment as string).startsWith("grid_cell:"),
    );
    const cellKeys = cellRegistrations.map((c) => c.fq as string);
    expect(cellKeys.length).toBe(6);

    // Listener slot has at least one entry (the SpatialFocusProvider's
    // global `focus-changed` listener) before unmount.
    const beforeUnmount = listeners.get("focus-changed")?.length ?? 0;
    expect(beforeUnmount).toBeGreaterThan(0);

    // Tear down. Wrap in act() so React's cleanup-effect chain runs.
    await act(async () => {
      result.unmount();
    });
    await flushSetup();

    const unregisterKeys = unregisterScopeCalls().map((c) => c.fq as string);

    // The grid zone key reaches `spatial_unregister_scope`. (The Rust
    // kernel deletes both `Zone` and `Scope` entries through the same
    // command — there is no separate `spatial_unregister_scope`.)
    expect(unregisterKeys).toContain(gridZoneKey);

    // Every cell key reaches `spatial_unregister_scope`.
    for (const key of cellKeys) {
      expect(unregisterKeys).toContain(key);
    }

    // The `focus-changed` listener slot empties on teardown. The
    // `<SpatialFocusProvider>` registers the global listener once on
    // mount and the cleanup effect calls the unlisten function we
    // captured in `mockListen`, which removes the entry from the
    // `listeners` map. A non-empty slot here would indicate a leaked
    // listener — every focus change for the rest of the process would
    // call into the now-stale closure references.
    const afterUnmount = listeners.get("focus-changed")?.length ?? 0;
    expect(afterUnmount).toBe(0);
  });

  // -------------------------------------------------------------------------
  // 7. Legacy nav stripped
  // -------------------------------------------------------------------------

  it("emits no entity_focus_*, claim_when_*, or broadcast_nav_* IPCs at any point", async () => {
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    // Click a cell so the click → spatial_focus path runs end-to-end.
    const cellNode = result.container.querySelector(
      "[data-segment='grid_cell:0:title']",
    ) as HTMLElement | null;
    expect(cellNode).not.toBeNull();
    await act(async () => {
      fireEvent.click(cellNode!.firstElementChild ?? cellNode!);
      await Promise.resolve();
    });

    // Drive a focus-changed event so the bridge fires too.
    const targetCell = registerScopeCalls().find(
      (c) => c.segment === "grid_cell:0:title",
    )!;
    await fireFocusChanged({
      next_fq: targetCell.fq as FullyQualifiedMoniker,
      next_segment: asSegment("grid_cell:0:title"),
    });

    // The legacy pull-based nav stack used `claim_when_*` predicates and
    // `broadcast_nav_*` events; the legacy entity-focus IPC family began
    // with `entity_focus_*`. None of those should appear in the IPC log
    // — the spatial-nav kernel is push-based and the grid no longer has
    // a per-cell claim or broadcast registration.
    const banned = /^(entity_focus_|claim_when_|broadcast_nav_)/;
    const offenders = mockInvoke.mock.calls
      .map((c) => c[0])
      .filter((cmd) => typeof cmd === "string" && banned.test(cmd));
    expect(offenders).toEqual([]);
  });

  // -------------------------------------------------------------------------
  // Per-component additions
  // -------------------------------------------------------------------------

  it("each cell carries [data-segment] and the data-focused attribute slot (Cell-as-FocusScope)", async () => {
    // Proves each cell's wrapping primitive is a real `<FocusScope>` leaf,
    // not a bare `<div>`. The `<FocusScope>` body always renders the
    // `data-moniker` attribute, and the `data-focused` slot is present
    // (as `null` when unfocused) — driving `focus-changed` flips it to
    // `"true"`. This is the canonical shape consumed by the spatial-nav
    // kernel's e2e selectors and debugging tooling.
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    // Grab every grid_cell DOM node via the data-moniker selector — the
    // `<FocusScope>` body is the only place this attribute is emitted.
    const cellNodes = Array.from(
      result.container.querySelectorAll<HTMLElement>(
        "[data-segment^='grid_cell:']",
      ),
    );
    // 3 rows × 2 cols = 6 cells.
    expect(cellNodes.length).toBe(6);

    for (const node of cellNodes) {
      // `data-segment` matches the canonical relative-segment wire shape.
      const moniker = node.getAttribute("data-segment") ?? "";
      expect(moniker).toMatch(/^grid_cell:[0-9]+:[a-z_]+$/);
      // `data-focused` slot exists in the React element shape (the
      // primitive emits `data-focused={focused || undefined}`, so the
      // attribute is absent when unfocused — but the React element
      // ALWAYS has the attribute slot, which is the key invariant). We
      // assert by flipping focus to one cell and re-reading the
      // attribute on it.
    }

    // Drive focus to a specific cell and assert its `data-focused`
    // attribute toggles. This exercises the same `useFocusClaim` →
    // React state → `data-focused` toggle path the indicator uses.
    const targetMoniker = cellNodes[0].getAttribute("data-segment")!;
    const targetCell = registerScopeCalls().find(
      (c) => c.segment === targetMoniker,
    )!;
    await fireFocusChanged({
      next_fq: targetCell.fq as FullyQualifiedMoniker,
      next_segment: targetMoniker,
    });

    await waitFor(() => {
      // The focused cell now carries `data-focused`; sibling cells do
      // not. (The DOM attribute is `"true"` when present, absent when
      // not — both branches of `data-focused={focused || undefined}`.)
      expect(cellNodes[0].getAttribute("data-focused")).not.toBeNull();
    });
    for (let i = 1; i < cellNodes.length; i++) {
      expect(cellNodes[i].getAttribute("data-focused")).toBeNull();
    }
  });

  it("re-rendering with the same data does not emit duplicate spatial_register_* calls per cell", async () => {
    // Stable cell `FullyQualifiedMoniker`s are critical: every duplicate
    // `spatial_register_scope` call would mint a fresh key in the
    // kernel registry under the same moniker, leaving the previous
    // entry orphaned (a beam-search dead-end) and inflating the
    // ResizeObserver count. The cell mints its key in a `useRef`, so
    // re-rendering with identical props must not produce any new
    // register calls.
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await flushSetup();

    // Snapshot the registration count after the initial mount.
    const initialZoneRegistrations = registerScopeCalls().length;
    const initialScopeRegistrations = registerScopeCalls().length;
    expect(initialScopeRegistrations).toBeGreaterThanOrEqual(6);

    // Re-render with the SAME entities reference — React commits but the
    // cell `<FocusScope>` mount-effects do not refire (the key ref stays
    // alive, the moniker is identical, and the layer/parent are stable),
    // so no fresh `spatial_register_*` calls should land.
    await act(async () => {
      result.rerender(<GridHarness entities={entities} />);
    });
    await flushSetup();

    const afterRerenderZoneRegistrations = registerScopeCalls().length;
    const afterRerenderScopeRegistrations = registerScopeCalls().length;

    expect(afterRerenderZoneRegistrations).toBe(initialZoneRegistrations);
    expect(afterRerenderScopeRegistrations).toBe(initialScopeRegistrations);
  });
});
