/**
 * Shared spatial-nav test harness — JS shadow registry plus an in-test
 * port of `BeamNavStrategy::next` from
 * `swissarmyhammer-focus/src/navigate.rs`.
 *
 * # Why this module exists
 *
 * Two test files mount production React + spatial-nav and need to drive
 * gestures end-to-end against the real registry that the production
 * components emit on mount:
 *
 *   - `kanban-app/ui/src/components/board-view.cross-column-nav.spatial.test.tsx`
 *     — exercises cross-column nav on a partial-app `<BoardView>` mount.
 *   - `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` — the
 *     full `<App/>` end-to-end smoke test (card
 *     `01KQ7PXYP62VQ18K9XYS4Y42GA`).
 *
 * Both share the same scaffolding: a `vi.hoisted` `mockInvoke` /
 * `mockListen` / `listeners` triple, the Tauri-API `vi.mock` shims, a
 * shadow registry that captures every `spatial_register_*` call, an
 * `installShadowNavigator` helper that routes `spatial_navigate` /
 * `spatial_focus` through the JS port, and a `fireFocusChanged` helper
 * that mimics the Rust kernel emitting a focus-changed event for the
 * active window.
 *
 * Extracting the scaffolding here gives us **one** place to maintain the
 * port of `BeamNavStrategy` — when the kernel's beam-search rules change,
 * exactly one file in the React test suite needs to follow.
 *
 * # Path-monikers identity model
 *
 * Card `01KQD6064G1C1RAXDFPJVT1F46` collapsed `SpatialKey` and the flat
 * `Moniker` newtypes into a single `FullyQualifiedMoniker`. The shadow
 * registry uses the FQM as its sole key. Every captured registration
 * carries the FQM (`fq`), the relative segment the consumer declared
 * (`segment`), the owning layer's FQM (`layerFq`), and the parent
 * zone's FQM (`parentZone`).
 *
 * # vi.mock is file-scoped — consumers declare their own
 *
 * Vitest's `vi.mock` is hoisted to the top of the file it appears in
 * and applies only to that file's transitive imports. A `vi.mock` call
 * in this helper module would not intercept a test file's
 * `import App from "@/App"` line, because that import goes through the
 * test file's import graph, not through the helper's.
 *
 * Each consuming test file therefore declares its own `vi.mock` calls
 * for the Tauri APIs and forwards to the spies exported here via a
 * `vi.hoisted` factory that dynamically imports this module. The
 * pattern is canonical:
 *
 * ```ts
 * const { mockInvoke, mockListen } = await vi.hoisted(async () => {
 *   const helper = await import("@/test/spatial-shadow-registry");
 *   return { mockInvoke: helper.mockInvoke, mockListen: helper.mockListen };
 * });
 *
 * vi.mock("@tauri-apps/api/core", () => ({
 *   invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
 * }));
 * // ... mocks for event / window / plugin-log
 *
 * import { setupSpatialHarness, type SpatialHarness } from "@/test/spatial-shadow-registry";
 * ```
 *
 * That keeps the helper as the single owner of the spy / listener
 * registry while satisfying vitest's file-scoped mock hoisting.
 *
 * # Hoisted spy triple
 *
 * The `mockInvoke` / `mockListen` / `listeners` triple is created via
 * `vi.hoisted` so it is initialized before any module's body runs.
 * Both consumers (cross-column-nav and end-to-end) share this single
 * triple — they are run in separate Vitest workers, so the stateful
 * `listeners` map and `mockInvoke` spy are isolated per test file.
 */

import { vi } from "vitest";
import { act } from "@testing-library/react";
import {
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
  type WindowLabel,
} from "@/types/spatial";

// Re-export `asFq` directly from `@/types/spatial` so test files can build
// FQM literals without importing from `@/types/spatial`. This pass-through
// re-export form (rather than `export { asFq }` after a named import)
// avoids a Vite browser-mode SyntaxError where the live binding is not
// always visible to downstream re-exporters.
export { asFq } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Listener callback type — wraps the focus-changed payload shape.
// ---------------------------------------------------------------------------

/** Callback signature matching `@tauri-apps/api/event::listen`. */
export type ListenCallback = (event: { payload: unknown }) => void;

