/**
 * `LayerScopeRegistry` — React-side registry of every `<FocusScope>`
 * mounted under a single `<FocusLayer>`. The sole scope-tracking
 * authority: the kernel reads scope state only via per-decision
 * snapshots built from this registry.
 *
 * # Layer-scoped, not global
 *
 * Each `<FocusLayer>` instance creates its own `LayerScopeRegistry` and
 * publishes it via `LayerScopeRegistryContext`. Modal layers (inspector,
 * dialog, palette) get their own registry — registries do **not** cross
 * modal boundaries. This matches the kernel: pathfinding and fallback
 * are scoped to a single layer's scopes.
 *
 * # Live-read, not snapshotted-at-register
 *
 * The registry holds the React `RefObject<HTMLElement>` for each scope,
 * not a frozen rect. `buildSnapshot()` walks the entries and reads
 * `getBoundingClientRect()` *at call time*, so the snapshot always
 * reflects the current viewport.
 *
 * `navOverride` is also read live — the registry stores the latest
 * `navOverride` value, so a `<FocusScope>` whose prop changes after
 * mount will see the new value reflected in the next snapshot.
 */

import { createContext, useContext } from "react";
import type {
  FocusOverrides,
  FullyQualifiedMoniker,
  NavSnapshot,
  Rect,
  SegmentMoniker,
  SnapshotScope,
} from "@/types/spatial";
import { asPixels } from "@/types/spatial";

// Re-export the wire-shape types so existing call sites that import
// them from this module keep compiling. The canonical home for these
// types is `@/types/spatial` — they sit there because they cross the
// Tauri IPC boundary.
export type { NavSnapshot, SnapshotScope };

// ---------------------------------------------------------------------------
// ScopeEntry — what each FQM maps to in the registry
// ---------------------------------------------------------------------------

/**
 * The per-scope record held in a `LayerScopeRegistry`.
 *
 * Holds a *ref* to the underlying DOM node (so rect reads happen at
 * snapshot time rather than at registration time) plus the structural
 * metadata the kernel's snapshot consumer needs.
 *
 * `navOverride` is intentionally an object, not a ref-snapshot — the
 * registry stores the latest value the consumer passed, so mid-life
 * changes are visible in subsequent snapshots without a re-register.
 *
 * `lastKnownRect` caches the most recent bounding rect sampled for this
 * scope. Two writers seed and refresh it: the mount-time `updateRect`
 * call from `<FocusScope>`'s registration `useEffect`, and the scope's
 * `useLayoutEffect` cleanup that resamples just before unmount (while
 * the bound ref is still attached). Held as a cache rather than
 * re-sampled at delete time because React's commit phase nullifies
 * bound `ref` callbacks before `useEffect` cleanups run, so an
 * unmount-time `getBoundingClientRect()` from the deletion listener
 * would observe a detached node. Initialised to `null` for the brief
 * window between `add()` and the first `updateRect()`; `null` means
 * "no rect ever cached".
 */
export interface ScopeEntry {
  /** Ref to the rendered DOM element; read at snapshot time. */
  readonly ref: React.RefObject<HTMLElement | null>;
  /** Enclosing zone's FQM, or `null` at the layer root. */
  readonly parentZone: FullyQualifiedMoniker | null;
  /** Per-direction overrides; `undefined` means "none". */
  readonly navOverride?: FocusOverrides;
  /** The relative segment the scope was mounted with. */
  readonly segment: SegmentMoniker;
  /** Last bounding rect sampled for this scope; `null` until the first
   * `updateRect` call. */
  lastKnownRect: Rect | null;
}

// ---------------------------------------------------------------------------
// LayerScopeRegistry — the registry itself
// ---------------------------------------------------------------------------

/**
 * Per-layer scope registry. Tracks every `<FocusScope>` mounted under a
 * single `<FocusLayer>` by its canonical FQM.
 *
 * Backed by a plain `Map`. `<FocusScope>` calls `add(fq, entry)` from a
 * mount effect and `delete(fq)` from the cleanup. React's effect cleanup
 * is synchronous and deterministic, so the registry is in lockstep with
 * the React tree without involving any IPC.
 *
 * Built as a class (rather than a plain object literal) so all four
 * methods plus `buildSnapshot` share the same backing store via `this`,
 * and so the type system accurately captures the live `Map` semantics
 * (`has`, `entries`) without us re-implementing them.
 */
