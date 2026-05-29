/**
 * Occlusion / viewport-filtering test for the Jump-To overlay — mounts the
 * full production `<App/>` with the AI panel docked open and a horizontally
 * overflowing board, then asserts the Jump-To overlay only labels scopes that
 * are actually visible.
 *
 * Source of truth for kanban task `01KSAQWXWSPP37V160HWM7YSEK` (Jump-To
 * occlusion/viewport filtering follow-up).
 *
 * # What this pins
 *
 * Standard vim-sneak / AceJump semantics: you can only jump to what you can
 * see. A board scope whose pill-anchor point (`rect.left + 4`, `rect.top + 4`,
 * the same point `JumpPill` paints at) is scrolled into the board's right-side
 * overflow region — past the visible scroll viewport and underneath the docked
 * AI panel — is NOT visible, so it must NOT get a `[data-jump-code]` pill.
 *
 *   1. **Occlusion drop** — at least one board column is scrolled fully into
 *      the overflow region so its pill anchor lands at/after the panel's left
 *      edge; that column (and its cards) must produce NO pill.
 *   2. **No overlapping pills** — no two rendered pill rects intersect (the
 *      user-facing collision symptom).
 *   3. **No over-filtering** — every column whose anchor is inside the visible
 *      well DOES get a pill (positive guard; the negative test below pairs
 *      with a fits-on-screen positive test).
 *
 * # Harness
 *
 * Mirrors `spatial-nav-jump-to.spatial.test.tsx` and
 * `ai-panel-dock.spatial.test.tsx` — the same Tauri IPC mock triple, the same
 * end-to-end board fixture, and a Tailwind-substitute stylesheet so the
 * production class strings lay out in real Chromium without `@tailwindcss/vite`.
 *
 * The overflow is forced by mounting `<App/>` in a deliberately NARROW viewport
 * (900 px). With the 420 px default panel docked open the board's visible well
 * is ~480 px wide — narrower than the three 24em (384 px) columns laid
 * side-by-side, so the rightmost column is pushed entirely into the off-screen
 * overflow region whose horizontal span sits under the panel. This is the exact
 * "column entirely in the right-side overflow at scrollLeft=0" case the dock
 * task flagged as a separate Jump-To occlusion concern.
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
import { commandToolCall } from "@/test/mock-command-list";
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
// plus the AI panel seams so the panel renders its expanded body, plus
// `generate_jump_codes` for the Jump-To overlay.
// ---------------------------------------------------------------------------

async function bootstrapInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "command_tool_call") return commandToolCall(args);
  if (cmd === "list_entity_types") return listEntityTypesResponse();
  if (cmd === "get_entity_schema") {
    const a = (args ?? {}) as Record<string, unknown>;
    return getEntitySchemaResponse(String(a.entityType));
  }
  if (cmd === "get_board_data") return getBoardDataResponse();
  if (cmd === "list_entities") {
    const a = (args ?? {}) as Record<string, unknown>;
    return listEntitiesResponse(String(a.entityType));
  }
  if (cmd === "list_open_boards") return listOpenBoardsResponse();
  if (cmd === "list_views") return listViewsResponse();
  if (cmd === "get_ui_state") return getUIStateResponse("cua");
  if (cmd === "get_undo_state") return getUndoStateResponse();
  if (cmd === "dispatch_command") {
    const a = (args ?? {}) as Record<string, unknown>;
    if (a.cmd === "perspective.list") return perspectiveListDispatchResponse();
    return { result: null, undoable: false };
  }
  if (cmd === "list_commands_for_scope") return [];
  // AI panel seams — one available model so the Container auto-selects it and
  // renders the expanded panel body rather than the no-model dead-end.
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
// the App.tsx → board chain and the AiPanelShell dock. Mirrors the dock test.
// ---------------------------------------------------------------------------

/**
 * The App is sized to the REAL browser viewport (`100vw` / `100vh`), not a
 * fixed-width wrapper.
 *
 * This is load-bearing for an `elementFromPoint`-based test. `elementFromPoint`
 * hit-tests against the actual browser viewport — under the Playwright provider
 * the test runs in a fixed viewport (~414×896 px in CI), and any point outside
 * it returns `null`. The sibling dock / spatial-nav tests size the App to a
 * 1400 px wrapper because they only ever read `getBoundingClientRect` (which
 * honours the wrapper, not the viewport) and never hit-test. Here the
 * production occlusion filter and the test's own ground-truth probe both call
 * `elementFromPoint`, so the App MUST lay out within the viewport the hit-test
 * actually sees. Sizing to the viewport makes the off-screen / under-panel
 * regions real for both.
 *
 * With the three 24em (384 px) columns laid side-by-side and the panel docked
 * open at its default width (clamped to 85vw on a narrow viewport), the board
 * overflows horizontally: the leftmost column is visible in the well, the
 * middle column's pill anchor lands under the panel, and the rightmost
 * column's anchor is off the viewport entirely — exactly the occlusion set the
 * filter must drop.
 */
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
  if (document.querySelector("style[data-test-jumpto-occlusion]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-jumpto-occlusion", "");
  style.textContent = TEST_LAYOUT_CSS;
  document.head.appendChild(style);
}

