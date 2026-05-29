/**
 * End-to-end spatial-nav test for the Jump-To overlay — mounts the full
 * production `<App/>` and exercises the Jump-To pipeline against the real
 * provider tree.
 *
 * Source of truth for kanban task `01KQYWWQ0C4E9AFM942BMJHMFX`. Validates
 * the integration of the YAML registry → keybinding pipeline → AppShell
 * `jumpOpen` flag → `<JumpToOverlay>` → spatial focus dispatch.
 *
 * # Test cases
 *
 * 1. **vim `s` opens the overlay** — pill count matches enumerable scopes.
 * 2. **cua `Mod+G` opens the overlay**.
 * 3. **Typing a code moves focus to that scope** — `data-jump-code` /
 *    `data-jump-fq` from a pill, dispatch each letter, focus-changed
 *    fires with the matching FQM.
 * 4. **Esc dismisses without focus change**.
 * 5. **Non-matching letter dismisses without focus change**.
 * 6. **Multi-letter code requires both keystrokes** — needs >23 scopes
 *    so the sneak generator spills into 2-letter codes (the alphabet
 *    has 23 letters).
 * 7. **Global keybindings suppressed while overlay is open** — pick a
 *    key bound globally that is NOT in the sneak alphabet so it neither
 *    matches a code nor extends a prefix.
 *
 * # No stubs
 *
 * Per the task: no stubbing of `JumpToOverlay`, `enumerateScopesInLayer`,
 * or `generateSneakCodes`. The overlay's React-side logic, the spatial
 * focus context's enumeration, and the sneak code generation are all
 * exercised end-to-end. The only mocked surface is the Tauri IPC
 * boundary itself: `generate_jump_codes` is answered from a JSON
 * fixture (`@/test/fixtures/sneak-fixture.json`) that is emitted by the
 * Rust test `swissarmyhammer-focus/tests/sneak_fixture.rs` from the
 * authoritative `generate_sneak_codes` implementation. The kernel isn't
 * running in the browser test, so the IPC boundary has to answer the
 * same way the Rust crate would; pinning the answers in a fixture file
 * makes drift impossible by construction. This is the same pattern the
 * rest of the spatial-nav test suite uses for `spatial_navigate` etc.
 *
 * # Layout note
 *
 * Mirrors `spatial-nav-end-to-end.spatial.test.tsx`'s Tailwind-substitute
 * stylesheet so the production class strings resolve to a row-of-columns
 * layout in browser-mode without `@tailwindcss/vite`. The `<App/>` is
 * pinned to a 1400×900 viewport so the column geometry is deterministic.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, waitFor, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — see `spatial-nav-end-to-end.spatial.test.tsx` for the
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
  PhysicalSize: class {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
  PhysicalPosition: class {
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
import { JumpToOverlay } from "@/components/jump-to-overlay";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { mkRect, stubScopeGeometry } from "@/test/stub-scope-geometry";
import {
  asSegment,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
} from "@/types/spatial";
import * as React from "react";

// ---------------------------------------------------------------------------
// Sneak code generator — fixture-backed mock of the Rust kernel.
//
// SOURCE OF TRUTH: `swissarmyhammer-focus/src/sneak.rs::generate_sneak_codes`.
//
// The Rust kernel is not running in the browser test, so the
// `generate_jump_codes` IPC boundary has to answer the same way the
// kernel would. Rather than re-implementing the algorithm in TS (which
// would silently drift from the Rust source the moment either side
// changed), the test consumes a JSON fixture written by a Rust test
// that runs `generate_sneak_codes(n)` over a representative count set:
//
//   - Generator: `swissarmyhammer-focus/tests/sneak_fixture.rs`
//   - Fixture:   `kanban-app/ui/src/test/fixtures/sneak-fixture.json`
//
// The fixture is keyed by stringified count. Drift detection is
// structural and self-contained: the default Rust test path
// (`cargo test -p swissarmyhammer-focus --test sneak_fixture`) reads
// the fixture and asserts it matches what `generate_sneak_codes`
// produces today. A drifted fixture fails the test directly. Regenerate
// (only when the algorithm intentionally changed) with:
//
//   BLESS=1 cargo test -p swissarmyhammer-focus --test sneak_fixture
// ---------------------------------------------------------------------------

import sneakFixture from "@/test/fixtures/sneak-fixture.json";

/**
 * Look up the pre-computed sneak codes for `count` from the fixture.
 *
 * The fixture is generated by Rust over a dense range that covers every
 * count the Jump-To overlay realistically presents in this test (the
 * 3×3 board fixture mounts ~30-50 enumerable scopes). If the count is
 * outside the fixture's coverage the lookup throws explicitly so a
 * future test that exercises an uncovered count fails loudly rather
 * than silently returning the wrong codes.
 */
