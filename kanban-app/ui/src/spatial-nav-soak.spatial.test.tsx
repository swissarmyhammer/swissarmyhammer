/**
 * IPC-shape soak suite for spatial-nav dual-source dev builds.
 *
 * Pins the React-side contract that every focus-mutating spatial-nav
 * IPC carries a populated `NavSnapshot` so the kernel's debug-build
 * `compare_paths` divergence diagnostic always has a snapshot path to
 * compare against the registry path. The Rust soak fixtures in
 * `swissarmyhammer-focus/tests/spatial_nav_soak.rs` cover the kernel
 * side; this file covers the matching frontend invariant.
 *
 * Coverage (every production gesture family the parent card lists):
 *
 *   - Arrow nav across all four directions from kanban-style positions.
 *   - Click focus on every scope kind (chip, field, button, card,
 *     column header).
 *   - Layer push (open inspector) → focus inside → close → restore.
 *   - Modal dialog push → focus inside → cancel → restore.
 *   - Filter hides the focused row → `spatial_focus_lost` with the
 *     populated snapshot, lost_parent_zone, lost_layer_fq, lost_rect.
 *   - Bulk delete that takes the focused card → `spatial_focus_lost`.
 *
 * Scenarios that vitest cannot faithfully exercise (real OS drag-drop,
 * real Tauri-driven modal lifecycle through the production AppShell)
 * are kept here as `it.skip` placeholders that name the gap; those
 * land in the manual ≥1 hour soak in `pnpm tauri dev`.
 *
 * Runs under `kanban-app/ui/vite.config.ts`'s browser project (real
 * Chromium via Playwright) — every `*.spatial.test.tsx` lands there.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { type ReactNode, type RefObject } from "react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
);
let listenHandlers: Record<string, ListenCallback[]> = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn((event: string, cb: ListenCallback): Promise<() => void> => {
    const cbs = listenHandlers[event] ?? [];
    cbs.push(cb);
    listenHandlers[event] = cbs;
    return Promise.resolve(() => {
      const arr = listenHandlers[event];
      if (arr) {
        const idx = arr.indexOf(cb);
        if (idx >= 0) arr.splice(idx, 1);
      }
    });
  }),
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

import {
  SpatialFocusProvider,
  useSpatialFocusActions,
} from "@/lib/spatial-focus-context";
import {
  LayerScopeRegistry,
  useOptionalLayerScopeRegistry,
  type ScopeEntry,
} from "@/lib/layer-scope-registry-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  asPixels,
  asSegment,
  composeFq,
  fqRoot,
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type NavSnapshot,
  type Rect,
  type SegmentMoniker,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

function rect(x: number, y: number, w: number, h: number): Rect {
  return {
    x: asPixels(x),
    y: asPixels(y),
    width: asPixels(w),
    height: asPixels(h),
  };
}

/** Build a `ScopeEntry` whose `ref.current` is a real DOM node with a
 *  stubbed `getBoundingClientRect` so `buildSnapshot` produces a non-zero
 *  rect on read. The cached `lastKnownRect` mirrors the same value so the
 *  delete-listener path also has live geometry. */
function makeEntry(
  parentZone: FullyQualifiedMoniker | null,
  segment: SegmentMoniker,
  r: Rect,
): { entry: ScopeEntry; node: HTMLDivElement } {
  const node = document.createElement("div");
  node.getBoundingClientRect = () =>
    ({
      x: r.x,
      y: r.y,
      width: r.width,
      height: r.height,
      top: r.y,
      left: r.x,
      right: r.x + r.width,
      bottom: r.y + r.height,
      toJSON: () => ({}),
    }) as DOMRect;
  const ref: RefObject<HTMLElement | null> = { current: node };
  return {
    entry: {
      ref,
      parentZone,
      segment,
      lastKnownRect: r,
    },
    node,
  };
}

/** Capture the registry the layer publishes from inside the layer. */
function CaptureRegistry({
  out,
}: {
  out: { current: LayerScopeRegistry | null };
}) {
  out.current = useOptionalLayerScopeRegistry();
  return null;
}

