/**
 * Spatial-nav integration tests for `<ColumnView>`.
 *
 * Mounts a column inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the column's
 * `<FocusScope>` and the inner column-name-field `<FocusScope>`
 * register through the live spatial primitives. The Tauri `invoke` boundary
 * is mocked at the module level so we can inspect the `spatial_register_scope`
 * and `spatial_register_scope` calls each emits on mount.
 *
 * Companion file: `column-view.guards.node.test.ts` pins the source-level
 * invariants (no `ClaimPredicate` import, no neighbor-moniker plumbing, no
 * column-level keydown listener). This file pins the runtime contract:
 *
 *   - The column body registers as a zone with moniker `column:{id}`.
 *   - Its `parentZone` is the surrounding `<FocusScope>` (e.g. `ui:board`)
 *     when one is present, and `null` when the column is mounted directly
 *     under the layer root.
 *   - The column header registers as a leaf with `parentZone` equal to the
 *     column's zone key.
 *   - Each task card registers as a navigable container (`<FocusScope>`)
 *     parented at the column zone — cards are zones because they hold
 *     multiple focusable atoms (drag handle, Field rows, inspect button)
 *     and the kernel's path-prefix scope-is-leaf invariant rejects a
 *     `<FocusScope>` whose FQM is a strict prefix of any descendant.
 *   - No claim-predicate registration calls are emitted for the column or
 *     its header.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before any module that imports them.
//
// `mockInvoke` is hoisted so the SpatialFocusProvider's invoke calls
// (`spatial_push_layer`, `spatial_register_scope`, …) flow through it and
// tests can assert against them.
// ---------------------------------------------------------------------------

// Schema for the column entity. The column-name surface is rendered by
// `<Field>`, which only mounts a `<FocusScope>` when the schema's
// `getFieldDef("column", "name")` returns a definition — without a
// schema the column header falls back to a bare `<span>` with no
// spatial-nav participation, and the field-zone registration assertions
// below cannot fire.
const COLUMN_SCHEMA = {
  entity: {
    name: "column",
    fields: ["name"],
    commands: [],
  },
  fields: [
    {
      id: "name",
      name: "name",
      type: { kind: "text" },
      section: "header",
      display: "text",
      editor: "text",
    },
  ],
};

const mockInvoke = vi.hoisted(() => {
  const fn = vi.fn(async (...args: unknown[]) => {
    if (args[0] === "list_entity_types") return ["column", "task"];
    if (args[0] === "get_entity_schema") {
      return COLUMN_SCHEMA;
    }
    if (args[0] === "get_ui_state") {
      return {
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      };
    }
    if (args[0] === "list_commands_for_scope") return [];
    return undefined;
  });
  return fn;
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
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
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { ColumnView } from "./column-view";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

function makeColumn(id = "col-1", name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

function makeTask(id: string, column = "col-1"): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: column,
      position_ordinal: "a0",
    },
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Flush microtasks queued by the spatial-primitive register effects.
 *
 * `<FocusScope>` / `<FocusScope>` perform their `spatial_register_*` invocations
 * inside `useEffect`, which React flushes asynchronously. Without this nudge
 * the assertions run before the register calls land in the mock.
 */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Render a `<ColumnView>` inside the production spatial stack and a
 * surrounding `ui:board` zone, so the column registers with a real parent
 * zone (mirroring its role inside `<BoardView>`).
 */
function renderColumnInBoard(ui: React.ReactElement) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{}}>
              <TooltipProvider>
                <ActiveBoardPathProvider value="/test/board">
                  <FocusScope moniker={asSegment("ui:board")}>{ui}</FocusScope>
                </ActiveBoardPathProvider>
              </TooltipProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/**
 * Pull every `spatial_register_scope` call as a typed record.
 *
 * After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy
 * split primitives into a single `<FocusScope>`, every spatial primitive
 * registers via `spatial_register_scope`. The legacy helpers
 * `registeredZones` / `registeredScopes` (which used to filter on a
 * separate zone command) now alias to the same single-source helper.
 */