function lookupSneakCodes(count: number): string[] {
  const codes = (sneakFixture as Record<string, string[] | undefined>)[
    String(count)
  ];
  if (codes === undefined) {
    throw new Error(
      `sneak-fixture.json has no entry for count=${count}; ` +
        `regenerate via \`cargo test -p swissarmyhammer-focus --test sneak_fixture\` ` +
        `with the count added to fixture_counts() in tests/sneak_fixture.rs`,
    );
  }
  return codes;
}

// ---------------------------------------------------------------------------
// Bootstrap-invoke impl — covers the Tauri commands the App fires on mount,
// plus the Jump-To-specific `generate_jump_codes`.
//
// The keymap mode is read from a closure-scoped `currentKeymapMode` so a
// single test can opt into vim mode for case 1 without minting a parallel
// fixture.
// ---------------------------------------------------------------------------

/**
 * Per-test keymap mode. Reset to `"cua"` in `beforeEach`; flipped to
 * `"vim"` by tests that need the vim binding (`s` → `nav.jump`).
 */
let currentKeymapMode: "vim" | "cua" | "emacs" = "cua";

/**
 * Build the bootstrap-invoke handler. The returned function answers every
 * Tauri command the production tree fires on mount, plus
 * `generate_jump_codes` for the Jump-To overlay's sneak code generator.
 *
 * Closes over the module-scope `currentKeymapMode` so per-test mutation
 * of that variable is reflected in subsequent `get_ui_state` calls.
 */