/**
 * Listener notified after a registry entry has been deleted.
 *
 * Receives the removed FQM and the entry's metadata at the moment of
 * deletion. Fires AFTER the entry leaves the underlying `Map`, so a
 * snapshot built inside the callback correctly excludes the lost FQM.
 */
export type ScopeDeletedListener = (
  fq: FullyQualifiedMoniker,
  entry: ScopeEntry,
) => void;

/**
 * Cross-instance hook fired for every `add` / `delete` on any
 * `LayerScopeRegistry`. Vitest test setups install one to mirror
 * registrations into the per-test mock-invoke history so legacy
 * `registerScopeArgs()` helpers (which scan `mockInvoke.mock.calls` for
 * `spatial_register_scope` entries) keep working after the IPC cutover.
 *
 * Production never installs a hook — the field is `null` by default and
 * the registry skips the call entirely on the hot path.
 */
export interface RegistryEventHook {
  onAdd(layerFq: FullyQualifiedMoniker, fq: FullyQualifiedMoniker, entry: ScopeEntry): void;
  onDelete(layerFq: FullyQualifiedMoniker, fq: FullyQualifiedMoniker, entry: ScopeEntry): void;
}

let globalRegistryHook: RegistryEventHook | null = null;

/**
 * Install a cross-instance hook that fires for every `add` / `delete` on
 * any `LayerScopeRegistry`. Returns the uninstaller. Tests use this to
 * mirror registrations into a captured trace.
 */
export function installRegistryHook(hook: RegistryEventHook): () => void {
  globalRegistryHook = hook;
  return () => {
    if (globalRegistryHook === hook) globalRegistryHook = null;
  };
}

export class LayerScopeRegistry {
  /**
   * The layer this registry is scoped to. Used by `buildSnapshot` so
   * callers don't have to thread the layer FQM through every call site.
   */
  readonly layerFq: FullyQualifiedMoniker;

  private readonly store: Map<FullyQualifiedMoniker, ScopeEntry> = new Map();

  private readonly deletedListeners: Set<ScopeDeletedListener> = new Set();

  /**
   * Construct a fresh, empty registry for the given layer. Each
   * `<FocusLayer>` instantiates one of these in a ref so the registry
   * survives re-renders but tears down with the layer.
   */
  constructor(layerFq: FullyQualifiedMoniker) {
    this.layerFq = layerFq;
  }

  /**
   * Register `fq` with the given `entry`. Replaces any existing entry
   * under the same FQM — re-registration with a fresh ref is a tolerated
   * (though rare) outcome of placeholder/real-mount swaps.
   */
  add(fq: FullyQualifiedMoniker, entry: ScopeEntry): void {
    this.store.set(fq, entry);
    if (globalRegistryHook !== null) {
      try {
        globalRegistryHook.onAdd(this.layerFq, fq, entry);
      } catch (err) {
        console.error("[LayerScopeRegistry] global add hook threw", err);
      }
    }
    // The `import.meta.env.DEV` check is inlined here on purpose — Vite's
    // `define` substitutes the literal at the exact call site, so in a
    // production build this becomes `if (false) { ... }` and the entire
    // block (including the `detectNeedlessNesting` reference) is dead-
    // code-eliminated. Hiding the gate behind a function call would
    // promote the check to runtime and anchor the detector body in the
    // production bundle. Do not extract this `if` into a helper.
    //
    // Microtask (rather than `requestAnimationFrame`) because by the time
    // React's mount effect has called `add()`, the commit phase is over
    // and layout has already run synchronously — `getBoundingClientRect()`
    // returns the painted rect immediately. The microtask delay is needed
    // only so the new entry's mount-time ref is observable from the
    // iteration; rAF would block the warning until the next frame and is
    // unnecessary.
    //
    // The pre-existing partners are captured at call time, before any
    // later sibling mounts in the same commit can race in, so each
    // structural overlap produces exactly one warning (the newer mount),
    // not a pair of mirrored warnings.
    if (import.meta.env.DEV) {
      const priorFqs = new Set(this.store.keys());
      priorFqs.delete(fq);
      queueMicrotask(() => {
        try {
          detectNeedlessNesting(fq, entry, this, priorFqs);
        } catch (err) {
          console.error(
            "[LayerScopeRegistry] needless-nesting detector threw",
            err,
          );
        }
      });
    }
  }