function registeredScopes(): Array<{
  fq: string;
  segment: string;
  rect: unknown;
  layerFq: string;
  parentZone: string | null;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map(
      (c) =>
        c[1] as {
          fq: string;
          segment: string;
          rect: unknown;
          layerFq: string;
          parentZone: string | null;
        },
    );
}

/** Legacy alias kept while call sites are migrated. */
const registeredZones = registeredScopes;

/** Pull every `spatial_unregister_scope` call's `fq` argument. */
function unregisteredScopeKeys(): string[] {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => (c[1] as { fq: string }).fq);
}

/**
 * Shape of one entry inside a `spatial_register_batch` invoke. Mirrors
 * the Rust `RegisterEntry` record the column ships across the IPC
 * boundary.
 *
 * After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy
 * split primitives into a single `<FocusScope>`, batch entries no
 * longer carry a `kind` discriminator — every spatial primitive is a
 * scope, so the kernel-side `RegisterEntry` enum collapsed to a single
 * variant.
 */
interface BatchScopeEntry {
  fq: string;
  segment: string;
  rect: { x: number; y: number; width: number; height: number };
  layer_fq: string;
  parent_zone: string | null;
  overrides: Record<string, unknown>;
}

/**
 * Pull every `spatial_register_batch` call's `entries` argument flattened
 * into one list of scope entries — convenient when assertions only care
 * about whether a particular task ever had a placeholder shipped.
 */
