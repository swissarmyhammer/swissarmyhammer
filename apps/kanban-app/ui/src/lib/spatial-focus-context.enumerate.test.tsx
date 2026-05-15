/**
 * Tests for `SpatialFocusActions.enumerateScopesInLayer` and
 * `SpatialFocusActions.layerFqOf` — the two read-only enumerators the
 * Jump-To overlay uses to discover every focusable scope visible in a
 * specific focus layer at the moment the overlay opens.
 *
 * These methods read from the existing `layerRegistriesRef` populated
 * by `actions.registerLayerRegistry` (driven by `<FocusLayer>` mount
 * effects). They do NOT extend the per-FQM claim map; they are
 * snapshot-style reads of the already-authoritative
 * `LayerScopeRegistry` instances.
 *
 * Properties pinned here:
 *
 * - `enumerateScopesInLayer(layerFq)` returns every scope whose
 *   `LayerScopeRegistry` contains it, with rects sampled live from
 *   `ref.current.getBoundingClientRect()`.
 * - Cross-layer isolation: a window-layer enumeration does not return
 *   modal-layer scopes and vice versa.
 * - Unknown layer FQM → `[]`.
 * - Entries whose `ref.current` is `null` are skipped (matches
 *   `LayerScopeRegistry.buildSnapshot`).
 * - Zero-rect entries (host present but `display: none` or detached)
 *   ARE included — the Jump-To overlay is responsible for filtering
 *   zero-area rects when laying out pills, mirroring the contract of
 *   `LayerScopeRegistry.buildSnapshot`.
 * - `layerFqOf(fq)` returns the layer FQM whose registry contains
 *   `fq`, or `null` when no registry has it.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { RefObject } from "react";

/* -------------------------------------------------------------------------- */
/* Tauri mocks                                                                */
/* -------------------------------------------------------------------------- */

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
);
let listenHandlers: Record<string, (event: { payload: unknown }) => void> = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (event: string, handler: (e: { payload: unknown }) => void) => {
      listenHandlers[event] = handler;
      return Promise.resolve(() => {
        delete listenHandlers[event];
      });
    },
  ),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import {
  LayerScopeRegistry,
  useOptionalLayerScopeRegistry,
  type ScopeEntry,
} from "./layer-scope-registry-context";
import {
  SpatialFocusProvider,
  useSpatialFocusActions,
  type SpatialFocusActions,
} from "./spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  asSegment,
  composeFq,
  fqRoot,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

/* -------------------------------------------------------------------------- */
/* Helpers                                                                    */
/* -------------------------------------------------------------------------- */

/**
 * Wait one microtask for the provider's `listen()` promise to resolve
 * and for `<FocusLayer>` mount effects to publish their registries.
 */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

interface RectLiteral {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Build a `ScopeEntry` whose `ref.current` points at a real DOM node
 * with a stubbed `getBoundingClientRect()` that returns the supplied
 * rect literal. Mirrors the helper used by `layer-scope-registry.test`
 * and `spatial-focus-lost.test` so the enumerate path is exercised
 * with the same DOM shape the rest of the spatial-nav tests use.
 *
 * Returns the entry plus the underlying node so callers can mutate the
 * stubbed rect (or null the ref) without rebuilding the entry.
 */
function makeEntry(
  parentZone: FullyQualifiedMoniker | null,
  rect: RectLiteral,
): { entry: ScopeEntry; node: HTMLDivElement } {
  const node = document.createElement("div");
  node.getBoundingClientRect = () =>
    ({
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
      top: rect.y,
      left: rect.x,
      right: rect.x + rect.width,
      bottom: rect.y + rect.height,
      toJSON: () => rect,
    }) as DOMRect;
  const ref: RefObject<HTMLElement | null> = { current: node };
  return {
    entry: {
      ref,
      parentZone,
      segment: asSegment("scope"),
      lastKnownRect: null,
    },
    node,
  };
}

/** Capture the registry the enclosing layer publishes. */
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
  out: { current: SpatialFocusActions | null };
}) {
  out.current = useSpatialFocusActions();
  return null;
}