// ---------------------------------------------------------------------------
// Hoisted mock triple — created once at module load.
//
// `vi.hoisted` runs its factory before the rest of the module body so
// the resulting triple is available for the `vi.mock` factories below.
// Both consumers (cross-column-nav test and end-to-end test) share this
// single triple — they are run in separate Vitest workers, so the
// stateful `listeners` map and `mockInvoke` spy are isolated per file.
// ---------------------------------------------------------------------------

const hoisted = vi.hoisted(() => {
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

/** The vitest spy installed on `@tauri-apps/api/core::invoke`. */
export const mockInvoke = hoisted.mockInvoke;
/** The vitest spy installed on `@tauri-apps/api/event::listen`. */
export const mockListen = hoisted.mockListen;
/** Map: event name → registered listener callbacks. */
export const listeners = hoisted.listeners;

// ---------------------------------------------------------------------------
// Shadow-registry data shapes
// ---------------------------------------------------------------------------

/**
 * Wire-shape rect captured from a `spatial_register_*` payload.
 *
 * Mirrors the kernel's `Rect` (a `Pixels`-typed `{x, y, width, height}`).
 * The `Pixels` brand is opaque from the JS side — we read it as `number`
 * because the production code rounds-trips through `asPixels(number)` and
 * the wire payload is the raw `{value: number}` envelope or just a number
 * depending on the `Pixels` representation (both work because we read
 * `.value` defensively).
 */
export interface RectLike {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Shadow-registry kind. After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE`
 * collapsed the legacy split primitives into a single `<FocusScope>`,
 * every registered entry is a `"scope"`. The string literal type is
 * preserved (rather than dropped entirely) so existing test fixtures
 * that carry a `kind` field still typecheck — but only the `"scope"`
 * variant is ever produced in practice.
 */
export type ShadowKind = "scope";

/** One entry in the JS shadow registry mirroring the kernel's `RegisteredScope`. */
export interface ShadowEntry {
  kind: ShadowKind;
  fq: FullyQualifiedMoniker;
  segment: SegmentMoniker;
  rect: RectLike;
  layerFq: FullyQualifiedMoniker;
  parentZone: FullyQualifiedMoniker | null;
  overrides: Record<string, unknown>;
}

/** Cardinal direction the JS port handles. */
export type Direction = "up" | "down" | "left" | "right";

// ---------------------------------------------------------------------------
// Wire-payload helpers
// ---------------------------------------------------------------------------

/** Extract a numeric value from either a `Pixels` envelope or a raw number. */
function readPixels(p: unknown): number {
  if (typeof p === "number") return p;
  if (p && typeof p === "object" && "value" in p) {
    const v = (p as { value: unknown }).value;
    return typeof v === "number" ? v : 0;
  }
  return 0;
}

/** Turn a wire-shape rect into a plain `{x, y, width, height}` of numbers. */
export function rectFromWire(r: unknown): RectLike {
  if (!r || typeof r !== "object") return { x: 0, y: 0, width: 0, height: 0 };
  const o = r as Record<string, unknown>;
  return {
    x: readPixels(o.x),
    y: readPixels(o.y),
    width: readPixels(o.width),
    height: readPixels(o.height),
  };
}

// ---------------------------------------------------------------------------
// JS port of `BeamNavStrategy::next` — cardinal directions only.
//
// Mirrors the unified cascade implemented in
// `swissarmyhammer-focus/src/navigate.rs`:
//
//   1. Iter 0 — ANY-KIND beam search among scopes sharing `from.parentZone`
//      (excluding `from` itself), filtered by layer. Both zones and
//      leaves are siblings under the same parent zone, so iter 0
//      considers them peers.
//   2. Escalate to `from.parentZone` (with a layer-boundary guard).
//      If the focused entry has no `parentZone`, return `null`.
//   3. Iter 1 — same-kind beam search among ZONES sharing the parent's
//      `parentZone` (excluding the parent itself). The parent IS a
//      zone, so its peers are zones by construction — this is
//      structural, not a kind policy.
//   4. Drill-out fallback — when neither iter finds a peer, return
//      the parent zone itself.
// ---------------------------------------------------------------------------

/**
 * In-test JS port of the Rust `BeamNavStrategy::next` for cardinal
 * directions, mirroring the unified cascade from
 * `swissarmyhammer-focus/src/navigate.rs`.
 *
 * The cascade has three observable outcomes:
 *
 *   1. **Iter 0** — peer match at the focused scope's level. All
 *      registered scopes sharing a `parent_zone` are siblings.
 *   2. **Iter 1** — peer match at the parent scope's level (after
 *      escalation, with a layer-boundary guard).
 *   3. **Drill-out** — return the parent scope itself when neither
 *      iter finds a peer. Returns `null` only when the focused entry
 *      sits at the very root of its layer with no parent scope.
 *
 * After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy
 * split primitives into a single `<FocusScope>`, the cascade no longer
 * filters on a kind discriminator — every registered entry is a scope,
 * and structural shape (container vs leaf) is determined by whether the
 * scope has child scopes. See `swissarmyhammer-focus/README.md` for
 * the prose contract.
 *
 * Returns the FQM of the next focus target, or `null` when the
 * navigator declines to navigate.
 */
export function navigateInShadow(
  registry: Map<FullyQualifiedMoniker, ShadowEntry>,
  fromFq: FullyQualifiedMoniker,
  direction: Direction,
): { nextFq: FullyQualifiedMoniker; nextSegment: SegmentMoniker } | null {
  const from = registry.get(fromFq);
  if (!from) return null;

  // Iter 0: peers sharing from.parentZone — under the unified primitive
  // every registered entry is a scope, so any sibling under the same
  // parent counts.
  const iter0 = beamAmongInZoneAnyKind(
    registry,
    from.layerFq,
    from.rect,
    from.parentZone,
    from.fq,
    direction,
  );
  if (iter0) return iter0;

  // Escalate. The layer-boundary guard refuses to cross layer FQMs —
  // an inspector layer's panel scope never lifts focus into the window
  // layer that hosts ui:board.
  if (from.parentZone === null) return null;
  const parent = registry.get(from.parentZone);
  if (!parent) return null;
  if (parent.layerFq !== from.layerFq) return null;

  // Iter 1: peers of the parent scope sharing its parent_zone. After
  // the single-primitive collapse there is no kind filter — every
  // registered entry is a scope, so any sibling of the parent is a
  // valid candidate.
  const iter1 = beamAmongSiblings(
    registry,
    parent.layerFq,
    parent.rect,
    parent.parentZone,
    parent.fq,
    direction,
  );
  if (iter1) return iter1;

  // Drill-out fallback: return the parent scope itself.
  return { nextFq: parent.fq, nextSegment: parent.segment };
}

/**
 * Beam-search candidates sharing `fromParent` (excluding `fromFq`),
 * filtered by `layer`. Matches `beam_among_in_zone_any_kind` in the
 * Rust kernel — this is the iter-0 helper.
 *
 * After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy
 * split primitives into a single `<FocusScope>`, every registered
 * entry is a scope; the kernel and this simulator filter only by
 * layer membership and shared `parentZone`.
 */
function beamAmongInZoneAnyKind(
  registry: Map<FullyQualifiedMoniker, ShadowEntry>,
  layer: FullyQualifiedMoniker,
  fromRect: RectLike,
  fromParent: FullyQualifiedMoniker | null,
  fromFq: FullyQualifiedMoniker,
  direction: Direction,
): { nextFq: FullyQualifiedMoniker; nextSegment: SegmentMoniker } | null {
  const candidates: ShadowEntry[] = [];
  for (const e of registry.values()) {
    if (
      e.layerFq === layer &&
      e.parentZone === fromParent &&
      e.fq !== fromFq
    ) {
      candidates.push(e);
    }
  }
  return pickBestRect(fromRect, candidates, direction);
}

/**
 * Beam-search candidates sharing `fromParent` (excluding `fromFq`),
 * filtered by `layer`. Matches `beam_among_siblings` in the Rust
 * kernel — used by iter 1.
 *
 * After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the legacy
 * split primitives into a single `<FocusScope>`, the kind filter was
 * removed: every registered entry is a scope, so any sibling of the
 * parent is a valid candidate.
 */
function beamAmongSiblings(
  registry: Map<FullyQualifiedMoniker, ShadowEntry>,
  layer: FullyQualifiedMoniker,
  fromRect: RectLike,
  fromParent: FullyQualifiedMoniker | null,
  fromFq: FullyQualifiedMoniker,
  direction: Direction,
): { nextFq: FullyQualifiedMoniker; nextSegment: SegmentMoniker } | null {
  const candidates: ShadowEntry[] = [];
  for (const e of registry.values()) {
    if (
      e.layerFq === layer &&
      e.parentZone === fromParent &&
      e.fq !== fromFq
    ) {
      candidates.push(e);
    }
  }
  return pickBestRect(fromRect, candidates, direction);
}

/**
 * Mirror of `pick_best_candidate` in the Rust kernel. The cross-axis
 * beam test is a hard filter: out-of-beam candidates are dropped before
 * scoring runs. Among in-beam candidates the lowest-scored one wins.
 *
 * The hard-filter behavior was tightened from a soft tier preference in
 * the directional-nav supersession card `01KQ7STZN3G5N2WB3FF4PM4DKX` —
 * out-of-beam fallbacks were letting visually disconnected scopes
 * (e.g. a navbar leaf above a card grid) win cardinal-direction nav
 * from cards in the rightmost column. See `pick_best_candidate` in
 * `swissarmyhammer-focus/src/navigate.rs` for the canonical rationale.
 *
 * Takes a `RectLike` rather than a `ShadowEntry` for `from` so the
 * cascade's iter-1 step can pass the parent zone's rect (the parent
 * is identified by FQM, not by the focused entry's `ShadowEntry`).
 */
function pickBestRect(
  fromRect: RectLike,
  candidates: ShadowEntry[],
  direction: Direction,
): { nextFq: FullyQualifiedMoniker; nextSegment: SegmentMoniker } | null {
  let bestEntry: ShadowEntry | null = null;
  let bestScore = Infinity;

  for (const cand of candidates) {
    const scored = scoreCandidate(fromRect, cand.rect, direction);
    if (!scored) continue;
    const [inBeam, score] = scored;
    // Hard in-beam filter — see function docs.
    if (!inBeam) continue;
    if (bestEntry === null || score < bestScore) {
      bestEntry = cand;
      bestScore = score;
    }
  }
  if (!bestEntry) return null;
  return { nextFq: bestEntry.fq, nextSegment: bestEntry.segment };
}

/**
 * JS port of `score_candidate` for cardinal directions. Returns
 * `[inBeam, score]` or `null` when the candidate is on the wrong side
 * of `from` along the major axis. Scoring formula matches the kernel's
 * `13 * major² + minor²` exactly so the test reproduces the same
 * answers.
 */
function scoreCandidate(
  from: RectLike,
  cand: RectLike,
  direction: Direction,
): [boolean, number] | null {
  const fLeft = from.x;
  const fRight = from.x + from.width;
  const fTop = from.y;
  const fBottom = from.y + from.height;
  const fCx = from.x + from.width / 2;
  const fCy = from.y + from.height / 2;
  const cLeft = cand.x;
  const cRight = cand.x + cand.width;
  const cTop = cand.y;
  const cBottom = cand.y + cand.height;
  const cCx = cand.x + cand.width / 2;
  const cCy = cand.y + cand.height / 2;

  let major: number;
  let minor: number;
  let inBeam: boolean;
  switch (direction) {
    case "down": {
      major = cTop - fBottom;
      if (major < 0 && cBottom <= fBottom) return null;
      if (major < 0) major = cCy - fCy;
      minor = Math.abs(fCx - cCx);
      inBeam = fLeft < cRight && cLeft < fRight;
      break;
    }
    case "up": {
      major = fTop - cBottom;
      if (major < 0 && cTop >= fTop) return null;
      if (major < 0) major = fCy - cCy;
      minor = Math.abs(fCx - cCx);
      inBeam = fLeft < cRight && cLeft < fRight;
      break;
    }
    case "right": {
      major = cLeft - fRight;
      if (major < 0 && cRight <= fRight) return null;
      if (major < 0) major = cCx - fCx;
      minor = Math.abs(fCy - cCy);
      inBeam = fTop < cBottom && cTop < fBottom;
      break;
    }
    case "left": {
      major = fLeft - cRight;
      if (major < 0 && cLeft >= fLeft) return null;
      if (major < 0) major = fCx - cCx;
      minor = Math.abs(fCy - cCy);
      inBeam = fTop < cBottom && cTop < fBottom;
      break;
    }
  }
  if (major < 0) major = 0;
  const score = 13 * major * major + minor * minor;
  return [inBeam, score];
}

// ---------------------------------------------------------------------------
// fireFocusChanged — drive a focus-changed event into the React tree.
// ---------------------------------------------------------------------------

/**
 * Drive a `focus-changed` event into the React tree, mimicking the Rust
 * kernel emitting one for the active window.
 *
 * Wraps the dispatch in `act()` from `@testing-library/react` so React
 * state updates flush before the caller asserts on post-update DOM.
 */
export async function fireFocusChanged({
  prev_fq = null,
  next_fq = null,
  next_segment = null,
}: {
  prev_fq?: FullyQualifiedMoniker | null;
  next_fq?: FullyQualifiedMoniker | null;
  next_segment?: SegmentMoniker | null;
}): Promise<void> {
  const payload: FocusChangedPayload = {
    window_label: "main" as WindowLabel,
    prev_fq,
    next_fq,
    next_segment,
  };
  const handlers = listeners.get("focus-changed") ?? [];
  await act(async () => {
    for (const handler of handlers) handler({ payload });
    await Promise.resolve();
  });
}

// ---------------------------------------------------------------------------
// Shadow-navigator installer — wires `mockInvoke` into the registry.
// ---------------------------------------------------------------------------

/** Bundle returned by `installShadowNavigator`. */
export interface ShadowHarness {
  /** The live JS shadow registry — keyed by `FullyQualifiedMoniker`. */
  registry: Map<FullyQualifiedMoniker, ShadowEntry>;
  /** Currently focused FQM (mutated by `spatial_focus` / `spatial_navigate`). */
  currentFocus: { fq: FullyQualifiedMoniker | null };
  /**
   * Look up the registered FQM by trailing segment.
   *
   * Returns `null` when no registration with that segment exists.
   * Useful for translating fixture-defined segment strings into the
   * runtime-composed FQMs after the production components mount.
   *
   * When multiple registrations share a segment (e.g. duplicate
   * `card:T1` mounts in different columns), returns the most recent
   * live entry.
   */
  getRegisteredFqBySegment(segment: string): FullyQualifiedMoniker | null;
}

/**
 * Bootstrap-invoke handler signature. Returns a value to satisfy the
 * Tauri command, or `undefined` to fall through to the default
 * (`undefined`) response. `args` is the raw command args bag.
 */
export type DefaultInvokeImpl = (
  cmd: string,
  args?: unknown,
) => Promise<unknown> | unknown;

/**
 * Install a `mockInvoke` implementation that:
 *   - records every `spatial_register_scope` call into a JS shadow
 *     registry,
 *   - drops entries on `spatial_unregister_scope`,
 *   - refreshes rects on `spatial_update_rect`,
 *   - on `spatial_navigate(focusedFq, direction)` runs the in-test
 *     BeamNavStrategy port against the shadow registry and emits a
 *     `focus-changed` event with the resulting FQM + segment,
 *   - on `spatial_focus` echoes the given FQM back as a `focus-changed`
 *     emit so the React tree picks up the new focus claim.
 *
 * Every other IPC falls through to `defaultInvokeImpl`. Returns a
 * `ShadowHarness` whose `registry` is the live mutable map and whose
 * `getRegisteredFqBySegment` walks the captured registrations to map
 * fixture segments onto runtime-composed FQMs.
 *
 * @param defaultInvokeImpl - Fallback for non-spatial commands. The
 *   end-to-end test uses this to serve `kanban_state_snapshot` and the
 *   board-data / list-entities / get-ui-state bootstrap commands.
 */
export function installShadowNavigator(
  defaultInvokeImpl: DefaultInvokeImpl = async () => undefined,
): ShadowHarness {
  const registry = new Map<FullyQualifiedMoniker, ShadowEntry>();
  const currentFocus: { fq: FullyQualifiedMoniker | null } = { fq: null };

  mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === "spatial_register_scope") {
      const a = (args ?? {}) as Record<string, unknown>;
      const entry: ShadowEntry = {
        kind: "scope",
        fq: a.fq as FullyQualifiedMoniker,
        segment: a.segment as SegmentMoniker,
        rect: rectFromWire(a.rect),
        layerFq: a.layerFq as FullyQualifiedMoniker,
        parentZone: (a.parentZone ?? null) as FullyQualifiedMoniker | null,
        overrides: (a.overrides ?? {}) as Record<string, unknown>,
      };
      registry.set(entry.fq, entry);
      return undefined;
    }
    if (cmd === "spatial_register_batch") {
      // `entries: Vec<RegisterEntry>` — the column-view virtualizer batches
      // off-screen scope placeholders through this command. After the
      // single-primitive collapse there is no `kind` discriminator on
      // batch entries; every entry is a scope.
      const a = (args ?? {}) as Record<string, unknown>;
      const entries = (a.entries ?? []) as Array<Record<string, unknown>>;
      for (const e of entries) {
        const entry: ShadowEntry = {
          kind: "scope",
          fq: e.fq as FullyQualifiedMoniker,
          segment: e.segment as SegmentMoniker,
          rect: rectFromWire(e.rect),
          layerFq: e.layer_fq as FullyQualifiedMoniker,
          parentZone: (e.parent_zone ?? null) as FullyQualifiedMoniker | null,
          overrides: (e.overrides ?? {}) as Record<string, unknown>,
        };
        registry.set(entry.fq, entry);
      }
      return undefined;
    }
    if (cmd === "spatial_unregister_scope") {
      const a = (args ?? {}) as Record<string, unknown>;
      registry.delete(a.fq as FullyQualifiedMoniker);
      return undefined;
    }
    if (cmd === "spatial_update_rect") {
      const a = (args ?? {}) as Record<string, unknown>;
      const e = registry.get(a.fq as FullyQualifiedMoniker);
      if (e) e.rect = rectFromWire(a.rect);
      return undefined;
    }
    if (cmd === "spatial_focus") {
      const a = (args ?? {}) as Record<string, unknown>;
      const nextFq = a.fq as FullyQualifiedMoniker;
      const entry = registry.get(nextFq);
      const prev = currentFocus.fq;
      currentFocus.fq = nextFq;
      // Emit focus-changed asynchronously so the kernel's emit-after-write
      // ordering is preserved. Listeners run synchronously inside `act()`
      // by the caller; here we just queue.
      const payload: FocusChangedPayload = {
        window_label: "main" as WindowLabel,
        prev_fq: prev,
        next_fq: nextFq,
        next_segment: entry?.segment ?? null,
      };
      queueMicrotask(() => {
        const handlers = listeners.get("focus-changed") ?? [];
        for (const h of handlers) h({ payload });
      });
      return undefined;
    }
    if (cmd === "spatial_clear_focus") {
      const prev = currentFocus.fq;
      if (prev === null) return undefined;
      currentFocus.fq = null;
      const payload: FocusChangedPayload = {
        window_label: "main" as WindowLabel,
        prev_fq: prev,
        next_fq: null,
        next_segment: null,
      };
      queueMicrotask(() => {
        const handlers = listeners.get("focus-changed") ?? [];
        for (const h of handlers) h({ payload });
      });
      return undefined;
    }
    if (cmd === "spatial_navigate") {
      const a = (args ?? {}) as Record<string, unknown>;
      const fromFq = a.focusedFq as FullyQualifiedMoniker;
      const direction = a.direction as Direction;
      const result = navigateInShadow(registry, fromFq, direction);
      if (!result) return undefined;
      // The prev_fq carried in the focus-changed payload must be the
      // FQM the kernel is moving AWAY from — which is `fromFq`, the
      // argument the navigator was called with. The SpatialFocusProvider
      // routes this prev_fq through to the focus-claim listener that
      // owns the prior `data-focused` attribute, so without this the
      // outgoing leaf keeps `data-focused="true"` and the next assertion
      // sees both old and new candidates marked focused.
      currentFocus.fq = result.nextFq;
      const payload: FocusChangedPayload = {
        window_label: "main" as WindowLabel,
        prev_fq: fromFq,
        next_fq: result.nextFq,
        next_segment: result.nextSegment,
      };
      queueMicrotask(() => {
        const handlers = listeners.get("focus-changed") ?? [];
        for (const h of handlers) h({ payload });
      });
      return undefined;
    }
    if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
      // Drill-in/out are kernel state changes; the React side dispatches
      // them but the test harness does not need to model the resulting
      // focus move. Echo `focusedFq` back to satisfy the no-silent-dropout
      // contract that the kernel always returns an FQM.
      const a = (args ?? {}) as Record<string, unknown>;
      return (a.focusedFq ?? "") as FullyQualifiedMoniker;
    }
    if (cmd === "spatial_push_layer" || cmd === "spatial_pop_layer") {
      // Layer push/pop are kernel bookkeeping operations — accept and
      // record nothing; tests audit `spatial_push_layer` calls separately
      // via `mockInvoke.mock.calls`.
      return undefined;
    }
    return defaultInvokeImpl(cmd, args);
  });

  return {
    registry,
    currentFocus,
    getRegisteredFqBySegment(segment: string): FullyQualifiedMoniker | null {
      // Walk the live registry first — it is post-unregister-aware.
      let mostRecent: FullyQualifiedMoniker | null = null;
      for (const e of registry.values()) {
        if (e.segment === segment) mostRecent = e.fq;
      }
      if (mostRecent) return mostRecent;
      // Fall back to the captured invoke calls — covers the case where a
      // scope was registered then unregistered (e.g. virtualized cards
      // scrolled out) and the test wants to find the most recent FQM for
      // a segment that isn't currently mounted.
      for (let i = mockInvoke.mock.calls.length - 1; i >= 0; i--) {
        const [cmd, args] = mockInvoke.mock.calls[i];
        if (cmd === "spatial_register_scope") {
          const a = (args ?? {}) as Record<string, unknown>;
          if (a.segment === segment) return a.fq as FullyQualifiedMoniker;
        } else if (cmd === "spatial_register_batch") {
          const a = (args ?? {}) as Record<string, unknown>;
          const entries = (a.entries ?? []) as Array<Record<string, unknown>>;
          for (const e of entries) {
            if (e.segment === segment) return e.fq as FullyQualifiedMoniker;
          }
        }
      }
      return null;
    },
  };
}

// ---------------------------------------------------------------------------
// setupSpatialHarness — convenience entry point for tests.
// ---------------------------------------------------------------------------

/** Result of `setupSpatialHarness` — the bundle the card description names. */
export interface SpatialHarness extends ShadowHarness {
  /** The vitest spy installed on `@tauri-apps/api/core::invoke`. */
  mockInvoke: typeof mockInvoke;
  /** Drive a `focus-changed` event into the React tree. */
  fireFocusChanged: (payload: {
    prev_fq?: FullyQualifiedMoniker | null;
    next_fq?: FullyQualifiedMoniker | null;
    next_segment?: SegmentMoniker | null;
  }) => Promise<void>;
}

/**
 * One-call harness setup for spatial-nav browser tests.
 *
 * Clears the hoisted `mockInvoke` / `mockListen` / `listeners` state,
 * installs the shadow navigator on top of `defaultInvokeImpl`, and
 * returns the bundle the test files consume:
 *
 * ```ts
 * const harness = setupSpatialHarness({ defaultInvokeImpl });
 * harness.mockInvoke           // raw spy
 * harness.fireFocusChanged({ ... })
 * harness.registry             // live JS shadow registry
 * harness.getRegisteredFqBySegment("card:T1") // segment → FullyQualifiedMoniker
 * ```
 *
 * @param defaultInvokeImpl - Fallback for non-spatial Tauri commands.
 *   The end-to-end test uses this to serve bootstrap commands like
 *   `get_board_data`, `list_entities`, `get_ui_state`, etc.
 */
export function setupSpatialHarness(opts?: {
  defaultInvokeImpl?: DefaultInvokeImpl;
}): SpatialHarness {
  mockInvoke.mockClear();
  mockListen.mockClear();
  listeners.clear();
  const harness = installShadowNavigator(opts?.defaultInvokeImpl);
  return {
    ...harness,
    mockInvoke,
    fireFocusChanged,
  };
}