function batchEntries(): BatchScopeEntry[] {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_batch")
    .flatMap((c) => (c[1] as { entries: BatchScopeEntry[] }).entries ?? []);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("ColumnView (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  it("registers the column body as a zone with moniker column:{id}", async () => {
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const columnZones = registeredZones().filter(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZones).toHaveLength(1);

    unmount();
  });

  it("parents the column zone at the surrounding ui:board zone", async () => {
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const boardZone = registeredZones().find((z) => z.segment === "ui:board");
    expect(boardZone).toBeTruthy();

    const columnZone = registeredZones().find(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZone).toBeTruthy();
    expect(columnZone!.parentZone).toBe(boardZone!.fq);

    unmount();
  });

  it("registers the column-name field zone inside the column zone", async () => {
    // After collapsing the synthetic `column:<id>.name` `<FocusScope>` wrap,
    // the column-name surface is registered exactly once — by the inner
    // `<Field>` component as a `<FocusScope>` with moniker
    // `field:column:<id>.name`. The registration's `parentZone` is the
    // enclosing column zone, so beam search treats the column-name field
    // as an in-column candidate.
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const columnZone = registeredZones().find(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZone).toBeTruthy();

    const fieldZone = registeredZones().find(
      (z) => z.segment === "field:column:col-doing.name",
    );
    expect(fieldZone).toBeTruthy();
    expect(fieldZone!.parentZone).toBe(columnZone!.fq);

    unmount();
  });

  it("does not register a synthetic column-name scope", async () => {
    // Regression guard: after the refactor, the only spatial-nav
    // registration for the column-name surface is the `<Field>` zone
    // with moniker `field:column:<id>.name`. The previous synthetic
    // `<FocusScope moniker="column:<id>.name">` wrap is gone — its
    // existence created two registrations against the same DOM rect
    // (a leaf and a zone), two click handlers, and two debug overlays.
    // This test pins the absence of the synthetic moniker so a future
    // change cannot silently re-introduce the duplication.
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const syntheticScopes = registeredScopes().filter(
      (s) => s.segment === "column:col-doing.name",
    );
    expect(syntheticScopes).toHaveLength(0);

    const syntheticZones = registeredZones().filter(
      (z) => z.segment === "column:col-doing.name",
    );
    expect(syntheticZones).toHaveLength(0);

    unmount();
  });

  it("registers each task card as a FocusScope parented at the column scope", async () => {
    // Cards register as `<FocusScope>` containers because they hold
    // multiple focusable atoms (drag handle, Field rows, inspect
    // button). The card's `parentZone` is the enclosing column's scope
    // key so the kernel groups cards by column for cross-column nav.
    //
    // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
    // legacy split primitives into a single `<FocusScope>`, every
    // spatial primitive registers via `spatial_register_scope`; the
    // structural distinction between a container and a leaf is whether
    // the scope has child scopes, not a separate registration command.
    const tasks = [makeTask("t1"), makeTask("t2")];
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    const columnZone = registeredZones().find(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZone).toBeTruthy();

    for (const id of ["t1", "t2"]) {
      const taskZone = registeredZones().find(
        (z) => z.segment === `task:${id}`,
      );
      expect(taskZone, `task:${id} scope registered`).toBeTruthy();
      expect(taskZone!.parentZone).toBe(columnZone!.fq);
    }

    unmount();
  });

  it("registers no claim predicates for the column or header (push-only nav)", async () => {
    const tasks = [makeTask("t1"), makeTask("t2"), makeTask("t3")];
    const { unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    const claimCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "spatial_register_claim" ||
        c[0] === "register_claim_predicates",
    );
    expect(claimCalls).toHaveLength(0);

    unmount();
  });

  it("emits a wrapper element with data-moniker='column:{id}'", async () => {
    const { container, unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={[]} />,
    );
    await flushSetup();

    const node = container.querySelector("[data-segment='column:col-doing']");
    expect(node).not.toBeNull();

    unmount();
  });

  it("ships a spatial_register_batch invoke for off-screen rows when virtualization is active", async () => {
    // Above the virtualization threshold (25), the column delegates to
    // TanStack Virtual which mounts only the visible window. Off-screen
    // rows have no real-mounted primitives, so the column registers
    // placeholder entries via `spatial_register_batch` so the spatial
    // graph has candidate rectangles for nav.down past the visible
    // window.
    //
    // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
    // legacy split primitives into a single `<FocusScope>`, batch
    // entries no longer carry a `kind` discriminator — every spatial
    // primitive is a scope, so the kernel-side `RegisterEntry` enum
    // collapsed to a single variant.
    const N = 60;
    const tasks: Entity[] = [];
    for (let i = 0; i < N; i++) tasks.push(makeTask(`t${i}`));

    const { container, unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    // Tailwind isn't bundled in tests, so utility classes don't produce
    // CSS. Stub a finite viewport on the scroll container so the
    // virtualizer can compute a real visible window and trigger the
    // placeholder hook's effect (the same pattern used by the layout
    // regression test in column-view.test.tsx and data-table.virtualized.test.tsx).
    const scrollEl = container.querySelector(
      "[class*='overflow-y-auto']",
    ) as HTMLDivElement | null;
    expect(scrollEl).toBeTruthy();
    scrollEl!.style.height = "400px";
    scrollEl!.style.maxHeight = "400px";
    scrollEl!.style.overflow = "auto";

    // Let the virtualizer's ResizeObserver settle and the placeholder
    // effect fire. `waitFor` is polled — it settles as soon as the
    // assertion passes instead of holding a fixed timeout.
    await waitFor(() => {
      const batchCalls = mockInvoke.mock.calls.filter(
        (c) => c[0] === "spatial_register_batch",
      );
      expect(batchCalls.length).toBeGreaterThan(0);
    });

    const batchCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_register_batch",
    );

    // Sanity-check the wire shape — entries are an array of
    // RegisterEntry records with newtyped fields. After the
    // single-primitive collapse there is no `kind` discriminator: every
    // entry is a scope.
    const lastBatch = batchCalls[batchCalls.length - 1];
    const args = lastBatch[1] as { entries: unknown[] };
    expect(Array.isArray(args.entries)).toBe(true);
    expect(args.entries.length).toBeGreaterThan(0);
    const first = args.entries[0] as {
      fq: string;
      segment: string;
      rect: { x: number; y: number; width: number; height: number };
      layer_fq: string;
      parent_zone: string | null;
      overrides: Record<string, unknown>;
    };
    expect(typeof first.fq).toBe("string");
    expect(first.segment).toMatch(/^task:/);
    expect(typeof first.rect.x).toBe("number");
    expect(typeof first.layer_fq).toBe("string");

    // The off-screen entries must parent at the column zone, not at the
    // surrounding ui:board — this matches how real-mounted task cards
    // register so kind/parent_zone stability holds across the
    // placeholder→real swap.
    const columnZone = registeredZones().find(
      (z) => z.segment === "column:col-doing",
    );
    expect(columnZone).toBeTruthy();
    expect(first.parent_zone).toBe(columnZone!.fq);

    unmount();
  });

  it("unregisters a placeholder when its task is removed from the column", async () => {
    // Regression test for an effect-ordering leak. The column body uses
    // two refs that both depend on `tasks`:
    //
    //   1. `useStableFullyQualifiedMonikers` — prunes its (id → FullyQualifiedMoniker) map for
    //      tasks that have left the list.
    //   2. `usePlaceholderRegistration` — emits `spatial_unregister_scope`
    //      for placeholders whose task IDs are no longer off-screen.
    //
    // (1) is declared first in `VirtualColumn` so its effect fires first
    // in commit order. If (2) reads the deleted task's key from the live
    // (and now-pruned) `stableKeys` map during the unregister loop, the
    // lookup misses and the kernel keeps a stale `FocusScope` entry
    // under an orphaned `FullyQualifiedMoniker` — a beam-search dead-end after
    // delete. (2) must therefore remember the key it registered against,
    // independent of the live `stableKeys` map.
    const N = 60;
    const tasks: Entity[] = [];
    for (let i = 0; i < N; i++) tasks.push(makeTask(`t${i}`));

    const { container, rerender, unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    const scrollEl = container.querySelector(
      "[class*='overflow-y-auto']",
    ) as HTMLDivElement | null;
    expect(scrollEl).toBeTruthy();
    scrollEl!.style.height = "400px";
    scrollEl!.style.maxHeight = "400px";
    scrollEl!.style.overflow = "auto";

    // Wait for the first placeholder batch to land so we can read off
    // the placeholder key for the task we'll delete.
    await waitFor(() => {
      expect(batchEntries().length).toBeGreaterThan(0);
    });

    // Pick a task that we know was off-screen at first paint — index 50
    // is well below the visible window of a 400px-tall viewport with
    // ~80px rows. Find its placeholder key from the batch entries.
    const targetTaskId = "t50";
    const targetEntry = batchEntries().find(
      (e) => e.segment === `task:${targetTaskId}`,
    );
    expect(
      targetEntry,
      "the off-screen task we're about to delete had a placeholder shipped",
    ).toBeTruthy();
    const targetKey = targetEntry!.fq;

    // Snapshot unregister calls so we can detect the *new* one fired by
    // the rerender below.
    const unregistersBefore = unregisteredScopeKeys().length;

    // Drop t50 from the task list. Both effects refire; the deleted
    // task's placeholder must be unregistered.
    const tasksAfter = tasks.filter((t) => t.id !== targetTaskId);
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <SchemaProvider>
              <EntityStoreProvider entities={{}}>
                <TooltipProvider>
                  <ActiveBoardPathProvider value="/test/board">
                    <FocusScope moniker={asSegment("ui:board")}>
                      <ColumnView
                        column={makeColumn("col-doing")}
                        tasks={tasksAfter}
                      />
                    </FocusScope>
                  </ActiveBoardPathProvider>
                </TooltipProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    await waitFor(() => {
      const keys = unregisteredScopeKeys().slice(unregistersBefore);
      expect(
        keys.includes(targetKey),
        `placeholder for ${targetTaskId} (key=${targetKey}) was unregistered`,
      ).toBe(true);
    });

    unmount();
  });

  it("computes placeholder rects in viewport coordinates after the column scrolls", async () => {
    // Regression test for a coordinate-frame mismatch. The real-mounted
    // `EntityCard`'s rect comes from `getBoundingClientRect()`, which is
    // viewport-relative — its y shrinks as the row scrolls up out of
    // view. Placeholders must share that frame: an above-viewport task
    // (one the user has scrolled past) should have a *negative* y, not
    // sit at the visible top edge. Otherwise beam search would compare
    // rects in two unrelated coordinate systems and pick wrong
    // candidates after any scroll.
    const N = 60;
    const tasks: Entity[] = [];
    for (let i = 0; i < N; i++) tasks.push(makeTask(`t${i}`));

    const { container, unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    const scrollEl = container.querySelector(
      "[class*='overflow-y-auto']",
    ) as HTMLDivElement | null;
    expect(scrollEl).toBeTruthy();
    scrollEl!.style.height = "400px";
    scrollEl!.style.maxHeight = "400px";
    scrollEl!.style.overflow = "auto";

    // Anchor the scroll container at a known viewport top so we can
    // reason about rect.y absolutely. jsdom's `getBoundingClientRect`
    // returns zeros by default; stub it to a fixed origin.
    const baseY = 100;
    Object.defineProperty(scrollEl, "getBoundingClientRect", {
      configurable: true,
      value: () =>
        ({
          x: 0,
          y: baseY,
          width: 320,
          height: 400,
          top: baseY,
          left: 0,
          right: 320,
          bottom: baseY + 400,
        }) as DOMRect,
    });

    // Wait for the first batch (no scroll yet — placeholders should be
    // at or below baseY).
    await waitFor(() => {
      expect(batchEntries().length).toBeGreaterThan(0);
    });

    // Now scroll the virtualizer down. Setting `scrollTop` and
    // dispatching a scroll event drives @tanstack/react-virtual's
    // offset observer.
    const scrollDistance = 1600; // ~20 rows at 80px each.
    await act(async () => {
      scrollEl!.scrollTop = scrollDistance;
      scrollEl!.dispatchEvent(new Event("scroll"));
    });

    // After scrolling, a fresh batch must land where the same task —
    // now *above* the viewport — has y < baseY (because its content-y
    // is below the current scroll offset, so viewport-y is negative
    // relative to baseY).
    await waitFor(() => {
      // Look for the most recent placeholder for an early-index task
      // (e.g. t1) that has now scrolled out of view. Its y must reflect
      // the viewport coordinate frame, not the document frame.
      const t1Entries = batchEntries().filter((e) => e.segment === "task:t1");
      expect(t1Entries.length).toBeGreaterThan(0);
      const lastT1 = t1Entries[t1Entries.length - 1];
      // `t1`'s content-y is ~80px and the user has scrolled ~1600px,
      // so its viewport-y is ~baseY + 80 - 1600 = -1420 — well below
      // the container's top edge.
      expect(lastT1.rect.y).toBeLessThan(baseY);
    });

    unmount();
  });

  it("unregisters every live placeholder when the column unmounts", async () => {
    // Pinned cleanup contract: tearing down a virtualized column must
    // not leak placeholder entries into the kernel registry. Without
    // this, a board that re-renders columns (perspective swap, project
    // filter change) would accumulate dead `FocusScope` keys
    // forever.
    const N = 60;
    const tasks: Entity[] = [];
    for (let i = 0; i < N; i++) tasks.push(makeTask(`t${i}`));

    const { container, unmount } = renderColumnInBoard(
      <ColumnView column={makeColumn("col-doing")} tasks={tasks} />,
    );
    await flushSetup();

    const scrollEl = container.querySelector(
      "[class*='overflow-y-auto']",
    ) as HTMLDivElement | null;
    expect(scrollEl).toBeTruthy();
    scrollEl!.style.height = "400px";
    scrollEl!.style.maxHeight = "400px";
    scrollEl!.style.overflow = "auto";

    await waitFor(() => {
      expect(batchEntries().length).toBeGreaterThan(0);
    });

    // Snapshot the set of placeholder keys the column has registered so
    // far (the off-screen tasks at first paint).
    const liveKeys = new Set(batchEntries().map((e) => e.fq));
    expect(liveKeys.size).toBeGreaterThan(0);

    const unregistersBefore = unregisteredScopeKeys().length;
    unmount();

    // Every live placeholder key must show up in the
    // `spatial_unregister_scope` calls fired during teardown.
    await waitFor(() => {
      const keysAfter = unregisteredScopeKeys().slice(unregistersBefore);
      for (const key of liveKeys) {
        expect(
          keysAfter.includes(key),
          `placeholder ${key} unregistered on unmount`,
        ).toBe(true);
      }
    });
  });
});
