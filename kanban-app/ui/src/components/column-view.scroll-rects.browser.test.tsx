/**
 * Browser-mode integration test for the rect-staleness fix in
 * `<ColumnView>`. Pins the bug fix from card
 * `01KQ9XBAG5P9W3JREQYNGAYM8Y`: real-mounted cards inside a scrolled
 * column must keep their kernel-side rects in sync with their
 * on-screen `getBoundingClientRect()`, and clicks on those cards must
 * focus them across scroll positions.
 *
 * Mounts a `<ColumnView>` with enough tasks (≥ 30) to force the
 * virtualizer on, inside a fixed-height ancestor that forces the
 * column body to scroll. Drives `scrollTop` directly on the column's
 * inner scroll container, asserts on the captured `spatial_update_rect`
 * and `spatial_focus` IPC calls.
 *
 * Test cases (the card's "Frontend tests" section):
 *
 * 1. `kernel_rects_track_visible_cards_after_scroll` — scroll the
 *    column, then assert every visible card's last kernel rect is
 *    within 1 px of its current `getBoundingClientRect()`. This is the
 *    rect-staleness regression guard.
 * 2. `click_card_at_top_of_scrolled_column_focuses_it` — scroll the
 *    column and click a card whose viewport-y has shifted ≥ 200 px
 *    from its mount-time position. Asserts `spatial_focus` fires for
 *    the clicked card's registered key.
 * 3. `click_each_visible_card_after_scroll_focuses_each` — scroll, then
 *    iterate every visible card and click each. Asserts each click
 *    produces a `spatial_focus` for that card's key. Rules out a
 *    one-off mount-race coincidence by asserting against multiple
 *    cards in a single test run.
 * 4. `click_card_immediately_after_scroll_into_view_focuses_it` —
 *    scrolls programmatically and clicks the now-visible card without
 *    waiting beyond the same animation frame. Regression guard for
 *    the mount-vs-click race.
 * 5. `non_scrolling_column_click_still_focuses_card` — non-regression:
 *    a column with too few tasks to scroll keeps working as today.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

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
  emit: vi.fn(() => Promise.resolve()),
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
// Imports — after mocks
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { ColumnView } from "./column-view";
import { FocusLayer } from "./focus-layer";
import { FocusZone } from "./focus-zone";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  asSegment,
  type FullyQualifiedMoniker
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/** Identity-stable column id used by every fixture in this file. */
const COLUMN_ID = "01ABCDEFGHJKMNPQRSTVWXYZ01";