  /**
   * Remove the entry for `fq` and notify deletion listeners.
   *
   * No-op if `fq` is not registered, so the cleanup path is safe to call
   * unconditionally from a `useEffect` cleanup function. Listeners fire
   * AFTER the underlying map deletion so they observe the post-delete
   * registry state — `buildSnapshot()` called from a listener will not
   * include `fq`.
   *
   * Listener exceptions are caught and logged so a single misbehaving
   * subscriber cannot break the cleanup path of an unrelated scope.
   */
  delete(fq: FullyQualifiedMoniker): void {
    const entry = this.store.get(fq);
    if (entry === undefined) return;
    this.store.delete(fq);
    for (const listener of this.deletedListeners) {
      try {
        listener(fq, entry);
      } catch (err) {
        console.error("[LayerScopeRegistry] deleted listener threw", err);
      }
    }
    if (globalRegistryHook !== null) {
      try {
        globalRegistryHook.onDelete(this.layerFq, fq, entry);
      } catch (err) {
        console.error("[LayerScopeRegistry] global delete hook threw", err);
      }
    }
  }

  /**
   * Subscribe to entry deletions. Returns the unsubscribe function — call
   * it on cleanup to remove the listener so a hot-reloaded provider does
   * not leak references.
   */
  onDeleted(listener: ScopeDeletedListener): () => void {
    this.deletedListeners.add(listener);
    return () => {
      this.deletedListeners.delete(listener);
    };
  }

  /**
   * Update the cached `lastKnownRect` for `fq`. No-op if `fq` is not
   * registered.
   *
   * Two call sites invoke this with a freshly sampled bounding rect:
   * the initial mount-time `getBoundingClientRect()` from
   * `<FocusScope>`'s registration `useEffect` (seeds the cache so the
   * focused-scope-unmount IPC has a non-null rect even when unmount
   * happens before the layout-effect cleanup ever runs), and the
   * scope's `useLayoutEffect` cleanup that resamples just before
   * unmount while the bound ref is still attached. The cached rect is
   * what the deletion listener reads to dispatch `spatial_focus_lost`,
   * so it must reflect the most recent live geometry; sampling at
   * delete time would observe a detached node because React clears the
   * bound `ref` during the commit phase before the `useEffect` cleanup
   * that calls `delete()` runs.
   */
  updateRect(fq: FullyQualifiedMoniker, rect: Rect): void {
    const entry = this.store.get(fq);
    if (entry === undefined) return;
    entry.lastKnownRect = rect;
  }

  /** True iff `fq` is currently registered. */
  has(fq: FullyQualifiedMoniker): boolean {
    return this.store.has(fq);
  }

  /**
   * Iterate over every `(fq, entry)` pair in the registry.
   *
   * Returns the underlying `Map`'s iterator so callers can spread to an
   * array or walk lazily. The iteration order is the insertion order of
   * the `Map`, which is the order `<FocusScope>` mount effects ran in
   * — useful for deterministic snapshot building.
   */
  entries(): IterableIterator<[FullyQualifiedMoniker, ScopeEntry]> {
    return this.store.entries();
  }

  /**
   * The number of currently-registered scopes. Primarily useful in
   * tests that want to assert the registry shrinks on unmount without
   * iterating.
   */
  get size(): number {
    return this.store.size;
  }