beforeEach(() => {
  mockInvoke.mockClear();
  listenHandlers = {};
});

/* -------------------------------------------------------------------------- */
/* Tests                                                                      */
/* -------------------------------------------------------------------------- */

describe("SpatialFocusActions.enumerateScopesInLayer", () => {
  it("returns every scope in the layer's registry with live rects", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));

    const registry = captured.current!;
    const { entry: aEntry } = makeEntry(layerFq, {
      x: 10,
      y: 20,
      width: 100,
      height: 30,
    });
    const { entry: bEntry } = makeEntry(layerFq, {
      x: 200,
      y: 50,
      width: 80,
      height: 24,
    });
    registry.add(aFq, aEntry);
    registry.add(bFq, bEntry);

    const result = actionsOut.current!.enumerateScopesInLayer(layerFq);

    expect(result).toHaveLength(2);
    const byFq = new Map(result.map((r) => [r.fq, r.rect]));
    expect(byFq.get(aFq)).toMatchObject({ x: 10, y: 20, width: 100, height: 30 });
    expect(byFq.get(bFq)).toMatchObject({ x: 200, y: 50, width: 80, height: 24 });
  });

  it("isolates layers — window-layer enumeration excludes modal-layer scopes", async () => {
    const windowOut: { current: LayerScopeRegistry | null } = { current: null };
    const modalOut: { current: LayerScopeRegistry | null } = { current: null };
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={windowOut} />
          <FocusLayer name={asSegment("modal")}>
            <CaptureRegistry out={modalOut} />
          </FocusLayer>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const windowLayerFq = fqRoot(asSegment("window"));
    const modalLayerFq = composeFq(windowLayerFq, asSegment("modal"));

    const windowScopeFq = composeFq(windowLayerFq, asSegment("win-scope"));
    const modalScopeFq = composeFq(modalLayerFq, asSegment("modal-scope"));

    const winReg = windowOut.current!;
    const modReg = modalOut.current!;
    expect(winReg).not.toBe(modReg);

    const { entry: winEntry } = makeEntry(windowLayerFq, {
      x: 1,
      y: 2,
      width: 3,
      height: 4,
    });
    const { entry: modEntry } = makeEntry(modalLayerFq, {
      x: 100,
      y: 200,
      width: 50,
      height: 60,
    });
    winReg.add(windowScopeFq, winEntry);
    modReg.add(modalScopeFq, modEntry);

    const winResult = actionsOut.current!.enumerateScopesInLayer(windowLayerFq);
    expect(winResult.map((r) => r.fq)).toEqual([windowScopeFq]);

    const modResult = actionsOut.current!.enumerateScopesInLayer(modalLayerFq);
    expect(modResult.map((r) => r.fq)).toEqual([modalScopeFq]);
  });

  it("returns [] for an unknown layer FQM", async () => {
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>{null}</FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const ghostLayer = fqRoot(asSegment("ghost"));
    expect(actionsOut.current!.enumerateScopesInLayer(ghostLayer)).toEqual([]);
  });

  it("skips entries whose ref.current is null", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const liveFq = composeFq(layerFq, asSegment("live"));
    const goneFq = composeFq(layerFq, asSegment("gone"));

    const registry = captured.current!;
    const { entry: liveEntry } = makeEntry(layerFq, {
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
    const { entry: goneEntry } = makeEntry(layerFq, {
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
    registry.add(liveFq, liveEntry);
    registry.add(goneFq, goneEntry);

    // Simulate the brief unmount window where React has already nulled
    // the bound `ref` callback but the registry deletion hasn't run yet.
    (goneEntry.ref as { current: HTMLElement | null }).current = null;

    const result = actionsOut.current!.enumerateScopesInLayer(layerFq);
    expect(result.map((r) => r.fq)).toEqual([liveFq]);
  });

  it("includes zero-rect entries (host present but no layout)", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const fq = composeFq(layerFq, asSegment("hidden"));

    const registry = captured.current!;
    // `display: none` host: ref.current is non-null but
    // getBoundingClientRect() reports all zeros. The Jump-To overlay
    // (next task) is responsible for filtering zero-area rects; the
    // enumerator itself mirrors `buildSnapshot`'s contract and includes
    // the entry.
    const { entry } = makeEntry(layerFq, { x: 0, y: 0, width: 0, height: 0 });
    registry.add(fq, entry);

    const result = actionsOut.current!.enumerateScopesInLayer(layerFq);
    expect(result).toHaveLength(1);
    expect(result[0].fq).toBe(fq);
    expect(result[0].rect).toMatchObject({
      x: 0,
      y: 0,
      width: 0,
      height: 0,
    });
  });

  it("samples rects fresh on every call (no cache)", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const fq = composeFq(layerFq, asSegment("a"));

    const registry = captured.current!;
    let liveRect: RectLiteral = { x: 1, y: 2, width: 3, height: 4 };
    const node = document.createElement("div");
    node.getBoundingClientRect = () =>
      ({
        x: liveRect.x,
        y: liveRect.y,
        width: liveRect.width,
        height: liveRect.height,
        top: liveRect.y,
        left: liveRect.x,
        right: liveRect.x + liveRect.width,
        bottom: liveRect.y + liveRect.height,
        toJSON: () => liveRect,
      }) as DOMRect;
    const ref: RefObject<HTMLElement | null> = { current: node };
    registry.add(fq, {
      ref,
      parentZone: layerFq,
      segment: asSegment("scope"),
      lastKnownRect: null,
    });

    const before = actionsOut.current!.enumerateScopesInLayer(layerFq);
    expect(before[0].rect).toMatchObject({ x: 1, y: 2, width: 3, height: 4 });

    liveRect = { x: 50, y: 60, width: 70, height: 80 };
    const after = actionsOut.current!.enumerateScopesInLayer(layerFq);
    expect(after[0].rect).toMatchObject({
      x: 50,
      y: 60,
      width: 70,
      height: 80,
    });
  });
});