/** Capture the spatial focus actions bag from inside the provider. */
function CaptureActions({
  out,
}: {
  out: { current: ReturnType<typeof useSpatialFocusActions> | null };
}) {
  out.current = useSpatialFocusActions();
  return null;
}

interface Harness {
  layerFq: FullyQualifiedMoniker;
  registry: LayerScopeRegistry;
  actions: NonNullable<ReturnType<typeof useSpatialFocusActions>>;
}

/** Render a `<SpatialFocusProvider>` + `<FocusLayer>` pair and capture
 *  the layer's registry + the provider's actions bag. */
async function setupHarness(layerName = "window"): Promise<Harness> {
  const registryRef: { current: LayerScopeRegistry | null } = { current: null };
  const actionsRef: {
    current: ReturnType<typeof useSpatialFocusActions> | null;
  } = { current: null };

  function Tree({ children }: { children?: ReactNode }) {
    return (
      <SpatialFocusProvider>
        <CaptureActions out={actionsRef} />
        <FocusLayer name={asSegment(layerName)}>
          <CaptureRegistry out={registryRef} />
          {children}
        </FocusLayer>
      </SpatialFocusProvider>
    );
  }

  render(<Tree />);
  await flushSetup();
  await flushSetup();

  if (!registryRef.current) {
    throw new Error("layer registry not captured");
  }
  if (!actionsRef.current) {
    throw new Error("spatial focus actions not captured");
  }

  return {
    layerFq: fqRoot(asSegment(layerName)),
    registry: registryRef.current,
    actions: actionsRef.current,
  };
}

/** Set the kernel-side focus from the React harness's perspective by
 *  driving a `focus-changed` event into the provider. The provider
 *  mirrors `next_fq` into its internal `focusedFqRef`, which the
 *  delete-listener path reads to decide whether to fire `focus_lost`. */
function setFocused(fq: FullyQualifiedMoniker, segment: SegmentMoniker) {
  const payload: FocusChangedPayload = {
    window_label: "main" as FocusChangedPayload["window_label"],
    prev_fq: null,
    next_fq: fq,
    next_segment: segment,
  };
  act(() => {
    const handlers = listenHandlers["focus-changed"] ?? [];
    for (const h of handlers) h({ payload });
  });
}

/** Pull the most recent invoke call for `cmd` and return its args. */
function lastCall(cmd: string): Record<string, unknown> | null {
  for (let i = mockInvoke.mock.calls.length - 1; i >= 0; i--) {
    const [c, args] = mockInvoke.mock.calls[i];
    if (c === cmd) return (args ?? {}) as Record<string, unknown>;
  }
  return null;
}

/** Pull every invoke call for `cmd` and return the args list in order. */
function allCalls(cmd: string): Record<string, unknown>[] {
  return mockInvoke.mock.calls
    .filter(([c]) => c === cmd)
    .map(([, args]) => (args ?? {}) as Record<string, unknown>);
}

/** Assert the snapshot field on an IPC arg bag is a populated NavSnapshot. */
function expectPopulatedSnapshot(
  args: Record<string, unknown> | null,
  expectedLayer: FullyQualifiedMoniker,
): NavSnapshot {
  expect(args, "ipc args must exist").not.toBeNull();
  const snapshot = (args as Record<string, unknown>).snapshot as
    | NavSnapshot
    | undefined;
  expect(snapshot, "snapshot field must be defined on the IPC").toBeDefined();
  expect(snapshot!.layer_fq).toBe(expectedLayer);
  expect(
    Array.isArray(snapshot!.scopes) && snapshot!.scopes.length > 0,
    "snapshot.scopes must be a non-empty array",
  ).toBe(true);
  return snapshot!;
}

beforeEach(() => {
  mockInvoke.mockClear();
  listenHandlers = {};
});

// ---------------------------------------------------------------------------
// Scenario 1: arrow nav across all four directions.
// ---------------------------------------------------------------------------