/**
 * Mount the full production `<App/>` sized to the real browser viewport.
 *
 * `100vw` / `100vh` so the layout the production occlusion filter hit-tests
 * (and the test's own `elementFromPoint` probe) matches what the viewport can
 * actually return — see the note on {@link TEST_LAYOUT_CSS}.
 */
function renderApp() {
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

/** Read the open panel's rect after mount, asserting it is present + expanded. */
function readOpenPanelRect(): DOMRect {
  const panel = document.querySelector<HTMLElement>(
    '[data-testid="ai-panel-container"]',
  );
  expect(panel, "the AI panel dock must be mounted").not.toBeNull();
  expect(
    panel!.getAttribute("data-ai-panel-collapsed"),
    "the AI panel must be expanded (open) for this occlusion contract",
  ).toBe("false");
  const panelRect = panel!.getBoundingClientRect();
  expect(panelRect.width, "the open panel must have width").toBeGreaterThan(0);
  return panelRect;
}

/** Open the Jump-To overlay via cua Mod+G, returning the overlay element. */
async function openJumpTo(harness: SpatialHarness): Promise<HTMLElement> {
  const cardFq = harness.getRegisteredFqBySegment("task:T1");
  expect(cardFq, "task:T1 must register before pre-focus").not.toBeNull();
  await harness.fireFocusChanged({
    next_fq: cardFq!,
    next_segment: asSegment("task:T1"),
  });
  await act(async () => {
    await new Promise((r) => setTimeout(r, 5));
  });
  // Cua mode binds `Mod+G` → `nav.jump`; browser-mode CI is macOS, so `metaKey`
  // is the Mod modifier there.
  fireEvent.keyDown(document.body, { key: "g", metaKey: true });
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
  return waitFor(() => {
    const el = document.querySelector('[data-testid="jump-to-overlay"]');
    expect(el, "jump-to overlay must mount after the trigger").not.toBeNull();
    return el as HTMLElement;
  });
}

/** Read every code pill currently painted by the overlay. */
function jumpPills(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-jump-code]"));
}

/** Trailing segment of an FQM (the legacy moniker). */
function segmentOf(fq: string): string {
  return fq.includes("/") ? fq.slice(fq.lastIndexOf("/") + 1) : fq;
}

/** Every board-content scope host currently in the DOM (board/column/card). */
function boardScopeHosts(): HTMLElement[] {
  return Array.from(
    document.querySelectorAll<HTMLElement>("[data-segment]"),
  ).filter((h) => {
    const seg = h.dataset.segment ?? "";
    return (
      seg.startsWith("task:") ||
      seg.startsWith("column:") ||
      seg.startsWith("board:")
    );
  });
}

/** Pill-anchor offset — mirrors `PILL_ANCHOR_OFFSET` in `jump-to-overlay.tsx`. */
const PILL_OFFSET_PX = 4;

/** Do two rects intersect (strictly, sharing positive-area overlap)? */
function rectsOverlap(a: DOMRect, b: DOMRect): boolean {
  return (
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
  );
}

/**
 * Ground-truth visibility probe — INDEPENDENT of the production code path.
 *
 * Returns true iff a hit-test at the host's pill anchor lands on the host or a
 * descendant — i.e. the host is the topmost painted surface at exactly the
 * point its pill would paint. This is the user-observable "can I see it?"
 * predicate; the test uses it to compute the expected visible set and compares
 * that against the pills the overlay actually paints, so the overlay is checked
 * both ways (no false drops, no false keeps) without the test asserting on the
 * overlay's internal mechanism.
 */