describe("SpatialFocusActions.layerFqOf", () => {
  it("returns the layer FQM whose registry contains the scope", async () => {
    const windowOut: { current: LayerScopeRegistry | null } = { current: null };
    const modalOut: { current: LayerScopeRegistry | null } = { current: null };
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={windowOut} />
          <FocusLayer name={asSegment("modal")}>
            <CaptureRegistry out={modalOut} />
          </FocusLayer>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const windowLayerFq = fqRoot(asSegment("window"));
    const modalLayerFq = composeFq(windowLayerFq, asSegment("modal"));
    const winScopeFq = composeFq(windowLayerFq, asSegment("win-scope"));
    const modScopeFq = composeFq(modalLayerFq, asSegment("mod-scope"));

    const { entry: winEntry } = makeEntry(windowLayerFq, {
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
    const { entry: modEntry } = makeEntry(modalLayerFq, {
      x: 0,
      y: 0,
      width: 10,
      height: 10,
    });
    windowOut.current!.add(winScopeFq, winEntry);
    modalOut.current!.add(modScopeFq, modEntry);

    expect(actionsOut.current!.layerFqOf(winScopeFq)).toBe(windowLayerFq);
    expect(actionsOut.current!.layerFqOf(modScopeFq)).toBe(modalLayerFq);
  });

  it("returns null for an unregistered FQM", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const actionsOut: { current: SpatialFocusActions | null } = {
      current: null,
    };

    render(
      <SpatialFocusProvider>
        <CaptureActions out={actionsOut} />
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const ghost = composeFq(layerFq, asSegment("ghost"));
    expect(actionsOut.current!.layerFqOf(ghost)).toBeNull();
  });
});