describe("spatial-nav soak: arrow nav from every column position", () => {
  it("every Direction-call to navigate() carries a populated snapshot", async () => {
    const { layerFq, registry, actions } = await setupHarness();

    // 3-column × 3-card kanban-shaped registry. Columns are zones at
    // the layer root; cards are leaves under each column zone.
    const columns: FullyQualifiedMoniker[] = [];
    const cards: { fq: FullyQualifiedMoniker; segment: SegmentMoniker }[] = [];
    for (const colIdx of [0, 1, 2]) {
      const colSeg = asSegment(`col:${colIdx}`);
      const colFq = composeFq(layerFq, colSeg);
      registry.add(
        colFq,
        makeEntry(null, colSeg, rect(colIdx * 200, 0, 180, 600)).entry,
      );
      columns.push(colFq);
      for (const rowIdx of [0, 1, 2]) {
        const cardSeg = asSegment(`card:${colIdx}-${rowIdx}`);
        const cardFq = composeFq(colFq, cardSeg);
        registry.add(
          cardFq,
          makeEntry(colFq, cardSeg, rect(colIdx * 200 + 10, rowIdx * 100 + 40, 160, 80))
            .entry,
        );
        cards.push({ fq: cardFq, segment: cardSeg });
      }
    }

    mockInvoke.mockClear();

    for (const card of cards) {
      for (const direction of ["up", "down", "left", "right"] as const) {
        await actions.navigate(card.fq, direction);
      }
    }

    const navCalls = allCalls("spatial_navigate");
    expect(navCalls).toHaveLength(cards.length * 4);
    for (const args of navCalls) {
      expectPopulatedSnapshot(args, layerFq);
    }
  });
});

// ---------------------------------------------------------------------------
// Scenario 2: click focus on every scope kind.
// ---------------------------------------------------------------------------

describe("spatial-nav soak: click focus on every scope kind", () => {
  it("focus() on chip / field / button / card / column header carries a populated snapshot", async () => {
    const { layerFq, registry, actions } = await setupHarness();

    // Column header at the top of a column zone.
    const colSeg = asSegment("col:1");
    const colFq = composeFq(layerFq, colSeg);
    registry.add(
      colFq,
      makeEntry(null, colSeg, rect(0, 0, 200, 600)).entry,
    );
    const headerSeg = asSegment("header");
    const headerFq = composeFq(colFq, headerSeg);
    registry.add(
      headerFq,
      makeEntry(colFq, headerSeg, rect(0, 0, 200, 30)).entry,
    );

    // Card and its inner scopes (chip, field, button) under the column.
    const cardSeg = asSegment("card:01");
    const cardFq = composeFq(colFq, cardSeg);
    registry.add(
      cardFq,
      makeEntry(colFq, cardSeg, rect(10, 40, 180, 100)).entry,
    );
    const chipSeg = asSegment("tag:bug");
    const chipFq = composeFq(cardFq, chipSeg);
    registry.add(
      chipFq,
      makeEntry(cardFq, chipSeg, rect(20, 50, 30, 16)).entry,
    );
    const fieldSeg = asSegment("field:title");
    const fieldFq = composeFq(cardFq, fieldSeg);
    registry.add(
      fieldFq,
      makeEntry(cardFq, fieldSeg, rect(20, 70, 150, 20)).entry,
    );
    const buttonSeg = asSegment("btn:open");
    const buttonFq = composeFq(cardFq, buttonSeg);
    registry.add(
      buttonFq,
      makeEntry(cardFq, buttonSeg, rect(20, 100, 60, 20)).entry,
    );

    mockInvoke.mockClear();

    for (const fq of [headerFq, cardFq, chipFq, fieldFq, buttonFq]) {
      await actions.focus(fq);
    }

    const focusCalls = allCalls("spatial_focus");
    expect(focusCalls).toHaveLength(5);
    for (const args of focusCalls) {
      expect(args.fq).toBeDefined();
      expectPopulatedSnapshot(args, layerFq);
    }
  });
});

// ---------------------------------------------------------------------------
// Scenario 3: layer push (open inspector) → focus inside → close → restore.
// ---------------------------------------------------------------------------