async function bootstrapInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "command_tool_call") return commandToolCall(args);
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
  // UI state — override `keymap_mode` per test.
  if (cmd === "get_ui_state") {
    const base = getUIStateResponse();
    return { ...base, keymap_mode: currentKeymapMode };
  }
  if (cmd === "get_undo_state") return getUndoStateResponse();
  // Command dispatch
  if (cmd === "dispatch_command") {
    const a = (args ?? {}) as Record<string, unknown>;
    if (a.cmd === "perspective.list") return perspectiveListDispatchResponse();
    if (a.cmd === "perspective.switch")
      return { result: null, undoable: false };
    if (a.cmd === "perspective.save") return { result: null, undoable: false };
    if (a.cmd === "perspective.rename") return { result: null, undoable: true };
    if (a.cmd === "view.set") return { result: null, undoable: false };
    if (a.cmd === "ui.inspect") return { result: null, undoable: false };
    if (a.cmd === "ui.setFocus") return { result: null, undoable: false };
    if (a.cmd === "file.switchBoard") return { result: null, undoable: false };
    return { result: null, undoable: false };
  }
  if (cmd === "list_commands_for_scope") return [];
  // Jump-To sneak code generator — fixture-backed mock of the Rust
  // kernel. See the SOURCE OF TRUTH comment block above for how the
  // fixture is generated and kept in sync.
  if (cmd === "generate_jump_codes") {
    const a = (args ?? {}) as Record<string, unknown>;
    // The IPC contract for `generate_jump_codes` requires a numeric
    // `count`. Throw explicitly if it is missing or non-numeric so a
    // future refactor that drops the arg surfaces as a loud test
    // failure rather than silently falling back to count=0 (which the
    // fixture answers with `[]` — equally silent and equally wrong).
    if (typeof a.count !== "number" || !Number.isFinite(a.count)) {
      throw new Error(
        `generate_jump_codes invoked without numeric count; got ${typeof a.count} (${String(a.count)})`,
      );
    }
    return lookupSneakCodes(a.count);
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// Layout substitute — same shape as `spatial-nav-end-to-end.spatial.test.tsx`
// ---------------------------------------------------------------------------

const TEST_VIEWPORT_WIDTH_PX = 1400;
const TEST_VIEWPORT_HEIGHT_PX = 900;

/**
 * CSS substitute for the production Tailwind output. The browser test
 * project does not load `@tailwindcss/vite`, so utility classes have to
 * be hand-defined for the production layout chain to lay out three
 * columns side-by-side.
 */
const TEST_LAYOUT_CSS = `
  .h-screen { height: 100vh; }
  .flex { display: flex; }
  .flex-col { flex-direction: column; }
  .flex-row { flex-direction: row; }
  .flex-1 { flex: 1 1 0%; min-width: 0; min-height: 0; }
  .min-h-0 { min-height: 0; }
  .min-w-0 { min-width: 0; }
  .overflow-hidden { overflow: hidden; }
  .overflow-x-auto { overflow-x: auto; }
  .overflow-y-auto { overflow-y: auto; }
  .relative { position: relative; }
  .absolute { position: absolute; }
  .min-w-\\[24em\\] { min-width: 24em; }
  .max-w-\\[48em\\] { max-width: 48em; }
  .shrink-0 { flex-shrink: 0; }
  .h-12 { height: 3rem; }
`;

/** Inject the layout substitute stylesheet exactly once per document. */
function ensureTestLayoutCss(): void {
  if (document.querySelector("style[data-test-jumpto-layout]")) return;
  const style = document.createElement("style");
  style.setAttribute("data-test-jumpto-layout", "");
  style.textContent = TEST_LAYOUT_CSS;
  document.head.appendChild(style);
}

/** Mount the full production `<App/>` inside a viewport-sized wrapper. */
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Wait long enough for the App's bootstrap chain to complete. See the
 * sibling end-to-end test for the rationale on the 250ms nudge.
 */
async function flushAppMount() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 250));
  });
}

/**
 * Wait for the Jump-To overlay to appear. The overlay is async on open
 * because `useJumpTargets` awaits the `generate_jump_codes` IPC; the
 * sneak port resolves on a microtask, so the overlay paints after one
 * tick. Combined with the registry's `LayerScopeRegistry` populating on
 * mount, ~50ms is more than enough.
 */
async function waitForJumpOverlay(): Promise<HTMLElement> {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 30));
  });
  const overlay = await waitFor(() => {
    const el = document.querySelector('[data-testid="jump-to-overlay"]');
    expect(
      el,
      "jump-to overlay must mount after the trigger keystroke",
    ).not.toBeNull();
    return el as HTMLElement;
  });
  return overlay;
}

/** Read every code pill currently rendered. */
function jumpPills(): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-jump-code]"));
}

/**
 * Pre-focus a card so the Jump-To overlay's `priorFocusedFq` is non-null
 * and the enumeration walks the window-root layer. Returns the FQM the
 * focus claim was seeded with.
 */
async function seedCardFocus(
  harness: SpatialHarness,
  segment: string,
): Promise<FullyQualifiedMoniker> {
  const fq = harness.getRegisteredFqBySegment(segment);
  expect(fq, `${segment} must register before pre-focus`).not.toBeNull();
  await harness.fireFocusChanged({
    next_fq: fq!,
    next_segment: asSegment(segment),
  });
  // One macrotask so the React tree's focus-claim listener flips
  // `data-focused` before subsequent assertions.
  await act(async () => {
    await new Promise((r) => setTimeout(r, 5));
  });
  return fq!;
}

