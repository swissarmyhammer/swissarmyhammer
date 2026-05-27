/**
 * Geometry regression guard for the AI panel as a NON-OVERLAPPING right dock.
 *
 * Source of truth for kanban task `01KSAQWXWSPP37V160HWM7YSEK` — "AI chat
 * panel must be a non-overlapping right dock so jump-to labels don't collide
 * with board labels".
 *
 * # Finding (root cause, established by this test's measurements)
 *
 * The panel is ALREADY a true non-overlapping flex dock: it is a `shrink-0`
 * flex sibling of the board well, and the board's `overflow-x-auto` scroll
 * container is correctly constrained (`min-w-0` all the way up the
 * `App.tsx → ViewsContainer → Perspective* → ViewContainer → BoardView`
 * chain). The board's visible scroll viewport ends exactly at the panel's
 * left edge — the panel pushes the board, it never paints over it.
 *
 * The task's literal Acceptance Criterion — "every board scope satisfies
 * `scopeRect.right <= panelRect.left`" — is NOT achievable for a horizontally
 * scrolling kanban board, and the residual `right > panel.left` it asks about
 * is NOT a layout overlap:
 *
 *   - Each column is `min-w-[24em] shrink-0`; with more columns than fit, the
 *     strip overflows the well and scrolls horizontally (intended kanban
 *     behaviour). A column scrolled partly/fully off the right edge has a
 *     `getBoundingClientRect().right` extending past the visible well — the
 *     overflow is clipped for *paint* by `overflow-x-auto`, but
 *     `getBoundingClientRect` reports the un-clipped geometric rect.
 *   - The Jump-To overlay anchors each scope's code pill at the scope's
 *     `rect.left` (`JumpPill`: `left: rect.left + 4`). Every board scope's
 *     `rect.left` is anchored inside the visible well (or scrolled off to the
 *     LEFT), so board pills paint left of the panel and DO NOT collide with
 *     the panel's pills. The measured fixture confirms this even after
 *     scrolling the board fully right.
 *   - The only way a board scope's *pill* lands under the panel is when a
 *     column sits entirely in the right-side overflow region at scrollLeft=0
 *     (boards with enough columns) — i.e. an OCCLUDED, off-viewport scope the
 *     Jump-To overlay enumerates without a visibility/occlusion check. Per the
 *     task, that is a SEPARATE Jump-To occlusion-filtering concern, explicitly
 *     out of scope for this dock task (do not change Jump-To here).
 *
 * # What this guard pins
 *
 * The genuine, achievable dock contract — so a future regression that makes
 * the panel an overlay, or that lets the board well extend under the panel,
 * is caught:
 *
 *   1. Every enumerable board scope's pill anchor (`rect.left`) stays left of
 *      the open panel (no board pill paints under the panel).
 *   2. The board's visible scroll viewport ends at (or before) the panel's
 *      left edge (the panel pushes the board; it is not an overlay).
 *   3. Opening the REAL Jump-To overlay with the panel open paints no board
 *      pill overlapping any panel pill — the user-facing symptom.
 *
 * # Harness
 *
 * Mirrors `spatial-nav-jump-to.spatial.test.tsx` — the same Tauri IPC mock
 * triple, the same end-to-end board fixture, and a Tailwind-substitute
 * stylesheet so the production class strings lay out in real Chromium without
 * `@tailwindcss/vite`. The substitute is faithful to every layout-affecting
 * class in the `App.tsx → … → BoardView → ColumnView` chain and the
 * `AiPanelShell` dock so the measured geometry matches production.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, fireEvent, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — see `spatial-nav-jump-to.spatial.test.tsx` for the
// rationale on why these live in the test file rather than the helper.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen, listeners } = await vi.hoisted(async () => {
  const helper = await import("@/test/spatial-shadow-registry");
  return {
    mockInvoke: helper.mockInvoke,
    mockListen: helper.mockListen,
    listeners: helper.listeners,
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
    setSize: vi.fn(() => Promise.resolve()),
    setPosition: vi.fn(() => Promise.resolve()),
    setFocus: vi.fn(() => Promise.resolve()),
    show: vi.fn(() => Promise.resolve()),
    hide: vi.fn(() => Promise.resolve()),
  }),
  WebviewWindow: class {
    label: string;
    constructor(label: string) {
      this.label = label;
    }
    listen() {
      return Promise.resolve(() => {});
    }
    emit() {
      return Promise.resolve();
    }
    close() {
      return Promise.resolve();
    }
    setSize() {
      return Promise.resolve();
    }
    show() {
      return Promise.resolve();
    }
    hide() {
      return Promise.resolve();
    }
  },
  LogicalSize: class {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
  LogicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
  currentMonitor: vi.fn(() =>
    Promise.resolve({
      name: "test-monitor",
      size: { width: 1920, height: 1080 },
      position: { x: 0, y: 0 },
      scaleFactor: 1,
    }),
  ),
  availableMonitors: vi.fn(() => Promise.resolve([])),
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

void listeners;

// ---------------------------------------------------------------------------
// Spatial harness + fixture imports
// ---------------------------------------------------------------------------

import {
  setupSpatialHarness,
  type SpatialHarness,
} from "@/test/spatial-shadow-registry";

import {
  getBoardDataResponse,
  listEntitiesResponse,
  listOpenBoardsResponse,
  listViewsResponse,
  perspectiveListDispatchResponse,
  getUIStateResponse,
  getUndoStateResponse,
  listEntityTypesResponse,
  getEntitySchemaResponse,
} from "@/test/fixtures/end-to-end-board";

// ---------------------------------------------------------------------------
// Production source under test — full <App/>.
// ---------------------------------------------------------------------------

import App from "@/App";
import { asSegment } from "@/types/spatial";

// Sneak-code fixture — the Rust kernel is not running in browser mode, so the
// `generate_jump_codes` IPC is answered from the same JSON fixture the
// end-to-end Jump-To test uses. See that test's SOURCE OF TRUTH note.
import sneakFixture from "@/test/fixtures/sneak-fixture.json";

/** Look up the pre-computed sneak codes for `count` from the fixture. */
function lookupSneakCodes(count: number): string[] {
  const codes = (sneakFixture as Record<string, string[] | undefined>)[
    String(count)
  ];
  if (codes === undefined) {
    throw new Error(
      `sneak-fixture.json has no entry for count=${count}; ` +
        `regenerate via \`cargo test -p swissarmyhammer-focus --test sneak_fixture\``,
    );
  }
  return codes;
}