  /**
   * Build a `NavSnapshot` from the current registry contents.
   *
   * Walks every entry, reads its `getBoundingClientRect()` at call time,
   * and produces a flat `SnapshotScope[]`. Entries whose `ref.current`
   * is `null` are skipped — that happens during the brief window where
   * React has scheduled an unmount but not yet run the cleanup that
   * removes the entry from the registry. Skipping them keeps the
   * snapshot self-consistent (every scope present has a real rect) at
   * the cost of dropping at most one in-flight scope per frame.
   *
   * @param layerFq - the FQM of the layer the snapshot is for. Pass
   *   `registry.layerFq` to build a snapshot for the registry's own
   *   layer, or any other FQM if the caller is composing a snapshot
   *   across layer boundaries (not used in step 1; kept open for later
   *   steps that may need it).
   */
  buildSnapshot(layerFq: FullyQualifiedMoniker): NavSnapshot {
    const scopes: SnapshotScope[] = [];
    for (const [fq, entry] of this.store) {
      const node = entry.ref.current;
      if (node === null) continue;
      const r = node.getBoundingClientRect();
      scopes.push({
        fq,
        rect: {
          x: asPixels(r.x),
          y: asPixels(r.y),
          width: asPixels(r.width),
          height: asPixels(r.height),
        },
        parent_zone: entry.parentZone,
        nav_override: entry.navOverride ?? {},
      });
    }
    return { layer_fq: layerFq, scopes };
  }
}

// ---------------------------------------------------------------------------
// React context plumbing
// ---------------------------------------------------------------------------

/**
 * Per-layer scope registry context.
 *
 * `<FocusLayer>` provides the registry for its own layer; descendant
 * `<FocusScope>` components consume it to register/unregister
 * themselves. The default value is `null` so primitives mounted outside
 * any `<FocusLayer>` (e.g. in unit tests that exercise a single
 * `<FocusScope>` without spinning up the spatial-nav stack) silently
 * skip the registry registration — matching the existing tolerance the
 * kernel-sync code path has for the same scenario.
 */
export const LayerScopeRegistryContext =
  createContext<LayerScopeRegistry | null>(null);

/**
 * Read the enclosing `<FocusLayer>`'s `LayerScopeRegistry`, or `null`
 * if no layer wraps the caller.
 *
 * Returns `null` rather than throwing so a `<FocusScope>` mounted
 * without a `<FocusLayer>` ancestor degrades gracefully — the registry
 * registration is best-effort and additive, exactly like the kernel
 * sync's tolerance for missing-spatial-context callers.
 */
export function useOptionalLayerScopeRegistry(): LayerScopeRegistry | null {
  return useContext(LayerScopeRegistryContext);
}

// ---------------------------------------------------------------------------
// Needless-nesting detection (dev-mode, observability-only)
// ---------------------------------------------------------------------------

/**
 * Pixel tolerance used when comparing two scope rects for "needless
 * nesting" overlap. Two rects are considered to share geometry when
 * their `x`, `y`, right edge (`x + width`), and bottom edge (`y +
 * height`) all agree within this many pixels. A small tolerance
 * absorbs subpixel rendering noise (anti-aliased borders, fractional
 * dpr math) without missing the structural pattern of a parent scope
 * wrapping a single child with no offset.
 */
const NEEDLESS_NESTING_TOLERANCE_PX = 2;

/**
 * `true` when both rects' four sides all agree within `tolerance` px.
 *
 * Compares `x`, `y`, right edge (`x + width`), and bottom edge (`y +
 * height`) absolute differences against `tolerance`. This is the
 * "two scopes share rect" predicate — width/height differences are
 * captured implicitly via the right and bottom edges, so the partner
 * rect must agree on all four sides, not just the origin.
 *
 * Internal to this module — not exported. The `add()` hook gates its
 * call site on a literal `import.meta.env.DEV` so Vite's `define`
 * substitutes `false` in production and the entire detection block is
 * dead-code-eliminated. Keeping this helper unexported lets tree-
 * shaking drop its body from production bundles. Tests reach it via
 * the `__test__` namespace export at the bottom of this module.
 */