describe("spatial-nav soak: inspector layer round-trip", () => {
  it("popLayer() → next_fq → spatial_focus carries a populated snapshot", async () => {
    // Mount a tree with a window layer + a nested inspector layer so
    // both registries are wired into the SpatialFocusProvider's
    // `layerRegistriesRef`. `buildSnapshotForFocused` walks every
    // registered layer to find the one containing the next_fq.
    const windowRegistryRef: { current: LayerScopeRegistry | null } = {
      current: null,
    };
    const inspectorRegistryRef: { current: LayerScopeRegistry | null } = {
      current: null,
    };
    const actionsRef: {
      current: ReturnType<typeof useSpatialFocusActions> | null;
    } = { current: null };

    function Tree() {
      return (
        <SpatialFocusProvider>
          <CaptureActions out={actionsRef} />
          <FocusLayer name={asSegment("window")}>
            <CaptureRegistry out={windowRegistryRef} />
            <FocusLayer name={asSegment("inspector")}>
              <CaptureRegistry out={inspectorRegistryRef} />
            </FocusLayer>
          </FocusLayer>
        </SpatialFocusProvider>
      );
    }

    render(<Tree />);
    await flushSetup();
    await flushSetup();

    const windowLayerFq = fqRoot(asSegment("window"));
    const inspectorLayerFq = composeFq(
      windowLayerFq,
      asSegment("inspector"),
    );
    const windowRegistry = windowRegistryRef.current!;
    const inspectorRegistry = inspectorRegistryRef.current!;
    const actions = actionsRef.current!;

    // Window-layer scope that focus restores to.
    const cardSeg = asSegment("card:01");
    const cardFq = composeFq(windowLayerFq, cardSeg);
    windowRegistry.add(
      cardFq,
      makeEntry(null, cardSeg, rect(0, 0, 200, 100)).entry,
    );

    // Inspector-layer scope inside the panel.
    const inspSeg = asSegment("field:title");
    const insppanelFq = composeFq(inspectorLayerFq, inspSeg);
    inspectorRegistry.add(
      insppanelFq,
      makeEntry(null, inspSeg, rect(0, 0, 300, 30)).entry,
    );

    // The kernel resolves `popLayer` to the next FQM to focus — the
    // window-layer card — and the React adapter follows that with a
    // `spatial_focus` carrying a snapshot built from the layer that
    // contains the next_fq.
    mockInvoke.mockImplementation(async (...args: unknown[]) => {
      if (args[0] === "spatial_pop_layer") return cardFq;
      return undefined;
    });

    await actions.popLayer(inspectorLayerFq);

    // The pop_layer call itself does not carry a snapshot, by design.
    const popArgs = lastCall("spatial_pop_layer");
    expect(popArgs).not.toBeNull();
    expect(popArgs!.fq).toBe(inspectorLayerFq);

    // The follow-up spatial_focus is the gate the soak suite cares
    // about — it MUST carry a populated snapshot for the layer that
    // contains the next_fq.
    const focusArgs = lastCall("spatial_focus");
    expect(focusArgs).not.toBeNull();
    expect(focusArgs!.fq).toBe(cardFq);
    expectPopulatedSnapshot(focusArgs, windowLayerFq);
  });
});

// ---------------------------------------------------------------------------
// Scenario 4: modal dialog push → focus inside → cancel → restore.
// ---------------------------------------------------------------------------