// ---------------------------------------------------------------------------
// Bootstrap-invoke impl — covers the Tauri commands the App fires on mount,
// plus the AI panel's `ai_list_models` / `ai_set_streaming` seams so the
// panel renders its expanded body (it never falls into the loading rail).
// ---------------------------------------------------------------------------

async function bootstrapInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  // Schema discovery
  if (cmd === "list_entity_types") return listEntityTypesResponse();
  if (cmd === "get_entity_schema") {
    const a = (args ?? {}) as Record<string, unknown>;
    return getEntitySchemaResponse(String(a.entityType));
  }
  // Board lifecycle
  if (cmd === "get_board_data") return getBoardDataResponse();
  if (cmd === "list_entities") {
    const a = (args ?? {}) as Record<string, unknown>;
    return listEntitiesResponse(String(a.entityType));
  }
  if (cmd === "list_open_boards") return listOpenBoardsResponse();
  if (cmd === "list_views") return listViewsResponse();
  // UI state
  if (cmd === "get_ui_state") return getUIStateResponse("cua");
  if (cmd === "get_undo_state") return getUndoStateResponse();
  // Command dispatch
  if (cmd === "dispatch_command") {
    const a = (args ?? {}) as Record<string, unknown>;
    if (a.cmd === "perspective.list") return perspectiveListDispatchResponse();
    return { result: null, undoable: false };
  }
  if (cmd === "list_commands_for_scope") return [];
  // AI panel seams. `ai_list_models` returns one available model so the
  // Container auto-selects it and renders the expanded panel body rather
  // than the no-model dead-end. `ai_start_agent` / `ai_set_streaming` are
  // accepted no-ops — the panel never actually connects in this test.
  if (cmd === "ai_list_models") {
    return [
      {
        id: "claude-code",
        label: "Claude Code",
        kind: "claude-code",
        available: true,
        hint: null,
      },
    ];
  }
  if (cmd === "ai_start_agent") {
    return { wsUrl: "ws://127.0.0.1:0", mcpUrl: null };
  }
  if (cmd === "ai_set_streaming") return undefined;
  // Jump-To sneak code generator — fixture-backed mock of the Rust kernel.
  if (cmd === "generate_jump_codes") {
    const a = (args ?? {}) as Record<string, unknown>;
    if (typeof a.count !== "number" || !Number.isFinite(a.count)) {
      throw new Error(
        `generate_jump_codes invoked without numeric count; got ${typeof a.count}`,
      );
    }
    return lookupSneakCodes(a.count);
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// Layout substitute — faithful to every layout-affecting production class in
// the App.tsx → board chain and the AiPanelShell dock.
// ---------------------------------------------------------------------------

const TEST_VIEWPORT_WIDTH_PX = 1400;
const TEST_VIEWPORT_HEIGHT_PX = 900;

const TEST_LAYOUT_CSS = `
  .h-screen { height: 100vh; }
  .h-full { height: 100%; }
  .flex { display: flex; }
  .flex-col { flex-direction: column; }
  .flex-1 { flex: 1 1 0%; min-width: 0; min-height: 0; }
  .min-h-0 { min-height: 0; }
  .min-w-0 { min-width: 0; }
  .overflow-hidden { overflow: hidden; }
  .overflow-x-auto { overflow-x: auto; }
  .overflow-y-auto { overflow-y: auto; }
  .relative { position: relative; }
  .absolute { position: absolute; }
  .shrink-0 { flex-shrink: 0; }
  .w-9 { width: 2.25rem; }
  .h-12 { height: 3rem; }
  .border-l { border-left: 1px solid #ccc; }
  .pl-2 { padding-left: 0.5rem; }
  .min-w-\\[24em\\] { min-width: 24em; }
  .max-w-\\[48em\\] { max-width: 48em; }
`;

/** Inject the layout substitute stylesheet exactly once per document. */
function ensureTestLayoutCss(): void {
  if (document.querySelector("style[data-test-ai-panel-dock]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-ai-panel-dock", "");
  style.textContent = TEST_LAYOUT_CSS;
  document.head.appendChild(style);
}

/** Mount the full production `<App/>` inside a fixed-size wrapper. */
function renderApp() {
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
      <App />
    </div>,
  );
}

/**
 * Mount the full production `<App/>` sized to the REAL browser viewport.
 *
 * The geometry assertions above read `getBoundingClientRect`, which honours the
 * fixed-size wrapper {@link renderApp} uses regardless of the browser viewport.
 * The end-to-end Jump-To assertion below instead reads the pills the overlay
 * actually paints, and the overlay now filters out scopes whose pill anchor is
 * not visible in the real viewport (off-screen / occluded — vim-sneak
 * semantics). With a 1400 px wrapper the docked panel sits off the narrower CI
 * viewport, so its scopes would (correctly) be filtered out and the test could
 * not observe any panel pill at all. Sizing to `100vw` / `100vh` keeps the
 * panel inside the viewport the hit-test sees, so the panel still gets pills
 * and the no-overlap contract is exercised against real, visible pills.
 */
function renderAppViewport() {
  ensureTestLayoutCss();
  return render(
    <div
      style={{
        width: "100vw",
        height: "100vh",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <App />
    </div>,
  );
}

/** Wait long enough for the App's bootstrap chain to complete. */
async function flushAppMount() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 250));
  });
}