/** Build a column entity. */
function makeColumn(id = COLUMN_ID, name = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

/**
 * Build a deterministic task entity. The id is left-padded to a ULID-ish
 * 26-char shape so the moniker matches the production format and the
 * `spatial_register_scope` calls stay readable in mock-call dumps.
 */
function makeTask(index: number): Entity {
  const id = `01TASK${String(index).padStart(20, "0")}`;
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${index}`,
      position_column: COLUMN_ID,
      position_ordinal: `a${String(index).padStart(4, "0")}`,
    },
  };
}

/**
 * Minimal task schema. Only `title` is needed because the cards render
 * their `title` field in the `header` section. Extra fields would just
 * inflate the visible card height and complicate scroll-offset math.
 */
const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    fields: ["title"],
    sections: [{ id: "header", on_card: true }],
  },
  fields: [
    {
      id: "f-title",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
} as unknown as import("@/types/kanban").EntitySchema;

// ---------------------------------------------------------------------------
// Default invoke responses
// ---------------------------------------------------------------------------

/**
 * Default mock-invoke implementation for the IPCs the providers fire on
 * mount. The spatial-focus IPCs (`spatial_register_*`, `spatial_focus`,
 * `spatial_update_rect`) all fall through to `undefined`; the test reads
 * them out of the mock's call log.
 */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  // `list_entity_types` returns only the types this fixture has a real
  // schema for; SchemaProvider asks `get_entity_schema` for each, and a
  // `null` response would land in the schema map and crash downstream.
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    if (entityType === "task") return TASK_SCHEMA;
    return null;
  }
  if (cmd === "get_ui_state") {
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
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "show_context_menu") return undefined;
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Two-tick microtask flush for register effects. First tick lets the
 * `useEffect` callbacks run; second lets any promise-resolution-driven
 * follow-on (listener registration, etc.) settle.
 */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Wait for one animation frame plus a microtask flush. The scroll
 * listener throttles via `requestAnimationFrame`, so a synchronous
 * `scroll` event needs one rAF tick to land in the IPC mock.
 */
async function flushScroll() {
  await act(async () => {
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => resolve()),
    );
    await Promise.resolve();
  });
}

/**
 * Pull every `spatial_register_scope` invocation argument bag for tasks.
 * Returns a `(moniker → key)` map so the test can convert visible
 * `data-moniker` attributes to the kernel-side keys for assertion.
 */
function taskMonikerToKey(): Map<string, FullyQualifiedMoniker> {
  const map = new Map<string, FullyQualifiedMoniker>();
  for (const [cmd, args] of mockInvoke.mock.calls) {
    if (cmd !== "spatial_register_scope") continue;
    const a = args as { segment?: string; fq?: FullyQualifiedMoniker };
    if (typeof a.segment === "string" && a.segment.startsWith("task:") && a.fq) {
      // Last-write-wins. The most recent register for this moniker is
      // the live key (placeholders may have been registered first then
      // unregistered by the visibility hook).
      map.set(a.segment, a.fq);
    }
  }
  return map;
}

/** Pull every `spatial_focus` call argument, in order. */
function spatialFocusCalls(): Array<{ fq: FullyQualifiedMoniker }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_focus")
    .map((c) => c[1] as { fq: FullyQualifiedMoniker });
}

/**
 * Pull every `spatial_update_rect` invocation. Used to verify the kernel
 * received fresh rects after a scroll.
 */
function updateRectCalls(): Array<{
  key: FullyQualifiedMoniker;
  rect: { x: number; y: number; width: number; height: number };
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_update_rect")
    .map(
      (c) =>
        c[1] as {
          key: FullyQualifiedMoniker;
          rect: { x: number; y: number; width: number; height: number };
        },
    );
}

/**
 * Find the test wrapper div that owns the outer scroll. The vitest
 * browser project does not bundle Tailwind, so the column-view's inner
 * `flex-1 overflow-y-auto` scroller produces no CSS rules and the
 * column body renders at its natural content height. The test fixture
 * sidesteps this by making the outer wrapper itself the scrollable
 * ancestor (inline `overflow-y: auto`); the scroll listener walks the
 * parent chain so this works the same as production.
 *
 * Throws on miss so test failures are diagnostic.
 */
function findOuterScroller(container: HTMLElement): HTMLElement {
  const node = container.querySelector(
    "[data-testid='board-shell']",
  ) as HTMLElement | null;
  if (!node) {
    throw new Error("expected to find the test wrapper [data-testid='board-shell']");
  }
  return node;
}

// ---------------------------------------------------------------------------
// Render helpers
// ---------------------------------------------------------------------------

/**
 * Render a `<ColumnView>` inside a fixed-height surrounding box that
 * forces the column to scroll. The column lives under a surrounding
 * `ui:board` zone so its `parentZone` matches production.
 *
 * The wrapper's height is set to 600 px so that a column populated with
 * the default 35 tasks (~80 px each = ~2800 px content) overflows and
 * the inner scroll container has real scroll mechanics.
 */
function renderColumn(column: Entity, tasks: Entity[]) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: tasks }}>
              <FieldUpdateProvider>
                <UIStateProvider>
                  <TooltipProvider>
                    <ActiveBoardPathProvider value="/test/board">
                      <div
                        data-testid="board-shell"
                        style={{
                          // The vitest browser project does not bundle
                          // Tailwind, so the column-view's inner
                          // `flex-1 overflow-y-auto min-h-0` chain
                          // produces no CSS rules and the column body
                          // renders at its natural content height
                          // (every card visible, no scrolling).
                          //
                          // Drive scroll from the OUTER wrapper instead:
                          // make this div the scrollable ancestor with
                          // inline `overflow-y: auto` plus a fixed
                          // `height`. The scroll listener walks the
                          // parent chain and picks up this wrapper at
                          // card mount time, so a scroll on this wrapper
                          // re-publishes the cards' rects correctly.
                          //
                          // The scroll listener is direction-agnostic:
                          // a scroll on any scrollable ancestor (whether
                          // the column's inner scroller in production or
                          // this outer wrapper in tests) updates the
                          // kernel rect.
                          height: "400px",
                          width: "400px",
                          overflowY: "auto",
                          overflowX: "hidden",
                        }}
                      >
                        <FocusZone moniker={asSegment("ui:board")}>
                          <ColumnView column={column} tasks={tasks} />
                        </FocusZone>
                      </div>
                    </ActiveBoardPathProvider>
                  </TooltipProvider>
                </UIStateProvider>
              </FieldUpdateProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<ColumnView> — rect freshness on scroll & click reliability", () => {
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
  // 1. Kernel rects track visible cards after scroll.
  // -------------------------------------------------------------------------

  it("kernel_rects_track_visible_cards_after_scroll", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 35 }, (_, i) => makeTask(i));
    const { container, unmount } = renderColumn(column, tasks);
    await flushSetup();
    await flushScroll();

    const scroller = findOuterScroller(container);
    expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);

    // Snapshot the moniker→key mapping NOW, before the scroll, while the
    // pre-scroll virtual window's cards are still mounted. Cards that
    // get unmounted by the virtualizer during the scroll keep their
    // entry in the snapshot, but the test only asserts against cards
    // that remain visible after the scroll — it filters by
    // `[data-moniker^='task:']` afterwards.
    const monikerToKey = taskMonikerToKey();

    // Scroll mid-list. 400 px is large enough to shift visible cards by
    // roughly one full card-height row, so any stale rect would leave
    // the kernel disagreeing with the DOM by hundreds of pixels.
    await act(async () => {
      scroller.scrollTop = 400;
      scroller.dispatchEvent(new Event("scroll"));
    });
    // Two animation frames: first lets the scroll listener's rAF fire,
    // second lets any virtualizer-driven remount + ResizeObserver's
    // mount-time update settle (newly-mounted cards register at the
    // post-scroll position directly via `spatial_register_scope`, so
    // they don't need an `update_rect` to be in sync).
    await flushScroll();
    await flushScroll();

    // Refresh the moniker→key map after the scroll so newly-mounted
    // cards (which registered AFTER the snapshot above) are picked up.
    const monikerToKeyAfter = taskMonikerToKey();
    const updates = updateRectCalls();

    // Build a `(key → most-recent rect)` table from the captured
    // updates; later writes overwrite earlier ones in last-write-wins.
    const lastRectByKey = new Map<
      FullyQualifiedMoniker,
      { x: number; y: number; width: number; height: number }
    >();
    for (const u of updates) {
      lastRectByKey.set(u.key, u.rect);
    }

    const visibleCards = container.querySelectorAll(
      "[data-moniker^='task:']",
    );
    expect(visibleCards.length).toBeGreaterThan(0);

    // For every currently visible card, the kernel's idea of its rect
    // (either from the most recent `update_rect` or, for cards that
    // mounted post-scroll, from the registration rect captured at
    // mount-time) must match the DOM's current `getBoundingClientRect()`
    // within 1 px.
    //
    // We assert against the latest write the kernel has seen for that
    // card's key: prefer the post-scroll `update_rect` write when one
    // exists; otherwise fall back to the registration rect captured by
    // `spatial_register_scope` (cards that mounted at the post-scroll
    // position never needed an `update_rect`).
    const lastRegisterRectByKey = new Map<
      FullyQualifiedMoniker,
      { x: number; y: number; width: number; height: number }
    >();
    for (const [cmd, args] of mockInvoke.mock.calls) {
      if (cmd !== "spatial_register_scope") continue;
      const a = args as {
        key?: FullyQualifiedMoniker;
        rect?: { x: number; y: number; width: number; height: number };
      };
      if (a.key && a.rect) lastRegisterRectByKey.set(a.key, a.rect);
    }

    let assertedAtLeastOne = false;
    for (const card of visibleCards) {
      const moniker = card.getAttribute("data-moniker")!;
      const key = monikerToKeyAfter.get(moniker) ?? monikerToKey.get(moniker);
      if (!key) continue;
      const kernelRect =
        lastRectByKey.get(key) ?? lastRegisterRectByKey.get(key);
      if (!kernelRect) continue;
      const domRect = (card as HTMLElement).getBoundingClientRect();
      expect(
        Math.abs(kernelRect.x - domRect.x),
        `kernel x mismatch for ${moniker}`,
      ).toBeLessThanOrEqual(1);
      expect(
        Math.abs(kernelRect.y - domRect.y),
        `kernel y mismatch for ${moniker}: kernel=${kernelRect.y} dom=${domRect.y}`,
      ).toBeLessThanOrEqual(1);
      expect(Math.abs(kernelRect.width - domRect.width)).toBeLessThanOrEqual(1);
      expect(Math.abs(kernelRect.height - domRect.height)).toBeLessThanOrEqual(
        1,
      );
      assertedAtLeastOne = true;
    }
    expect(
      assertedAtLeastOne,
      "expected at least one visible card to have a kernel-recorded rect to compare",
    ).toBe(true);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 2. Click on a card after scroll focuses it.
  // -------------------------------------------------------------------------

  it("click_card_at_top_of_scrolled_column_focuses_it", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 35 }, (_, i) => makeTask(i));
    const { container, unmount } = renderColumn(column, tasks);
    await flushSetup();
    await flushScroll();

    const scroller = findOuterScroller(container);
    expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);

    // Scroll mid-list — far enough that any visible card's viewport-y
    // has shifted by ≥ 200 px from its mount-time position.
    await act(async () => {
      scroller.scrollTop = 400;
      scroller.dispatchEvent(new Event("scroll"));
    });
    await flushScroll();

    // Pick the topmost visible card after the scroll. Its viewport-y
    // is roughly the scroll container's top edge, while at mount-time
    // its rect was registered far further down in the document.
    const cards = Array.from(
      container.querySelectorAll("[data-moniker^='task:']"),
    ) as HTMLElement[];
    expect(cards.length).toBeGreaterThan(0);
    const topCard = cards[0];
    const moniker = topCard.getAttribute("data-moniker")!;
    const monikerToKey = taskMonikerToKey();
    const expectedKey = monikerToKey.get(moniker)!;
    expect(expectedKey).toBeTruthy();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    fireEvent.click(topCard);
    await flushSetup();

    const focusCalls = spatialFocusCalls();
    expect(focusCalls.length).toBe(1);
    expect(focusCalls[0].fq).toBe(expectedKey);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 3. Click each visible card after scroll — focus tracks every click.
  // -------------------------------------------------------------------------

  it("click_each_visible_card_after_scroll_focuses_each", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 35 }, (_, i) => makeTask(i));
    const { container, unmount } = renderColumn(column, tasks);
    await flushSetup();
    await flushScroll();

    const scroller = findOuterScroller(container);
    await act(async () => {
      scroller.scrollTop = 400;
      scroller.dispatchEvent(new Event("scroll"));
    });
    await flushScroll();

    const monikerToKey = taskMonikerToKey();
    const visibleCards = Array.from(
      container.querySelectorAll("[data-moniker^='task:']"),
    ) as HTMLElement[];
    expect(visibleCards.length).toBeGreaterThan(1);

    // Click each visible card. After every click, assert exactly one
    // `spatial_focus` fired and its `key` matches the clicked card's
    // registered key. Reset the mock between clicks so each click's
    // IPC count starts fresh.
    for (const card of visibleCards) {
      const moniker = card.getAttribute("data-moniker")!;
      const expectedKey = monikerToKey.get(moniker);
      if (!expectedKey) continue;

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      fireEvent.click(card);
      await flushSetup();

      const focusCalls = spatialFocusCalls();
      expect(
        focusCalls.length,
        `click on '${moniker}' must produce exactly one spatial_focus`,
      ).toBe(1);
      expect(focusCalls[0].fq).toBe(expectedKey);
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // 4. Click immediately after scroll — no extra delay between scroll and
  //    click. Mount-vs-click race regression guard.
  // -------------------------------------------------------------------------

  it("click_card_immediately_after_scroll_into_view_focuses_it", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 35 }, (_, i) => makeTask(i));
    const { container, unmount } = renderColumn(column, tasks);
    await flushSetup();
    await flushScroll();

    const scroller = findOuterScroller(container);

    // Programmatically scroll, then click without waiting beyond the
    // same animation frame. This is the regression guard for the
    // mount-vs-click race: when a card scrolls into view, the
    // virtualizer mounts its real `<EntityCard>` at the same paint;
    // a click landing on the same frame must not race the
    // `spatial_register_scope` IPC.
    await act(async () => {
      scroller.scrollTop = 600;
      scroller.dispatchEvent(new Event("scroll"));
    });
    // One rAF for the virtualizer's measurement frame; do NOT add a
    // second timeout. The click below lands on the very next tick.
    await flushScroll();

    const cards = Array.from(
      container.querySelectorAll("[data-moniker^='task:']"),
    ) as HTMLElement[];
    expect(cards.length).toBeGreaterThan(0);
    const candidate = cards[Math.floor(cards.length / 2)];
    const moniker = candidate.getAttribute("data-moniker")!;
    const monikerToKey = taskMonikerToKey();
    const expectedKey = monikerToKey.get(moniker)!;
    expect(expectedKey).toBeTruthy();

    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);

    fireEvent.click(candidate);
    await flushSetup();

    const focusCalls = spatialFocusCalls();
    expect(focusCalls.length).toBe(1);
    expect(focusCalls[0].fq).toBe(expectedKey);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 5. Non-regression: a column with too few tasks to scroll keeps working.
  // -------------------------------------------------------------------------

  it("non_scrolling_column_click_still_focuses_card", async () => {
    const column = makeColumn();
    // Three tasks — well below the virtualization threshold and not
    // enough content to overflow the 600 px wrapper. The column body
    // does not scroll; the scroll listener has nothing to fire on.
    const tasks = [makeTask(0), makeTask(1), makeTask(2)];
    const { container, unmount } = renderColumn(column, tasks);
    await flushSetup();

    const monikerToKey = taskMonikerToKey();
    const cards = Array.from(
      container.querySelectorAll("[data-moniker^='task:']"),
    ) as HTMLElement[];
    expect(cards.length).toBe(3);

    for (const card of cards) {
      const moniker = card.getAttribute("data-moniker")!;
      const expectedKey = monikerToKey.get(moniker);
      if (!expectedKey) continue;

      mockInvoke.mockClear();
      mockInvoke.mockImplementation(defaultInvokeImpl);

      fireEvent.click(card);
      await flushSetup();

      const focusCalls = spatialFocusCalls();
      expect(focusCalls.length).toBe(1);
      expect(focusCalls[0].fq).toBe(expectedKey);
    }

    unmount();
  });
});