function rectsOverlapTightly(
  a: { x: number; y: number; width: number; height: number },
  b: { x: number; y: number; width: number; height: number },
  tolerance: number = NEEDLESS_NESTING_TOLERANCE_PX,
): boolean {
  return (
    Math.abs(a.x - b.x) <= tolerance &&
    Math.abs(a.y - b.y) <= tolerance &&
    Math.abs(a.x + a.width - (b.x + b.width)) <= tolerance &&
    Math.abs(a.y + a.height - (b.y + b.height)) <= tolerance
  );
}

/**
 * Inspect `newEntry` against every live entry in `registry` and emit
 * one structured `console.warn` per partner whose rect overlaps tightly.
 * Observability-only — never throws (callers wrap defensively), never
 * mutates the registry, never affects focus state. Skips entries whose
 * `ref.current` has gone null (the brief unmount window) so a
 * tearing-down sibling cannot trigger a spurious warning.
 *
 * `restrictToFqs`, when provided, narrows the iteration to only those
 * partners that existed when the new entry was registered. The `add`
 * hook uses this so a structural overlap warns exactly once (from the
 * later mount's perspective), rather than producing mirrored warnings
 * when both partners' microtasks run with the registry already
 * containing both.
 *
 * The warning carries both FQMs, both segments, and the shared rect so
 * a developer can locate the offending nesting in the React tree.
 * React DevTools augments `console.warn` with the component stack
 * automatically; the structured payload supplements that with the
 * spatial-nav identifiers.
 *
 * Internal to this module — not exported. See the `rectsOverlapTightly`
 * doc-comment for the rationale; tests use `__test__` below.
 */
function detectNeedlessNesting(
  newFq: FullyQualifiedMoniker,
  newEntry: ScopeEntry,
  registry: LayerScopeRegistry,
  restrictToFqs?: ReadonlySet<FullyQualifiedMoniker>,
): void {
  const newNode = newEntry.ref.current;
  if (newNode === null) return;
  const newRect = newNode.getBoundingClientRect();
  // Pre-layout / detached rects are all zeros; skip rather than warn
  // because a zero-rect "overlap" is a layout-not-yet-run artefact,
  // not a structural nesting bug.
  if (newRect.width === 0 && newRect.height === 0) return;

  for (const [otherFq, otherEntry] of registry.entries()) {
    if (otherFq === newFq) continue;
    if (restrictToFqs !== undefined && !restrictToFqs.has(otherFq)) continue;
    const otherNode = otherEntry.ref.current;
    if (otherNode === null) continue;
    const otherRect = otherNode.getBoundingClientRect();
    if (otherRect.width === 0 && otherRect.height === 0) continue;

    if (rectsOverlapTightly(newRect, otherRect)) {
      console.warn(
        "[spatial-nav] needless-nesting: two scopes share rect",
        {
          newFq,
          otherFq,
          newSegment: newEntry.segment,
          otherSegment: otherEntry.segment,
          rect: {
            x: newRect.x,
            y: newRect.y,
            width: newRect.width,
            height: newRect.height,
          },
          newEntry,
          otherEntry,
        },
      );
    }
  }
}

/**
 * Test-only entry point for the otherwise-internal needless-nesting
 * helpers.
 *
 * The detection helpers (`rectsOverlapTightly`, `detectNeedlessNesting`,
 * `NEEDLESS_NESTING_TOLERANCE_PX`) are deliberately unexported so tree-
 * shaking can drop their bodies from production bundles. Tests still
 * need to exercise them directly, so we expose them through this single
 * named-export object. The `add()` hook is gated on a literal
 * `import.meta.env.DEV`, which Vite's `define` rewrites to `false` in
 * production builds — at that point the entire detector block is dead
 * code, no production import path references the helpers, and tree-
 * shaking is free to drop both the helper bodies and this `__test__`
 * binding from `dist/`.
 *
 * Production code MUST NOT import from `__test__`. The grep-for-
 * `needless-nesting` test in this module's spec verifies the production
 * bundle never reaches this object.
 */
export const __test__ = {
  rectsOverlapTightly,
  detectNeedlessNesting,
  NEEDLESS_NESTING_TOLERANCE_PX,
};