/**
 * Compute the expected jump-target count under the overlay's visibility
 * contract.
 *
 * The overlay enumerates every scope in the topmost layer, then drops any
 * whose pill anchor is not actually visible — off-screen, scrolled out of a
 * clipping ancestor, or occluded by a higher surface (see
 * `useJumpTargets` / `isScopeAnchorVisible` in `jump-to-overlay.tsx`). Pills
 * paint only for VISIBLE scopes (vim-sneak / AceJump "you can only jump to
 * what you can see" semantics).
 *
 * This helper mirrors that contract: it counts every scope host
 * (`[data-segment]`) whose pill anchor (`rect.left + 4`, `rect.top + 4`)
 * passes the same `elementFromPoint` hit-test the overlay uses — the topmost
 * element there is the host or one of its descendants. The pill count must
 * equal this visible count, not the raw enumerable count (some scopes are
 * scrolled off the board's overflow well in the test viewport).
 */
function countVisibleScopes(): number {
  const hosts = Array.from(
    document.querySelectorAll<HTMLElement>("[data-segment]"),
  );
  let count = 0;
  for (const h of hosts) {
    // Jump pills land only on **top-tier focusables** — the navigation units
    // (cards, buttons): focusable scopes (`data-focusable`) whose nearest
    // focusable ancestor is none. Structural zones (`data-focusable` absent)
    // and nested focusables (a card's fields, which have a focusable ancestor)
    // are excluded — reached by drill-in, not jump — mirroring the kernel's
    // tier-locked nav and the overlay's tier filter.
    if (h.dataset.focusable === undefined) continue;
    if (h.parentElement?.closest("[data-focusable]")) continue;
    const r = h.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) continue;
    const hit = document.elementFromPoint(r.left + 4, r.top + 4);
    if (hit !== null && h.contains(hit)) count += 1;
  }
  return count;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("End-to-end spatial test for Jump-To overlay — full <App/>", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    currentKeymapMode = "cua";
    harness = setupSpatialHarness({ defaultInvokeImpl: bootstrapInvokeImpl });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // Case 1 — vim `s` opens the overlay, pill count matches enumerable scopes.
  // -------------------------------------------------------------------------

  it("vim `s` opens the overlay; pill count matches the visible scope count", async () => {
    currentKeymapMode = "vim";
    const { unmount } = renderApp();
    await flushAppMount();

    // Pre-focus a card so the overlay's enumeration walks the
    // window-root layer (the layer that owns the focused card).
    await seedCardFocus(harness, "task:T1");

    // Capture the VISIBLE-scope count BEFORE opening — once the overlay
    // mounts its own `<FocusLayer>` and sentinel, the DOM grows by the
    // sentinel host and (per `useJumpTargets`'s mount-time capture) every
    // visible scope host gets a pill. Off-screen / occluded scopes are
    // filtered out, so the predictor is the visible count, not the raw
    // enumerable count.
    const expectedPills = countVisibleScopes();
    expect(
      expectedPills,
      "App must register some visible focusable scopes before Jump-To opens",
    ).toBeGreaterThan(0);

    // Vim mode binds `s` → `nav.jump`.
    fireEvent.keyDown(document.body, { key: "s" });
    const overlay = await waitForJumpOverlay();
    expect(overlay).not.toBeNull();

    const pills = jumpPills();
    expect(
      pills.length,
      `pill count (${pills.length}) must equal visible scope count (${expectedPills})`,
    ).toBe(expectedPills);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 2 — cua `Mod+G` opens the overlay.
  // -------------------------------------------------------------------------

  it("cua `Mod+G` opens the overlay", async () => {
    currentKeymapMode = "cua";
    const { unmount } = renderApp();
    await flushAppMount();

    await seedCardFocus(harness, "task:T1");

    const expectedPills = countVisibleScopes();
    expect(expectedPills).toBeGreaterThan(0);

    // Cua mode binds `Mod+G` → `nav.jump`. The browser-mode tests run
    // on macOS in CI; `normalizeKeyEvent` derives `Mod` from `metaKey`
    // on macOS and from `ctrlKey` on other platforms (see
    // `keybindings.ts` — `mod = mac ? e.metaKey : e.ctrlKey`).
    // Setting only `metaKey` produces `Mod+g` which matches the cua
    // binding. Setting `ctrlKey: true` as well would add a separate
    // `Ctrl` modifier on macOS (see the `mac && e.ctrlKey` branch),
    // producing `Mod+Ctrl+g` and missing the binding.
    fireEvent.keyDown(document.body, { key: "g", metaKey: true });
    const overlay = await waitForJumpOverlay();
    expect(overlay).not.toBeNull();
    const pills = jumpPills();
    expect(pills.length).toBe(expectedPills);

    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 3 — Typing a code moves focus to that scope.
  // -------------------------------------------------------------------------

  it("typing a code dispatches focus to the matching scope and dismisses the overlay", async () => {
    currentKeymapMode = "vim";
    const { unmount } = renderApp();
    await flushAppMount();

    await seedCardFocus(harness, "task:T1");

    fireEvent.keyDown(document.body, { key: "s" });
    const overlay = await waitForJumpOverlay();

    // Pick the first pill and read both its code and target FQM. Codes
    // are at most two letters; this works for both single and
    // multi-letter cases.
    const pills = jumpPills();
    expect(pills.length).toBeGreaterThan(0);
    const chosen = pills[0];
    const code = chosen.dataset.jumpCode;
    const targetFq = chosen.dataset.jumpFq;
    expect(code, "first pill must carry a data-jump-code").toBeTruthy();
    expect(targetFq, "first pill must carry a data-jump-fq").toBeTruthy();

    // Reset the focus-call spy so we observe only the overlay-driven
    // dispatch.
    mockInvoke.mockClear();

    // Type each letter of the code as a separate keydown on the
    // overlay's host (the keydown handler is bound to the overlay
    // chrome's wrapper div). The handler treats each printable letter
    // as one buffer extension.
    for (const ch of code!) {
      await act(async () => {
        fireEvent.keyDown(overlay, { key: ch });
      });
    }
    // Let microtasks settle so the spatial_focus IPC's queueMicrotask
    // emit-after-write fires before assertions.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });

    const focusCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_focus")
      .map((c) => (c[1] as { fq: FullyQualifiedMoniker }).fq);
    expect(
      focusCalls,
      `spatial_focus must be dispatched against the chosen pill's FQM (${targetFq})`,
    ).toContain(targetFq as FullyQualifiedMoniker);

    // Overlay dismissed on match.
    expect(
      document.querySelector('[data-testid="jump-to-overlay"]'),
      "overlay must unmount after a unique-code match",
    ).toBeNull();

    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 4 — Esc dismisses without focus change.
  // -------------------------------------------------------------------------

  it("Esc dismisses the overlay without dispatching a new focus", async () => {
    currentKeymapMode = "vim";
    const { unmount } = renderApp();
    await flushAppMount();

    const priorFq = await seedCardFocus(harness, "task:T1");

    fireEvent.keyDown(document.body, { key: "s" });
    await waitForJumpOverlay();

    // Capture every pill FQM so we can assert no focus call hit one of
    // them after Esc.
    const pillFqs = new Set(jumpPills().map((p) => p.dataset.jumpFq));

    mockInvoke.mockClear();
    fireEvent.keyDown(document.body, { key: "Escape" });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30));
    });

    // Overlay closed.
    await waitFor(() => {
      expect(
        document.querySelector('[data-testid="jump-to-overlay"]'),
        "overlay must unmount on Esc",
      ).toBeNull();
    });

    // No spatial_focus call landed on a pill's FQM. (The overlay
    // restores prior focus, which fires `spatial_focus(priorFq)` —
    // that's expected and explicitly NOT a "new" focus change.)
    const focusCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_focus")
      .map((c) => (c[1] as { fq: FullyQualifiedMoniker }).fq);
    for (const fq of focusCalls) {
      // Prior-focus restore is permitted; pill targets are not.
      if (fq === priorFq) continue;
      expect(
        pillFqs.has(fq as string),
        `Esc must not dispatch focus to a pill target (got ${fq})`,
      ).toBe(false);
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 5 — Non-matching letter dismisses without focus change.
  // -------------------------------------------------------------------------

  it("a letter that is not a valid code prefix dismisses without focus dispatch", async () => {
    currentKeymapMode = "vim";
    const { unmount } = renderApp();
    await flushAppMount();

    const priorFq = await seedCardFocus(harness, "task:T1");

    fireEvent.keyDown(document.body, { key: "s" });
    const overlay = await waitForJumpOverlay();

    const pills = jumpPills();
    const codes = new Set(pills.map((p) => p.dataset.jumpCode!));
    const pillFqs = new Set(pills.map((p) => p.dataset.jumpFq!));

    // The sneak alphabet skips `i`, `l`, `o` (high-confusion letters)
    // and the four corner letters — `i` / `l` / `o` are never prefixes
    // or matches in any sneak code, so they are the canonical
    // non-matching letters for this case. The overlay's keydown
    // handler accepts them (printable single ASCII letters pass the
    // `/[a-zA-Z]/` filter) but the buffered match falls through to
    // the no-match flash → dismiss path.
    const NEVER_USED_LETTERS = ["i", "l", "o"] as const;
    let nonPrefixLetter: string | null = null;
    for (const letter of NEVER_USED_LETTERS) {
      const isPrefix = Array.from(codes).some((c) => c.startsWith(letter));
      if (!isPrefix) {
        nonPrefixLetter = letter;
        break;
      }
    }
    expect(
      nonPrefixLetter,
      "fixture must produce at least one alphabet letter that is not a code prefix",
    ).not.toBeNull();

    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.keyDown(overlay, { key: nonPrefixLetter! });
    });
    // The non-match path flashes for 150ms before dismissing — wait
    // longer than the flash window for the dismiss to land.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 200));
    });

    await waitFor(() => {
      expect(
        document.querySelector('[data-testid="jump-to-overlay"]'),
        "overlay must unmount after a non-matching letter",
      ).toBeNull();
    });

    // No pill-target focus dispatch happened (prior-focus restore is
    // permitted as part of the dismiss path).
    const focusCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_focus")
      .map((c) => (c[1] as { fq: FullyQualifiedMoniker }).fq);
    for (const fq of focusCalls) {
      if (fq === priorFq) continue;
      expect(
        pillFqs.has(fq as string),
        `non-matching letter must not dispatch focus to a pill target (got ${fq})`,
      ).toBe(false);
    }

    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 6 — Multi-letter code requires both keystrokes.
  //
  // The sneak alphabet has 23 letters; counts ≤ 23 produce only
  // single-letter codes, counts > 23 spill into 2-letter codes. To force a
  // 2-letter code deterministically we need MORE THAN 23 *visible* scopes —
  // and visibility now matters: the overlay drops off-screen / occluded
  // scopes (see Cases 1/2). The full <App/> at the test viewport exposes
  // fewer than 24 visible scopes (the board overflows and the AI panel
  // occludes the rest), so this case mounts a controlled set of 28 visible
  // leaf scopes in a window layer instead — laid out as a non-overlapping
  // grid that fits the test viewport so every scope passes the visibility
  // hit-test. It still drives the REAL `<JumpToOverlay>` and the REAL
  // Rust-generated sneak codes (the fixture), which is the unique coverage
  // this case adds over the synthetic browser-test 2-letter case.
  // -------------------------------------------------------------------------

  it("multi-letter code: first letter narrows the buffer, second letter dispatches focus", async () => {
    currentKeymapMode = "vim";

    // 28 scopes → > 23, so the sneak generator spills into 2-letter codes.
    // Lay them out as a non-overlapping grid that fits the test browser
    // viewport (≤ ~414 px wide) so all 28 pass the overlay's anchor
    // visibility hit-test (vim-sneak: only visible scopes get a pill).
    const SCOPE_COUNT = 28;
    const COLS = 6;
    const CELL_W = 60;
    const CELL_H = 24;
    const GUTTER = 6;
    const rects = new Map<string, DOMRect>();
    for (let i = 0; i < SCOPE_COUNT; i++) {
      const col = i % COLS;
      const row = Math.floor(i / COLS);
      const x = 4 + col * (CELL_W + GUTTER);
      const y = 4 + row * (CELL_H + GUTTER);
      rects.set(`seed-${i}`, mkRect(x, y, CELL_W, CELL_H));
    }
    const cleanupRects = stubScopeGeometry(rects);

    /**
     * Defer the overlay open one tick so seed scopes register first, and own
     * the open-state so `onClose` actually unmounts the overlay — mirroring
     * `AppShell`'s `jumpOpen` flag (a bare `vi.fn()` `onClose` would leave the
     * overlay mounted after a match, so the dismiss assertion would never see
     * it unmount).
     */
    function DeferredJumpToOverlay({ onClose }: { onClose: () => void }) {
      const [open, setOpen] = React.useState(false);
      React.useEffect(() => {
        const id = setTimeout(() => setOpen(true), 0);
        return () => clearTimeout(id);
      }, []);
      return (
        <JumpToOverlay
          open={open}
          onClose={() => {
            setOpen(false);
            onClose();
          }}
        />
      );
    }

    const onClose = vi.fn();
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            {Array.from({ length: SCOPE_COUNT }, (_, i) => (
              <FocusScope
                key={i}
                moniker={asSegment(`scope:${i}`)}
                data-testid={`seed-${i}`}
              >
                <span>scope {i}</span>
              </FocusScope>
            ))}
            <DeferredJumpToOverlay onClose={onClose} />
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    const overlay = await waitForJumpOverlay();

    // All 28 visible scopes get a pill; > 23 forces at least one 2-letter
    // code (the sneak generator emits single-letter codes first).
    const pills = jumpPills();
    expect(
      pills.length,
      `all ${SCOPE_COUNT} visible scopes must get a pill`,
    ).toBe(SCOPE_COUNT);
    const twoLetterPill = pills.find(
      (p) => (p.dataset.jumpCode ?? "").length === 2,
    );
    expect(
      twoLetterPill,
      "with >23 scopes the sneak generator must emit at least one 2-letter code",
    ).toBeTruthy();
    const code = twoLetterPill!.dataset.jumpCode!;
    const targetFq = twoLetterPill!.dataset.jumpFq!;
    expect(code.length).toBe(2);

    mockInvoke.mockClear();

    // First letter — must NOT dispatch focus or close the overlay.
    await act(async () => {
      fireEvent.keyDown(overlay, { key: code[0] });
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
    expect(
      document.querySelector('[data-testid="jump-to-overlay"]'),
      "overlay must remain mounted after a single prefix letter",
    ).not.toBeNull();
    const firstLetterFocusCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(
      firstLetterFocusCalls.length,
      "first letter of a 2-letter code must not dispatch a new spatial_focus",
    ).toBe(0);

    // Second letter — completes the code; overlay dismisses with
    // focus dispatched against the matched pill's FQM.
    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.keyDown(overlay, { key: code[1] });
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
    const focusCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_focus")
      .map((c) => (c[1] as { fq: FullyQualifiedMoniker }).fq);
    expect(
      focusCalls,
      `second letter must dispatch spatial_focus against the matched pill (${targetFq})`,
    ).toContain(targetFq as FullyQualifiedMoniker);
    expect(
      document.querySelector('[data-testid="jump-to-overlay"]'),
      "overlay must unmount after the unique-code match",
    ).toBeNull();

    cleanupRects();
    unmount();
  });

  // -------------------------------------------------------------------------
  // Case 7 — Global keybindings suppressed while overlay is open.
  //
  // The user-observable invariant: while the overlay is up, pressing a
  // globally-bound key that is not part of the sneak alphabet must not
  // move focus to a regular scope. The overlay's claim has placed focus
  // on the sentinel inside its `<FocusLayer name="jump-to">`; the
  // sentinel is the only scope in that layer, so any `nav.*` invocation
  // routes a `spatial_navigate(sentinel, …)` request through the
  // kernel that finds no in-layer peer (and the sentinel sits at the
  // layer root with no parent zone, so escalation is also blocked).
  // The result: no `focus-changed` event for a regular scope, and the
  // overlay remains mounted.
  //
  // We pick `ArrowUp` (cua's `nav.up` binding). It is not in the sneak
  // alphabet (so the overlay's letter-only filter ignores it and lets
  // it bubble), and it is bound globally to `nav.up`. The assertion is
  // that no `spatial_focus` IPC lands on a non-sentinel target after
  // the keystroke — exactly the "global handler is rendered inert"
  // contract the task names.
  // -------------------------------------------------------------------------

  it("global nav keybindings are suppressed while the overlay is open (cua ArrowUp)", async () => {
    currentKeymapMode = "cua";
    const { unmount } = renderApp();
    await flushAppMount();

    const priorFq = await seedCardFocus(harness, "task:T1");

    // Open the overlay via cua Mod+G. Browser-mode tests run on macOS
    // in CI; `normalizeKeyEvent` reads `metaKey` as the Mod modifier
    // on mac, so setting `metaKey` alone produces `Mod+g`.
    fireEvent.keyDown(document.body, { key: "g", metaKey: true });
    const overlay = await waitForJumpOverlay();
    expect(overlay).not.toBeNull();

    // Capture the sentinel FQM (the focus the overlay claimed on
    // mount) so the assertion can distinguish "kernel routed to
    // sentinel" (allowed) from "kernel routed to a regular scope"
    // (forbidden).
    const sentinelFq = composeFq(
      composeFq(fqRoot(asSegment("window")), asSegment("jump-to")),
      asSegment("jump-to-sentinel"),
    );

    mockInvoke.mockClear();

    // ArrowUp is `nav.up` in cua mode. It is not in the sneak alphabet
    // (`a..b` minus `i,l,o`), so the overlay's keydown handler ignores
    // it and the event bubbles to the global keymap handler on
    // `document`. The global handler will dispatch `spatial_navigate`,
    // but with focus pinned on the sentinel inside the single-scope
    // jump-to layer the kernel can't move focus anywhere.
    await act(async () => {
      fireEvent.keyDown(overlay, { key: "ArrowUp" });
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30));
    });

    // No `spatial_focus` lands on a regular scope. Restore-prior or
    // sentinel re-claims would target the sentinel or `priorFq`, both
    // of which are part of the overlay's own bookkeeping — only a
    // landing on a non-overlay scope would constitute a leaked nav.
    const focusCalls = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_focus")
      .map((c) => (c[1] as { fq: FullyQualifiedMoniker }).fq);
    for (const fq of focusCalls) {
      expect(
        fq === sentinelFq || fq === priorFq,
        `global ArrowUp must not move focus to a regular scope (got ${fq})`,
      ).toBe(true);
    }

    // The overlay is still mounted — confirms the global handler did
    // not produce an effect that tore down the overlay.
    expect(
      document.querySelector('[data-testid="jump-to-overlay"]'),
      "overlay must remain mounted while a globally-bound non-alphabet key is pressed",
    ).not.toBeNull();

    unmount();
  });
});