describe("spatial-nav soak: modal dialog round-trip", () => {
  it("popLayer() on dismiss → spatial_focus carries a populated snapshot for the parent layer", async () => {
    const windowRegistryRef: { current: LayerScopeRegistry | null } = {
      current: null,
    };
    const modalRegistryRef: { current: LayerScopeRegistry | null } = {
      current: null,
    };
    const actionsRef: {
      current: ReturnType<typeof useSpatialFocusActions> | null;
    } = { current: null };

    function Tree() {
      return (
        <SpatialFocusProvider>
          <CaptureActions out={actionsRef} />
          <FocusLayer name={asSegment("window")}>
            <CaptureRegistry out={windowRegistryRef} />
            <FocusLayer name={asSegment("modal")}>
              <CaptureRegistry out={modalRegistryRef} />
            </FocusLayer>
          </FocusLayer>
        </SpatialFocusProvider>
      );
    }

    render(<Tree />);
    await flushSetup();
    await flushSetup();

    const windowLayerFq = fqRoot(asSegment("window"));
    const modalLayerFq = composeFq(windowLayerFq, asSegment("modal"));
    const windowRegistry = windowRegistryRef.current!;
    const modalRegistry = modalRegistryRef.current!;
    const actions = actionsRef.current!;

    // Trigger button on the window layer (focus restores to it).
    const triggerSeg = asSegment("btn:open-modal");
    const triggerFq = composeFq(windowLayerFq, triggerSeg);
    windowRegistry.add(
      triggerFq,
      makeEntry(null, triggerSeg, rect(0, 0, 100, 30)).entry,
    );

    // Confirm + Cancel buttons inside the modal.
    const confirmSeg = asSegment("btn:confirm");
    const confirmFq = composeFq(modalLayerFq, confirmSeg);
    modalRegistry.add(
      confirmFq,
      makeEntry(null, confirmSeg, rect(40, 100, 80, 30)).entry,
    );
    const cancelSeg = asSegment("btn:cancel");
    const cancelFq = composeFq(modalLayerFq, cancelSeg);
    modalRegistry.add(
      cancelFq,
      makeEntry(null, cancelSeg, rect(140, 100, 80, 30)).entry,
    );

    // The user navigates between confirm and cancel inside the modal;
    // each navigate must carry a populated snapshot.
    mockInvoke.mockClear();
    await actions.navigate(confirmFq, "right");
    await actions.navigate(cancelFq, "left");

    const navCalls = allCalls("spatial_navigate");
    expect(navCalls).toHaveLength(2);
    for (const args of navCalls) {
      expectPopulatedSnapshot(args, modalLayerFq);
    }

    // Cancel dismisses the modal — the kernel returns the trigger
    // button as the next focused FQM and the React adapter dispatches
    // a follow-up spatial_focus carrying the window-layer snapshot.
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(async (...args: unknown[]) => {
      if (args[0] === "spatial_pop_layer") return triggerFq;
      return undefined;
    });

    await actions.popLayer(modalLayerFq);

    const focusArgs = lastCall("spatial_focus");
    expect(focusArgs).not.toBeNull();
    expect(focusArgs!.fq).toBe(triggerFq);
    expectPopulatedSnapshot(focusArgs, windowLayerFq);
  });
});

// ---------------------------------------------------------------------------
// Scenario 5: filter changes that hide the focused row → focus_lost fires.
// ---------------------------------------------------------------------------

describe("spatial-nav soak: filter hides focused row", () => {
  it("registry.delete on the focused FQM dispatches spatial_focus_lost with a populated snapshot", async () => {
    const { layerFq, registry } = await setupHarness();

    // Focused card and a peer that survives the filter change.
    const cardSeg = asSegment("card:hidden");
    const cardFq = composeFq(layerFq, cardSeg);
    const cardRect = rect(10, 40, 160, 80);
    const { entry: cardEntry } = makeEntry(layerFq, cardSeg, cardRect);
    registry.add(cardFq, cardEntry);

    const peerSeg = asSegment("card:visible");
    const peerFq = composeFq(layerFq, peerSeg);
    registry.add(
      peerFq,
      makeEntry(layerFq, peerSeg, rect(10, 140, 160, 80)).entry,
    );

    setFocused(cardFq, cardSeg);

    mockInvoke.mockClear();
    registry.delete(cardFq);

    const args = lastCall("spatial_focus_lost");
    expect(args).not.toBeNull();
    expect(args!.focusedFq).toBe(cardFq);
    expect(args!.lostParentZone).toBe(layerFq);
    expect(args!.lostLayerFq).toBe(layerFq);
    expect(args!.lostRect).toEqual(cardRect);
    const snapshot = expectPopulatedSnapshot(args, layerFq);
    // The lost FQM must NOT appear in the snapshot — the registry
    // deletion runs first, so the snapshot built inside the delete
    // listener excludes it.
    const snapshotFqs = snapshot.scopes.map((s) => s.fq);
    expect(snapshotFqs).not.toContain(cardFq);
    expect(snapshotFqs).toContain(peerFq);
  });
});