function hostAnchorVisible(host: HTMLElement): boolean {
  const r = host.getBoundingClientRect();
  if (r.width <= 0 || r.height <= 0) return false;
  const hit = document.elementFromPoint(
    r.left + PILL_OFFSET_PX,
    r.top + PILL_OFFSET_PX,
  );
  return hit !== null && host.contains(hit);
}

/** Partition board scope hosts into the visible set and the occluded set. */
function partitionBoardScopes(): {
  visible: HTMLElement[];
  occluded: HTMLElement[];
} {
  const visible: HTMLElement[] = [];
  const occluded: HTMLElement[] = [];
  for (const host of boardScopeHosts()) {
    const r = host.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) continue;
    (hostAnchorVisible(host) ? visible : occluded).push(host);
  }
  return { visible, occluded };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Jump-To overlay — occlusion / viewport filtering", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    harness = setupSpatialHarness({ defaultInvokeImpl: bootstrapInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Negative — occluded board scopes (anchor under the panel or off the
  // viewport) get NO pill, and no two pills overlap.
  // -------------------------------------------------------------------------

  it("drops board scopes whose pill anchor is occluded or off-screen", async () => {
    const { unmount } = renderApp();
    await flushAppMount();

    const panelRect = readOpenPanelRect();

    // Independently (NOT via the production filter) classify board scopes into
    // visible vs occluded using a ground-truth hit-test at each scope's pill
    // anchor. With the columns overflowing under / past the docked panel there
    // must be at least one occluded board scope, or the test is not exercising
    // the filter at all.
    const { visible, occluded } = partitionBoardScopes();
    expect(
      occluded.length,
      `the overflowing board must put at least one board scope under the ` +
        `panel or off-screen (panel.left=${panelRect.left.toFixed(1)}, ` +
        `innerW=${window.innerWidth})`,
    ).toBeGreaterThan(0);
    expect(
      visible.length,
      "at least one board scope must remain visible in the well",
    ).toBeGreaterThan(0);

    const occludedFqs = new Set(occluded.map((h) => h.dataset.moniker!));

    const overlay = await openJumpTo(harness);
    expect(overlay).not.toBeNull();

    const pills = jumpPills();
    expect(pills.length, "the overlay must paint some pills").toBeGreaterThan(
      0,
    );
    const pillFqs = new Set(pills.map((p) => p.dataset.jumpFq));

    // Every occluded board scope must be absent from the painted pill set.
    for (const fq of occludedFqs) {
      expect(
        pillFqs.has(fq),
        `occluded scope ${segmentOf(fq)} must NOT get a jump pill`,
      ).toBe(false);
    }

    // Every painted board pill must itself be visible at its anchor — no pill
    // paints under the panel or off-screen. This is the user-facing symptom,
    // checked directly against the painted pills (mechanism-agnostic).
    for (const p of pills) {
      const seg = segmentOf(p.dataset.jumpFq ?? "");
      const isBoardPill =
        seg.startsWith("task:") ||
        seg.startsWith("column:") ||
        seg.startsWith("board:");
      if (!isBoardPill) continue;
      const left = parseFloat(p.style.left || "0");
      const top = parseFloat(p.style.top || "0");
      const hit = document.elementFromPoint(left, top);
      expect(
        hit !== null,
        `board pill ${seg} at (${left.toFixed(1)}, ${top.toFixed(1)}) must ` +
          `paint on-screen (elementFromPoint returned null)`,
      ).toBe(true);
      expect(
        left < panelRect.left,
        `board pill ${seg} at left=${left.toFixed(1)} must paint left of the ` +
          `panel (${panelRect.left.toFixed(1)})`,
      ).toBe(true);
    }

    // No two rendered pills overlap.
    const collisions: string[] = [];
    for (let i = 0; i < pills.length; i++) {
      for (let j = i + 1; j < pills.length; j++) {
        const a = pills[i].getBoundingClientRect();
        const b = pills[j].getBoundingClientRect();
        if (rectsOverlap(a, b)) {
          collisions.push(
            `${pills[i].dataset.jumpFq} overlaps ${pills[j].dataset.jumpFq}`,
          );
        }
      }
    }
    expect(
      collisions,
      `no two pills may overlap; collisions:\n${collisions.join("\n")}`,
    ).toEqual([]);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Positive — the painted board pills are EXACTLY the ground-truth-visible
  // board scopes. This is the over-filtering guard: every visible board scope
  // gets a pill (no false drops) and no occluded one does (no false keeps).
  // -------------------------------------------------------------------------

  it("labels exactly the board scopes that are actually visible", async () => {
    const { unmount } = renderApp();
    await flushAppMount();

    readOpenPanelRect();

    // Jump pills land only on top-tier focusables (cards) — structural zones
    // (board well, columns: `data-focusable` absent) and nested focusables (a
    // card's fields: they have a focusable ancestor) are excluded, mirroring
    // the kernel's tier-locked nav. Restrict the expected set accordingly so
    // this test stays an OCCLUSION test (visible-vs-hidden) over the
    // jump-eligible scopes rather than re-asserting the old "pill on every
    // scope" model.
    const isTopTierFocusable = (h: HTMLElement): boolean =>
      h.dataset.focusable !== undefined &&
      !h.parentElement?.closest("[data-focusable]");
    const { visible } = partitionBoardScopes();
    const expectedVisibleFqs = new Set(
      visible.filter(isTopTierFocusable).map((h) => h.dataset.moniker!),
    );
    expect(
      expectedVisibleFqs.size,
      "at least one focusable board card must be visible",
    ).toBeGreaterThan(0);

    const overlay = await openJumpTo(harness);
    expect(overlay).not.toBeNull();

    // The board pills the overlay actually painted.
    const paintedBoardFqs = new Set(
      jumpPills()
        .map((p) => p.dataset.jumpFq!)
        .filter((fq) => {
          const seg = segmentOf(fq);
          return (
            seg.startsWith("task:") ||
            seg.startsWith("column:") ||
            seg.startsWith("board:")
          );
        }),
    );

    // No false drops: every visible board scope got a pill.
    const falseDrops = [...expectedVisibleFqs].filter(
      (fq) => !paintedBoardFqs.has(fq),
    );
    expect(
      falseDrops,
      `every visible board scope must get a pill (no over-filtering); ` +
        `missing:\n${falseDrops.map(segmentOf).join("\n")}`,
    ).toEqual([]);

    // No false keeps: every painted board pill corresponds to a visible scope.
    const falseKeeps = [...paintedBoardFqs].filter(
      (fq) => !expectedVisibleFqs.has(fq),
    );
    expect(
      falseKeeps,
      `every painted board pill must be a visible scope (no occluded keeps); ` +
        `extra:\n${falseKeeps.map(segmentOf).join("\n")}`,
    ).toEqual([]);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Positive — the docked panel's own scopes (on top, not occluded) still get
  // pills. Guards that the visibility filter does not nuke the panel surface
  // it is meant to protect from underdraw.
  // -------------------------------------------------------------------------

  it("still labels the docked panel's own visible scopes", async () => {
    const { unmount } = renderApp();
    await flushAppMount();

    readOpenPanelRect();

    // The panel's own top-tier focusable scopes that are visible on top of
    // everything. Jump pills land only on top-tier focusables, so restrict to
    // panel scopes that are focusable (`data-focusable`) with no focusable
    // ancestor — the panel's structural zones (`ui:ai-panel`, transcript
    // wrapper) and any nested focusables are excluded, mirroring the overlay's
    // tier filter.
    const isTopTierFocusable = (h: HTMLElement): boolean =>
      h.dataset.focusable !== undefined &&
      !h.parentElement?.closest("[data-focusable]");
    const visiblePanelFqs = new Set(
      Array.from(document.querySelectorAll<HTMLElement>("[data-segment]"))
        .filter((h) => (h.dataset.segment ?? "").startsWith("ui:ai-panel"))
        .filter(isTopTierFocusable)
        .filter((h) => hostAnchorVisible(h))
        .map((h) => h.dataset.moniker!),
    );

    expect(
      visiblePanelFqs.size,
      "the open panel must register at least one visible top-tier focusable scope",
    ).toBeGreaterThan(0);

    const overlay = await openJumpTo(harness);
    expect(overlay).not.toBeNull();

    const pillFqs = new Set(jumpPills().map((p) => p.dataset.jumpFq));
    for (const fq of visiblePanelFqs) {
      expect(
        pillFqs.has(fq),
        `visible panel scope ${segmentOf(fq)} must get a jump pill`,
      ).toBe(true);
    }

    unmount();
  });
});
