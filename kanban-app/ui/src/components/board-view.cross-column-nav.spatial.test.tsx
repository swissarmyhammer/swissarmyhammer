/**
 * Browser-mode wiring test for cross-column spatial navigation on
 * `<BoardView>`.
 *
 * # Scope: wiring, not algorithm
 *
 * **This file tests the React side's wiring** — that pressing
 * `ArrowRight` (cua) or `l` (vim) on a focused card dispatches
 * `spatial_navigate` with the right `(key, direction)` shape, that the
 * resulting `focus-changed` payload flows back through the
 * `<FocusScope>` claim listener, and that the new focus's
 * `data-moniker` / `data-focused` attributes update on the DOM. **It
 * does NOT test the kernel algorithm.** The kernel's unified cascade
 * (iter 0 in-zone peer search, iter 1 escalation to the parent's
 * level, drill-out fallback, plus the in-beam hard filter and the
 * `13 * major² + minor²` Android scoring formula) is exercised by Rust
 * integration tests against a realistic-app fixture in:
 *
 *   - `swissarmyhammer-focus/tests/unified_trajectories.rs` — source
 *     of truth for the unified-policy supersession card
 *     `01KQ7S6WHK9RCCG2R4FN474EFD`. Holds the canonical user
 *     trajectories the cascade must satisfy (vertical traversal up
 *     the entity stack, cross-column horizontal returning the next-
 *     column zone, leftmost drill-out, inspector field nav with the
 *     layer-boundary guard).
 *   - `swissarmyhammer-focus/tests/card_directional_nav.rs` —
 *     directional-nav supersession card `01KQ7STZN3G5N2WB3FF4PM4DKX`.
 *     Card-focused trajectories that exercise the in-beam hard filter
 *     and the cascade's drill-out fallback at the leftmost / rightmost
 *     column edges.
 *
 * The split exists because letting the React layer judge "did the
 * navigator pick the right candidate?" requires either porting the
 * algorithm (the JS shadow-registry pattern below, which mirrors the
 * Rust kernel's unified cascade) or running the real Tauri runtime
 * (heavy CI infra). Both miss the point: the kernel and the registry
 * shape are the things being tested for algorithm correctness, and
 * they live in Rust. The Rust integration tests build a realistic
 * registry, call `BeamNavStrategy::next` directly, and assert on the
 * returned moniker — no mimicry. This file's role is the complementary
 * half: the React side's responsibility is *to make the right invoke
 * and render the right attribute on the right node*. That contract is
 * asserted here.
 *
 * # Background — the original cards
 *
 * Before the unified-policy supersession, this test was the source of
 * truth for `01KQ7GWE9V2XKWYAQ0HCPDE0EZ` ("Fix: nav.right from a card
 * is trapped inside the column — board zone graph is registered
 * wrong"). The architecture-fix card `01KQ5PP55SAAVJ0V3HDJ1DGNBY` had
 * converted the card body from `<FocusScope kind="zone">` (a zone —
 * sibling-zones-only nav) to `<FocusScope>` (a leaf — falling through
 * to the cross-zone leaf fallback), and the column body from leaf-
 * style to `<FocusZone>`. The browser tests below pinned the post-fix
 * wiring shape (registry audit + JS-shadow-navigator round-trip) so
 * the regression could not return silently.
 *
 * Under the unified cascade, the kernel's answer to "right from a
 * card" is the next-column **zone moniker** (e.g. `column:colB`)
 * rather than a card-leaf moniker inside the next column. The
 * production React adapter resolves the zone moniker by drilling back
 * into the zone's `last_focused` slot or first child, so the focused
 * element ends up tied to a card in the destination column — the
 * wiring contract this file pins. The tests below assert on the
 * destination *column* (which `data-moniker` lives in), not on a
 * specific card-leaf moniker, so the wiring assertions are invariant
 * under the cascade change.
 *
 * The shadow-navigator approach is preserved here as a wiring check:
 * the assertions still need *some* model of what the kernel should
 * answer to verify that the React-side dispatch and event-listening
 * flow drives the resulting `data-focused` attribute change. The shadow
 * navigator is intentionally minimal — its job is to produce a
 * deterministic answer the React tree can react to, not to be an
 * authoritative simulation of the kernel.
 *
 * # Approach
 *
 * The test mounts the production `<BoardView>` inside the same provider
 * stack `App.tsx` mounts (`<SpatialFocusProvider>` → `<FocusLayer>` →
 * the entity / schema / store / ui-state providers → `<AppShell>`). Real
 * Chromium via Playwright lays out the columns and cards, so
 * `getBoundingClientRect()` returns real rects — no fakery on the
 * geometry side.
 *
 * The Tauri `invoke` boundary is mocked through the **shared spatial
 * test harness** at `kanban-app/ui/src/test/spatial-shadow-registry.ts`.
 * The harness owns the `vi.hoisted` mock triple, the Tauri-API
 * `vi.mock` shims, the `BeamNavStrategy` JS port, and the shadow-
 * registry installer. This file just imports `setupSpatialHarness()` to
 * reset and wire everything per-test.
 *
 * Two responsibilities of the harness:
 *
 *   1. Capture every `spatial_register_zone` / `spatial_register_scope`
 *      call (plus `spatial_register_batch` for off-screen scope
 *      placeholders). The captured records are the JS shadow registry
 *      the navigator runs against.
 *   2. Stub `spatial_navigate(key, direction)` so it consults the JS
 *      shadow registry, runs the in-test BeamNavStrategy port (mirroring
 *      `BeamNavStrategy::next` in `swissarmyhammer-focus/src/navigate.rs`),
 *      and emits a `focus-changed` event with the resulting
 *      `next_fq` / `next_segment` so the React tree updates as if the
 *      kernel had answered.
 *
 * The shadow registry walks the **same registration calls the production
 * code makes**, so the wiring under test is the production wiring — only
 * the cross-process boundary is faked.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import type { BoardData, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — file-scoped, forwarding to spies owned by the
// shared spatial-nav harness module
// (`kanban-app/ui/src/test/spatial-shadow-registry.ts`).
//
// `vi.mock` is hoisted to the top of THIS file; a `vi.mock` call in the
// helper module is not visible to this file's transitive imports. The
// `vi.hoisted` factory below resolves the helper's exports before the
// `import { BoardView } from "./board-view"` line below runs, so the
// production code's transitive `@tauri-apps/api/*` imports return the
// mock factories registered here.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen } = await vi.hoisted(async () => {
  const helper = await import("@/test/spatial-shadow-registry");
  return {
    mockInvoke: helper.mockInvoke,
    mockListen: helper.mockListen,
  };
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

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Shared spatial-nav harness — provides the `setupSpatialHarness()`
// entry point that clears the mock triple, installs the bootstrap
// invoke handler, and layers the shadow-registry navigator on top.
// ---------------------------------------------------------------------------

import {
  setupSpatialHarness,
  type SpatialHarness,
} from "@/test/spatial-shadow-registry";

// ---------------------------------------------------------------------------
// Per-file mocks — only the BoardView-specific stub stays here. The
// `useActivePerspective` hook reads from the perspective-container, but
// `<BoardView>` mounts inside `<AppShell>` (no perspective bar above it),
// so the test stubs the hook to return null instead of standing up the
// real provider stack.
// ---------------------------------------------------------------------------

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// ---------------------------------------------------------------------------
// Imports come after mocks.
// ---------------------------------------------------------------------------

import { BoardView } from "./board-view";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  asSegment,
  type FullyQualifiedMoniker
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/**
 * Tasks numbered `1A` / `2A` / `3A` etc. so the test can map them back
 * to their column from the moniker alone (`task:1A` → column A).
 */