// ---------------------------------------------------------------------------
// Scenario 6: bulk delete that removes the focused card.
// ---------------------------------------------------------------------------

describe("spatial-nav soak: bulk delete includes focused card", () => {
  it("deleting multiple scopes including the focused one fires spatial_focus_lost with a populated snapshot", async () => {
    const { layerFq, registry } = await setupHarness();

    // 5-card row; the bulk delete removes 3 of them including the
    // currently-focused one in the middle. Two survive.
    const cards: { fq: FullyQualifiedMoniker; segment: SegmentMoniker }[] = [];
    for (let i = 0; i < 5; i++) {
      const seg = asSegment(`card:${i}`);
      const fq = composeFq(layerFq, seg);
      registry.add(
        fq,
        makeEntry(layerFq, seg, rect(i * 180, 40, 160, 80)).entry,
      );
      cards.push({ fq, segment: seg });
    }

    const focused = cards[2];
    setFocused(focused.fq, focused.segment);

    mockInvoke.mockClear();
    // Bulk delete fires `delete` per FQM; production flushes them in
    // a single React commit but the registry's delete listener runs
    // synchronously per call.
    for (const idx of [1, 2, 3]) {
      registry.delete(cards[idx].fq);
    }

    // Exactly one focus_lost fires — only the deletion of the focused
    // FQM triggers the IPC. The other two deletions are unfocused.
    const lostCalls = allCalls("spatial_focus_lost");
    expect(lostCalls).toHaveLength(1);
    const args = lostCalls[0];
    expect(args.focusedFq).toBe(focused.fq);

    const snapshot = expectPopulatedSnapshot(args, layerFq);
    const snapshotFqs = snapshot.scopes.map((s) => s.fq);
    // The IPC fires from the focused FQM's deletion; at that moment
    // only that one entry has been removed, the two siblings being
    // bulk-deleted are still in the registry. The snapshot reflects
    // the registry state at the moment the listener runs.
    expect(snapshotFqs).not.toContain(focused.fq);
    expect(snapshotFqs).toContain(cards[0].fq);
    expect(snapshotFqs).toContain(cards[4].fq);
  });
});

// ---------------------------------------------------------------------------
// Skipped: scenarios that need the real Tauri runtime / OS drag-drop.
// These land in the manual ≥1 hour soak in `pnpm tauri dev`.
// ---------------------------------------------------------------------------

describe("spatial-nav soak: scenarios reserved for manual dev-mode soak", () => {
  it.skip("real OS drag-drop a card between columns — needs Playwright drag emulation against the real AppShell", () => {
    // The Rust soak suite covers the post-move registry/snapshot parity
    // (`soak_drag_drop_card_between_columns` in
    // `swissarmyhammer-focus/tests/spatial_nav_soak.rs`). Driving an
    // OS-level drag through the production AppShell — which depends on
    // the Tauri runtime, native drag previews, and the
    // `useDragDrop` hook chain — is out of reach for vitest's
    // browser-mode harness; the manual soak in `pnpm tauri dev` is the
    // gate.
  });

  it.skip("modal dismissal via Escape through the production AppShell — needs the real keymap + dialog wiring", () => {
    // The IPC-shape contract is pinned by the
    // "modal dialog round-trip" scenario above. Driving the dismissal
    // through the production keymap (Escape → modal close → focus
    // restore) needs the full AppShell + keymap registry + dialog
    // primitive composition, which is the manual soak's territory.
  });

  it.skip("inspector open/close through the real layer machinery — needs production AppShell + inspector registry", () => {
    // The IPC-shape contract is pinned by the "inspector layer
    // round-trip" scenario above. Driving the inspector through the
    // real `<InspectorsContainer>` + dispatch chain is reserved for the
    // manual soak.
  });

  it.skip("≥1 hour continuous interaction with no divergence warns — manual gate", () => {
    // The acceptance criterion explicitly names the ≥1 hour manual
    // soak in `pnpm tauri dev` watching
    // `just logs | grep \"snapshot/registry divergence\"` as the
    // user-side gate before step 10/cutover.
  });
});