/**
 * Collect every board-content focus scope host currently in the DOM.
 *
 * Board scopes carry a `data-segment` whose moniker prefix marks them as
 * board geometry: the board zone (`board:…`), columns (`column:…`), and
 * cards (`task:…`). The panel's own scopes (`ui:ai-panel…`) and the chrome
 * scopes (nav-bar, perspective bar) are excluded — the contract is about
 * the board content well, which is what shares the horizontal axis with the
 * panel.
 */
function boardScopeHosts(): HTMLElement[] {
  const hosts = Array.from(
    document.querySelectorAll<HTMLElement>("[data-segment]"),
  );
  return hosts.filter((h) => {
    const seg = h.dataset.segment ?? "";
    return (
      seg.startsWith("task:") ||
      seg.startsWith("column:") ||
      seg.startsWith("board:")
    );
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AI panel — non-overlapping right dock geometry", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    harness = setupSpatialHarness({ defaultInvokeImpl: bootstrapInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Read the open panel's rect after mount, asserting it is present and
   * expanded. Shared by both geometry assertions below.
   */
  function readOpenPanelRect(): DOMRect {
    const panel = document.querySelector<HTMLElement>(
      '[data-testid="ai-panel-container"]',
    );
    expect(panel, "the AI panel dock must be mounted").not.toBeNull();
    expect(
      panel!.getAttribute("data-ai-panel-collapsed"),
      "the AI panel must be expanded (open) for this geometry contract",
    ).toBe("false");
    const panelRect = panel!.getBoundingClientRect();
    expect(
      panelRect.width,
      "the open panel must have a real on-screen width",
    ).toBeGreaterThan(0);
    return panelRect;
  }

  // -------------------------------------------------------------------------
  // The board's visible content well is fully constrained to the LEFT of the
  // panel — the panel pushes the board and never paints over it.
  //
  // This is the load-bearing "non-overlapping dock" contract. The Jump-To
  // overlay anchors each scope's code pill at the scope's `rect.left` (see
  // `JumpPill` in `jump-to-overlay.tsx`: `left: rect.left + 4`). So the
  // user-observable collision the task names — a board card's pill stacking
  // on a panel control's pill — happens iff a board scope's `rect.left`
  // lands inside the panel's horizontal span. This asserts it never does:
  // every enumerable board scope anchors strictly left of the panel.
  //
  // NOTE on the stricter `rect.right <= panel.left` form the task's
  // Acceptance Criteria names: that is unachievable for ANY horizontally
  // scrolling kanban board, because a column scrolled partly off the right
  // edge always has a `rect.right` extending past the visible well (its
  // overflow is clipped for *paint* by the board's `overflow-x-auto` scroll
  // container, but `getBoundingClientRect` reports the un-clipped geometric
  // rect). That overflow is occluded, off-viewport content — its pill still
  // anchors at the scope's left edge inside the visible well, so it does not
  // collide with the panel. Pinning the pill-anchor (`rect.left`) is the
  // contract that matches what the user actually sees.
  // -------------------------------------------------------------------------

  it("every board scope's pill anchor stays left of the open AI panel", async () => {
    const { unmount } = renderApp();
    await flushAppMount();

    const panelRect = readOpenPanelRect();

    const scopes = boardScopeHosts();
    expect(
      scopes.length,
      "the board must register card/column scopes to compare against the panel",
    ).toBeGreaterThan(0);

    // The pill anchor is `rect.left + 4` (see `JumpPill`). A pill collides
    // with the panel iff its anchor lands at/after the panel's left edge.
    const PILL_OFFSET_PX = 4;
    const offenders: string[] = [];
    for (const host of scopes) {
      const r = host.getBoundingClientRect();
      if (r.width <= 0 || r.height <= 0) continue; // not enumerable by jump-to
      const pillX = r.left + PILL_OFFSET_PX;
      if (pillX >= panelRect.left) {
        offenders.push(
          `${host.dataset.segment} pillX=${pillX.toFixed(1)} >= panel.left=${panelRect.left.toFixed(1)}`,
        );
      }
    }

    expect(
      offenders,
      `board scope pills must anchor left of the panel; offenders:\n${offenders.join("\n")}`,
    ).toEqual([]);

    unmount();
  });

  // -------------------------------------------------------------------------
  // The board's scroll/clip container ends at (or before) the panel's left
  // edge — i.e. the panel is a flex sibling that PUSHES the board, never an
  // overlay painted on top of it. This pins the dock relationship directly:
  // the visible board well (the `overflow-x-auto` scroll viewport) and the
  // panel abut, they do not overlap.
  // -------------------------------------------------------------------------

  it("the board's visible scroll viewport ends at the panel's left edge", async () => {
    const { unmount } = renderApp();
    await flushAppMount();

    const panelRect = readOpenPanelRect();

    const boardZone = document.querySelector<HTMLElement>(
      '[data-segment^="board:"]',
    );
    expect(boardZone, "the board zone must be mounted").not.toBeNull();
    const scroller = boardZone!.querySelector<HTMLElement>(".overflow-x-auto");
    expect(
      scroller,
      "the board must own an overflow-x-auto scroll container",
    ).not.toBeNull();

    const scrollerRect = scroller!.getBoundingClientRect();
    // A 0.5px tolerance absorbs sub-pixel rounding from the flex layout and
    // the panel's 1px left border without admitting a real overlap.
    expect(
      scrollerRect.right,
      `the board scroll viewport (right=${scrollerRect.right.toFixed(1)}) must not extend past the panel's left edge (${panelRect.left.toFixed(1)})`,
    ).toBeLessThanOrEqual(panelRect.left + 0.5);

    unmount();
  });

  // -------------------------------------------------------------------------
  // End-to-end symptom guard: with the panel open, opening the real Jump-To
  // overlay must not paint a board scope's code pill on top of a panel
  // scope's code pill. This drives the production trigger (cua Mod+G), reads
  // the real `[data-jump-code]` pills the overlay paints, and asserts no
  // board pill rect overlaps any panel pill rect.
  //
  // This is the user-facing symptom from the task: "code pills for the
  // panel's controls visually collide with the jump-label pills for the
  // board cards." It exercises the same `JumpPill` painting path the user
  // sees, so it stays green only while the dock keeps board geometry left of
  // the panel.
  // -------------------------------------------------------------------------

  it("Jump-To paints no board pill overlapping a panel pill while the panel is open", async () => {
    // Viewport-sized so the docked panel is inside the real viewport the
    // overlay's visibility filter hit-tests — see `renderAppViewport`.
    const { unmount } = renderAppViewport();
    await flushAppMount();

    readOpenPanelRect();

    // Seed focus on a board card so the overlay enumerates the window layer
    // (the layer that owns both the board scopes and the panel scopes).
    const cardFq = harness.getRegisteredFqBySegment("task:T1");
    expect(cardFq, "task:T1 must register before pre-focus").not.toBeNull();
    await harness.fireFocusChanged({
      next_fq: cardFq!,
      next_segment: asSegment("task:T1"),
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 5));
    });

    // Open the overlay via cua Mod+G (browser-mode CI is macOS; `metaKey`
    // is the Mod modifier there).
    fireEvent.keyDown(document.body, { key: "g", metaKey: true });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    const overlay = await waitFor(() => {
      const el = document.querySelector('[data-testid="jump-to-overlay"]');
      expect(el, "jump-to overlay must mount after the trigger").not.toBeNull();
      return el as HTMLElement;
    });
    expect(overlay).not.toBeNull();

    // Partition the painted pills into board pills and panel pills by their
    // target FQM's trailing segment.
    const pills = Array.from(
      document.querySelectorAll<HTMLElement>("[data-jump-code]"),
    );
    expect(pills.length, "the overlay must paint code pills").toBeGreaterThan(
      0,
    );

    const segmentOf = (fq: string): string =>
      fq.includes("/") ? fq.slice(fq.lastIndexOf("/") + 1) : fq;
    const isBoardPill = (p: HTMLElement): boolean => {
      const seg = segmentOf(p.dataset.jumpFq ?? "");
      return (
        seg.startsWith("task:") ||
        seg.startsWith("column:") ||
        seg.startsWith("board:")
      );
    };
    const isPanelPill = (p: HTMLElement): boolean =>
      segmentOf(p.dataset.jumpFq ?? "").startsWith("ui:ai-panel");

    const boardPills = pills.filter(isBoardPill);
    const panelPills = pills.filter(isPanelPill);
    expect(
      boardPills.length,
      "the overlay must enumerate board scopes",
    ).toBeGreaterThan(0);
    expect(
      panelPills.length,
      "the overlay must enumerate panel scopes (panel is open)",
    ).toBeGreaterThan(0);

    // No board pill may intersect any panel pill.
    const rectsOverlap = (a: DOMRect, b: DOMRect): boolean =>
      a.left < b.right &&
      b.left < a.right &&
      a.top < b.bottom &&
      b.top < a.bottom;

    const collisions: string[] = [];
    for (const bp of boardPills) {
      const br = bp.getBoundingClientRect();
      for (const pp of panelPills) {
        const pr = pp.getBoundingClientRect();
        if (rectsOverlap(br, pr)) {
          collisions.push(
            `${bp.dataset.jumpFq} (${bp.dataset.jumpCode}) overlaps ${pp.dataset.jumpFq} (${pp.dataset.jumpCode})`,
          );
        }
      }
    }

    expect(
      collisions,
      `board pills must not overlap panel pills:\n${collisions.join("\n")}`,
    ).toEqual([]);

    unmount();
  });
});