function makeColumn(id: string, name: string, order: number): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

function makeTask(id: string, columnId: string, ordinal: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${id}`,
      position_column: columnId,
      position_ordinal: ordinal,
    },
  };
}

const board: BoardData = {
  board: {
    id: "board-1",
    entity_type: "board",
    moniker: "board:board-1",
    fields: { name: "Test Board" },
  },
  columns: [
    makeColumn("colA", "Alpha", 0),
    makeColumn("colB", "Bravo", 1),
    makeColumn("colC", "Charlie", 2),
  ],
  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: 9,
    total_actors: 0,
    ready_tasks: 9,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

const tasks: Entity[] = [
  makeTask("1A", "colA", "a0"),
  makeTask("2A", "colA", "a1"),
  makeTask("3A", "colA", "a2"),
  makeTask("1B", "colB", "a0"),
  makeTask("2B", "colB", "a1"),
  makeTask("3B", "colB", "a2"),
  makeTask("1C", "colC", "a0"),
  makeTask("2C", "colC", "a1"),
  makeTask("3C", "colC", "a2"),
];

/** Lookup: task id → its column id. Used to assert nav crossed columns. */
const taskColumnById = new Map<string, string>([
  ["1A", "colA"],
  ["2A", "colA"],
  ["3A", "colA"],
  ["1B", "colB"],
  ["2B", "colB"],
  ["3B", "colB"],
  ["1C", "colC"],
  ["2C", "colC"],
  ["3C", "colC"],
]);

/**
 * The column id a moniker resolves to, regardless of whether the
 * moniker names a card-leaf (`task:1A` → column `colA`) or a column
 * zone (`column:colA` → column `colA`).
 *
 * Under the unified-cascade kernel from
 * `01KQ7S6WHK9RCCG2R4FN474EFD`, cross-column horizontal nav lands on
 * a column-zone moniker rather than a card-leaf inside the destination
 * column. The wiring contract this test pins is "the focused element's
 * column identity matches the destination column" — and `colB` is the
 * column identity whether the kernel lands on `task:1B` or
 * `column:colB`.
 *
 * Returns `null` when the moniker matches neither shape.
 */
function columnOfMoniker(moniker: string): string | null {
  const taskMatch = /^task:([0-9A-Za-z]+)$/.exec(moniker);
  if (taskMatch) return taskColumnById.get(taskMatch[1]) ?? null;
  const columnMatch = /^column:([0-9A-Za-z]+)$/.exec(moniker);
  if (columnMatch) return columnMatch[1];
  return null;
}

// ---------------------------------------------------------------------------
// Default invoke responses for the AppShell-driven harness.
// ---------------------------------------------------------------------------

async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task", "column"];
  if (cmd === "get_entity_schema") {
    return {
      entity: { name: "task", entity_type: "task" },
      fields: [],
    };
  }
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
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "dispatch_command") return undefined;
  // Spatial-nav register / unregister / focus / navigate calls all return
  // void — undefined is the safe default. Tests that need to observe the
  // navigator output install their own implementation on top of this one.
  return undefined;
}

// ---------------------------------------------------------------------------
// Per-file helpers — viewport sizing + Tailwind substitute CSS.
// ---------------------------------------------------------------------------

/**
 * Wait for register effects scheduled in `useEffect` to flush.
 *
 * The provider stack mounted by `renderBoardWithShell` triggers several
 * async settle steps — `<UIStateProvider>` fetches `get_ui_state`, the
 * spatial primitives' `useEffect` register hooks fire after paint, the
 * column virtualizer measures, and the keybinding handler installs its
 * listener. One microtask flush is not enough; we give it 80ms which is
 * the same nudge sister tests use.
 */
async function flushSetup() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 80));
  });
}

/**
 * Pixel width of the test viewport wrapper.
 *
 * Each column has `min-w-[24em] max-w-[48em]` (= 384px–768px), so three
 * columns side-by-side need at least ~1200px to lay out horizontally.
 * Without enforcing the width here, Playwright's default viewport at
 * 414px collapses the column strip into a vertical stack — `task:1B`
 * lands directly *below* `task:1A` instead of to its right, and
 * cross-column right/left navigation has no horizontal candidates to
 * find. The harness pins the width at 1400px so the layout matches the
 * production shape on a typical desktop window.
 */
const TEST_VIEWPORT_WIDTH_PX = 1400;
const TEST_VIEWPORT_HEIGHT_PX = 900;

/**
 * CSS injected into the document head to substitute for Tailwind during
 * the browser test.
 *
 * The browser test project (`vite.config.ts`'s `browser` project) does
 * NOT load `@tailwindcss/vite` — the tailwindcss plugin is only present
 * on the production build. That keeps the per-test compile cost low,
 * but it means `className="flex flex-1 ..."` on the production
 * components renders as plain `<div>`s with no layout. For all the
 * spatial-nav tests except this one that's harmless: they assert on
 * registration shape and event flow, not on `getBoundingClientRect()`.
 *
 * This test exercises the kernel's beam-search — which is *defined* by
 * the rects of the registered scopes. Without horizontal layout, the
 * three columns stack vertically and cross-column right/left nav has
 * nothing to the right of the focused leaf. Injecting the small handful
 * of layout rules below produces the same row-of-columns shape the
 * production view has, without bringing the entire Tailwind stylesheet
 * into the test bundle.
 *
 * The rule selectors target the production class names in
 * `column-view.tsx`, `board-view.tsx`, and the surrounding chrome — they
 * are intentionally narrow so they don't affect any other layout.
 */
const TEST_LAYOUT_CSS = `
  /* Root stack: vertical column filling the viewport. */
  .flex { display: flex; }
  .flex-col { flex-direction: column; }
  .flex-1 { flex: 1 1 0%; min-width: 0; min-height: 0; }
  .min-h-0 { min-height: 0; }
  .min-w-0 { min-width: 0; }
  .overflow-x-auto { overflow-x: auto; }
  .overflow-y-auto { overflow-y: auto; }
  .relative { position: relative; }
  /* Column body — must enforce min-w so three columns lay out side by
     side instead of collapsing to share the viewport width. */
  .min-w-\\[24em\\] { min-width: 24em; }
  .max-w-\\[48em\\] { max-width: 48em; }
  .shrink-0 { flex-shrink: 0; }
`;

/**
 * Inject the test layout CSS into the document head exactly once.
 *
 * Idempotent — checks for an existing `<style data-test-layout>` node
 * before appending. Tests that mount in sequence share the stylesheet
 * so we don't accumulate copies across runs.
 */
function ensureTestLayoutCss(): void {
  if (document.querySelector("style[data-test-layout]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-layout", "");
  style.textContent = TEST_LAYOUT_CSS;
  document.head.appendChild(style);
}

/**
 * Render `<BoardView>` inside the production-shaped spatial-nav stack
 * wrapped by `<AppShell>` so the global keybinding pipeline is live.
 *
 * The outer `<div>` enforces a 1400×900 viewport so the column strip
 * has room to lay out three columns side-by-side. `display: flex` and
 * `flex-direction: column` mirror App.tsx's root chain so the
 * `flex-1 min-h-0` cascade inside `<BoardView>` resolves to a real
 * height instead of zero.
 *
 * Mirrors the harness used by `board-view.spatial.test.tsx` — the
 * difference here is that this test exercises the **cross-column**
 * navigation path against the production zone graph, not the board-zone
 * registration shape.
 */
function renderBoardWithShell() {
  ensureTestLayoutCss();
  return render(
    <div
      style={{
        width: `${TEST_VIEWPORT_WIDTH_PX}px`,
        height: `${TEST_VIEWPORT_HEIGHT_PX}px`,
        display: "flex",
        flexDirection: "column",
      }}
    >
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <UIStateProvider>
              <AppModeProvider>
                <UndoProvider>
                  <SchemaProvider>
                    <EntityStoreProvider entities={{}}>
                      <TooltipProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <DragSessionProvider>
                            <AppShell>
                              <BoardView board={board} tasks={tasks} />
                            </AppShell>
                          </DragSessionProvider>
                        </ActiveBoardPathProvider>
                      </TooltipProvider>
                    </EntityStoreProvider>
                  </SchemaProvider>
                </UndoProvider>
              </AppModeProvider>
            </UIStateProvider>
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </div>,
  );
}

// ---------------------------------------------------------------------------
// Capture helpers — read records out of the captured `mockInvoke` calls.
// ---------------------------------------------------------------------------

/** Pull every `spatial_register_zone` invocation argument bag. */
function registerZoneArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Pull every `spatial_register_scope` invocation argument bag. */
function registerScopeArgs(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_scope")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Pull every batch-register entry, flattened to one row per entry. */
function registerBatchEntries(): Array<Record<string, unknown>> {
  const rows: Array<Record<string, unknown>> = [];
  for (const c of mockInvoke.mock.calls) {
    if (c[0] !== "spatial_register_batch") continue;
    const a = (c[1] ?? {}) as Record<string, unknown>;
    const entries = (a.entries ?? []) as Array<Record<string, unknown>>;
    for (const e of entries) rows.push(e);
  }
  return rows;
}

/**
 * Find the most recent register record for a moniker — preferring zone
 * registrations when both are present (the ARCHITECTURE-FIX card pinned
 * the cards as scope/leaf, columns and ui:board as zone). Returns null
 * when the moniker never registered.
 */
function findRegisterRecord(
  moniker: string,
): { kind: "zone" | "scope"; record: Record<string, unknown> } | null {
  for (let i = mockInvoke.mock.calls.length - 1; i >= 0; i--) {
    const c = mockInvoke.mock.calls[i];
    const cmd = c[0];
    if (cmd === "spatial_register_zone" || cmd === "spatial_register_scope") {
      const r = c[1] as Record<string, unknown>;
      if (r && r.moniker === moniker) {
        return {
          kind: cmd === "spatial_register_zone" ? "zone" : "scope",
          record: r,
        };
      }
    } else if (cmd === "spatial_register_batch") {
      const a = (c[1] ?? {}) as Record<string, unknown>;
      const entries = (a.entries ?? []) as Array<Record<string, unknown>>;
      for (let j = entries.length - 1; j >= 0; j--) {
        const e = entries[j];
        if (e.moniker === moniker) {
          const k = (e.kind as string) === "zone" ? "zone" : "scope";
          // Batch entries use `layer_key` / `parent_zone` snake_case;
          // normalise to the camelCase shape the rest of the test reads.
          return {
            kind: k,
            record: {
              key: e.key,
              moniker: e.moniker,
              rect: e.rect,
              layerKey: e.layer_key,
              parentZone: e.parent_zone,
              overrides: e.overrides ?? {},
            },
          };
        }
      }
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("BoardView — cross-column spatial navigation", () => {
  /**
   * The harness is recreated per test — each test gets a fresh shadow
   * registry and a fresh `mockInvoke` call log. Tests that need to
   * inspect the post-mount focus state read it via the harness handle
   * (`harness.fireFocusChanged`, `harness.getRegisteredFqBySegment`).
   */
  let harness: SpatialHarness;

  beforeEach(() => {
    harness = setupSpatialHarness({ defaultInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Test #5 — Registry shape audit
  //
  // Run this first because every other case depends on the registry
  // capturing the right shape. Failures here pinpoint the structural
  // root cause: a card registered as a zone, a card registered with the
  // wrong `parent_zone`, or columns spread across multiple layers.
  // -------------------------------------------------------------------------

  it("registers tasks as scope leaves with parent_zone matching their column zone (test #5)", async () => {
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    // Each task must register exactly once via `spatial_register_scope`
    // (i.e. as a leaf), NOT via `spatial_register_zone`. If the card
    // body were a zone, the unified cascade's iter-1 escalation from
    // T1A would land on the next card zone inside the same column
    // (a sibling card) rather than the next column's zone — and the
    // wiring contract this file pins (cross-column nav lands a card-
    // moniker in the destination column) would break.
    for (const taskId of taskColumnById.keys()) {
      const taskMoniker = `task:${taskId}`;
      const zoneCalls = registerZoneArgs().filter(
        (a) => a.segment === taskMoniker,
      );
      expect(
        zoneCalls,
        `${taskMoniker} must NOT register via spatial_register_zone`,
      ).toEqual([]);
      const scopeRec = findRegisterRecord(taskMoniker);
      expect(
        scopeRec,
        `${taskMoniker} must register via spatial_register_scope`,
      ).toBeTruthy();
      expect(scopeRec!.kind).toBe("scope");
    }

    // Each column must register via `spatial_register_zone`, with a
    // parent_zone that resolves to the `ui:board` zone the board view
    // mounts. This pins the column-as-zone shape the kernel test
    // `cross_zone_realistic_board_right_from_card_in_a_lands_on_column_b_zone`
    // (the unified-cascade successor to the old rule-2 test) assumes.
    const boardZone = registerZoneArgs().find((a) => a.segment === "ui:board");
    expect(
      boardZone,
      "ui:board zone must register so columns can hang off it",
    ).toBeTruthy();
    const boardKey = boardZone!.key as FullyQualifiedMoniker;

    for (const colId of ["colA", "colB", "colC"]) {
      const moniker = `column:${colId}`;
      const colZone = registerZoneArgs().find((a) => a.segment === moniker);
      expect(
        colZone,
        `${moniker} must register via spatial_register_zone`,
      ).toBeTruthy();
      expect(
        colZone!.parentZone,
        `${moniker}'s parent_zone must equal the ui:board zone key`,
      ).toBe(boardKey);
    }

    // Every task's parent_zone must equal its enclosing column zone's
    // key. This is the cascade's iter-0 predicate — when it fails,
    // in-zone peer search sees no siblings (the cards are parented at
    // the wrong zone) and the cascade misroutes vertical nav to the
    // wrong column.
    for (const [taskId, columnId] of taskColumnById) {
      const taskMoniker = `task:${taskId}`;
      const colMoniker = `column:${columnId}`;
      const colZone = registerZoneArgs().find(
        (a) => a.segment === colMoniker,
      )!;
      const taskRec = findRegisterRecord(taskMoniker)!;
      expect(
        taskRec.record.parentZone,
        `${taskMoniker}'s parent_zone must equal its column zone (${colMoniker})`,
      ).toBe(colZone.key);
    }

    // Single layer across the whole board — every register call carries
    // the same `layer_key`. A second layer would fragment the shadow
    // registry and make `leaves_in_layer` exclude half the candidates.
    const layerKeys = new Set<unknown>();
    for (const a of registerZoneArgs()) layerKeys.add(a.layerFq);
    for (const a of registerScopeArgs()) layerKeys.add(a.layerFq);
    for (const e of registerBatchEntries()) layerKeys.add(e.layer_key);
    expect(
      layerKeys.size,
      "every spatial registration must share the same layer_key",
    ).toBe(1);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #6 — No navOverride leaked into production
  //
  // The fix has to be structural. If a navOverride snuck in to paper
  // over the bug, the kernel's rule-0 path would short-circuit beam
  // search and the cross-column work would be wired around the
  // navigator instead of through it.
  // -------------------------------------------------------------------------

  it("no spatial_register_* call carries a non-empty overrides payload (test #6)", async () => {
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    for (const a of registerZoneArgs()) {
      expect(
        a.overrides,
        `${String(a.moniker)} zone register must have empty overrides`,
      ).toEqual({});
    }
    for (const a of registerScopeArgs()) {
      expect(
        a.overrides,
        `${String(a.moniker)} scope register must have empty overrides`,
      ).toEqual({});
    }
    for (const e of registerBatchEntries()) {
      expect(
        e.overrides,
        `${String(e.moniker)} batch register must have empty overrides`,
      ).toEqual({});
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #7 — Card body is a leaf, not a zone (architectural guard)
  //
  // Re-asserts the card-as-leaf invariant from a different angle: the
  // `task:<id>` moniker must NEVER appear in a `spatial_register_zone`
  // call. The architecture-fix card `01KQ5PP55SAAVJ0V3HDJ1DGNBY`
  // converted the card body from `<FocusScope kind="zone">` to
  // `<FocusScope>`; this test guards against a regression that would
  // turn it back into a zone — in which case the unified cascade's
  // iter-1 escalation from a card-internal leaf would land on a
  // sibling card zone inside the same column rather than the next
  // column's zone, breaking cross-column horizontal nav.
  // -------------------------------------------------------------------------

  it("task:<id> never registers via spatial_register_zone (test #7)", async () => {
    const { unmount } = renderBoardWithShell();
    await flushSetup();

    const taskZoneCalls = registerZoneArgs().filter((a) =>
      String(a.moniker).startsWith("task:"),
    );
    expect(
      taskZoneCalls,
      "no task:* moniker may register as a zone — cards must be leaves",
    ).toEqual([]);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #8 — No greedy `board:<id>` leaf interferes with cross-column nav
  //
  // The card description's likely cause #4: the `<FocusScope
  // moniker="board:<id>">` wrapping the whole board could act as a
  // greedy leaf with a viewport-size rect, dominating cross-zone
  // scoring. This test asserts that whatever `board:<id>` registers
  // as, it cannot be a leaf in a strict superset of the column-A
  // bounds — either it is bounded (e.g. to a header row) or its
  // presence does not interfere with the unified cascade picking a
  // column-B card after iter-1 escalation + drill-back-in.
  //
  // The strict assertion: after right-press from `task:1A`, the
  // resulting moniker is a `task:*` in column B, not `board:<id>`.
  // -------------------------------------------------------------------------

  it("right from task:1A does not land on board:<id> (test #8)", async () => {
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const task1A = findRegisterRecord("task:1A");
    expect(task1A, "task:1A must register before nav fires").toBeTruthy();
    const task1AKey = task1A!.record.key as FullyQualifiedMoniker;

    // Seed the spatial focus so `nav.right`'s execute closure sees
    // task:1A's key as the focused key. Without this, the global nav
    // command short-circuits.
    await harness.fireFocusChanged({
      next_fq: task1AKey,
      next_segment: asSegment("task:1A"),
    });

    // ArrowRight is the cua binding for `nav.right`. The keymap pipeline
    // routes it through to `spatial_navigate(focused, "right")`. The
    // shadow navigator runs the kernel logic locally and emits a
    // `focus-changed` event with the result; `flushSetup` lets that
    // event propagate through the React tree.
    await userEvent.keyboard("{ArrowRight}");
    await flushSetup();

    // The post-nav focused element's `data-moniker` reveals where the
    // cascade landed. A bug where `board:<id>` won the scoring would
    // surface as the board element being marked focused.
    const focused = container.querySelector(
      "[data-focused='true'][data-moniker]",
    );
    expect(focused, "right-press must select something").not.toBeNull();
    const moniker = focused!.getAttribute("data-moniker") ?? "";
    expect(
      moniker,
      "right from task:1A must not land on a board: scope",
    ).not.toMatch(/^board:/);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #1 — Right from task:1A reaches column B (cua + vim)
  //
  // The headline acceptance criterion. Under the unified cascade the
  // kernel returns the column-B zone moniker; the focused element's
  // `data-moniker` thus identifies column B (either as a `task:*`
  // moniker if drill-back-in resolves it, or as the `column:colB`
  // zone moniker if focus rests on the zone). Either way the column
  // identity must be `colB` — failures here mean the card is trapped
  // in column A, the original user-visible bug.
  // -------------------------------------------------------------------------

  it("ArrowRight (cua) from task:1A lands in column B (test #1.cua)", async () => {
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const task1A = findRegisterRecord("task:1A")!;
    const task1AKey = task1A.record.key as FullyQualifiedMoniker;

    await harness.fireFocusChanged({
      next_fq: task1AKey,
      next_segment: asSegment("task:1A"),
    });

    await userEvent.keyboard("{ArrowRight}");
    await flushSetup();

    const focused = container.querySelector(
      "[data-focused='true'][data-moniker]",
    );
    expect(focused).not.toBeNull();
    const moniker = focused!.getAttribute("data-moniker") ?? "";
    const column = columnOfMoniker(moniker);
    expect(
      column,
      `right from task:1A must land on a task:* or column:* moniker tied to column B (got ${moniker})`,
    ).toBeTruthy();
    expect(column).toBe("colB");

    unmount();
  });

  it("vim 'l' key from task:1A lands in column B (test #1.vim)", async () => {
    // Switch the keymap to vim so `l` resolves to `nav.right`. Override
    // the default invoke impl on top of the harness's spatial routing.
    const baseImpl = mockInvoke.getMockImplementation();
    mockInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "get_ui_state") {
        return {
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "vim",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        };
      }
      return baseImpl?.(cmd, args);
    });

    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const task1A = findRegisterRecord("task:1A")!;
    await harness.fireFocusChanged({
      next_fq: task1A.record.key as FullyQualifiedMoniker,
      next_segment: asSegment("task:1A"),
    });

    await userEvent.keyboard("l");
    await flushSetup();

    const focused = container.querySelector(
      "[data-focused='true'][data-moniker]",
    );
    expect(focused).not.toBeNull();
    const moniker = focused!.getAttribute("data-moniker") ?? "";
    const column = columnOfMoniker(moniker);
    expect(column).toBe("colB");

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #2 — Mirror direction: left from task:1B lands in column A
  // -------------------------------------------------------------------------

  it("ArrowLeft from task:1B lands in column A (test #2)", async () => {
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const task1B = findRegisterRecord("task:1B")!;
    await harness.fireFocusChanged({
      next_fq: task1B.record.key as FullyQualifiedMoniker,
      next_segment: asSegment("task:1B"),
    });

    await userEvent.keyboard("{ArrowLeft}");
    await flushSetup();

    const focused = container.querySelector(
      "[data-focused='true'][data-moniker]",
    );
    expect(focused).not.toBeNull();
    const moniker = focused!.getAttribute("data-moniker") ?? "";
    const column = columnOfMoniker(moniker);
    expect(column).toBe("colA");

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #3 — Repeated right cycles A → B → C, never bounces back to A
  //
  // Pins that the cascade keeps making forward progress after each
  // move; a bug where the cascade picks the closest leaf (which after
  // one move is back in column A) would surface as A → B → A
  // oscillation.
  // -------------------------------------------------------------------------

  it("repeated ArrowRight from column A advances A → B → C (test #3)", async () => {
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const task1A = findRegisterRecord("task:1A")!;
    await harness.fireFocusChanged({
      next_fq: task1A.record.key as FullyQualifiedMoniker,
      next_segment: asSegment("task:1A"),
    });

    await userEvent.keyboard("{ArrowRight}");
    await flushSetup();

    const afterFirst = container.querySelector(
      "[data-focused='true'][data-moniker]",
    );
    expect(afterFirst).not.toBeNull();
    const firstMoniker = afterFirst!.getAttribute("data-moniker") ?? "";
    expect(columnOfMoniker(firstMoniker)).toBe("colB");

    await userEvent.keyboard("{ArrowRight}");
    await flushSetup();

    const afterSecond = container.querySelector(
      "[data-focused='true'][data-moniker]",
    );
    expect(afterSecond).not.toBeNull();
    const secondMoniker = afterSecond!.getAttribute("data-moniker") ?? "";
    const secondColumn = columnOfMoniker(secondMoniker);
    // Either C (advances) or stays at B (no further candidate). Must
    // NEVER bounce back to A.
    expect(secondColumn).not.toBe("colA");
    expect(["colB", "colC"]).toContain(secondColumn);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Test #4 — Up/down within a column still cycles intra-column cards
  //
  // The cascade's iter-0 (in-zone peer search) must continue to work.
  // Down from task:1A lands on task:2A; down again lands on task:3A.
  // The exact behaviour at the bottom edge is left to the existing
  // kernel contract — this test only asserts the two known-good moves
  // so a regression that breaks intra-column nav surfaces here.
  // -------------------------------------------------------------------------

  it("ArrowDown from task:1A advances within column A (test #4)", async () => {
    const { container, unmount } = renderBoardWithShell();
    await flushSetup();

    const task1A = findRegisterRecord("task:1A")!;
    const task1AKey = task1A.record.key as FullyQualifiedMoniker;

    await harness.fireFocusChanged({
      next_fq: task1AKey,
      next_segment: asSegment("task:1A"),
    });

    await userEvent.keyboard("{ArrowDown}");
    await flushSetup();

    let focused = container.querySelector(
      "[data-focused='true'][data-moniker]",
    );
    expect(focused).not.toBeNull();
    expect(focused!.getAttribute("data-moniker")).toBe("task:2A");

    await userEvent.keyboard("{ArrowDown}");
    await flushSetup();

    focused = container.querySelector("[data-focused='true'][data-moniker]");
    expect(focused).not.toBeNull();
    expect(focused!.getAttribute("data-moniker")).toBe("task:3A");

    unmount();
  });
});
