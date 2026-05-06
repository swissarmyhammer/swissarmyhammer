/**
 * `LayerScopeRegistry` — React-side registry of every `<FocusScope>`
 * mounted under a single `<FocusLayer>`.
 *
 * # What this is, and why
 *
 * Step 1 of the spatial-nav redesign described in card
 * `01KQTC1VNQM9KC90S65P7QX9N1`. The kernel currently keeps a replica of
 * the React scope tree (`SpatialRegistry::scopes`) populated over async
 * IPC; the redesign moves that registry into React and lets the kernel
 * read a fresh snapshot per decision instead. This file is the *first*
 * step toward that cutover — it stands the React-side registry up
 * **alongside** the existing kernel sync. Both sources of truth coexist
 * for now; nothing is removed from the kernel path. The point of this
 * step is purely to give later steps a place to build snapshots from
 * React state, and to provide a parity diagnostic that proves the dual-
 * source model works before the cutover happens.
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
 * `getBoundingClientRect()` *at call time*, so the snapshot reflects the
 * current viewport regardless of how stale the kernel's replica is.
 *
 * `navOverride` is also read live — the registry stores the latest
 * `navOverride` value. This is a deliberate behavior change from the
 * existing kernel registration path, which snapshots `navOverride` only
 * at register time and ignores mid-life changes (see the comment in
 * `focus-scope.tsx`'s navOverride contract). The redesign explicitly
 * improves this; a `<FocusScope>` whose `navOverride` prop changes after
 * mount will see the new value reflected in the next snapshot.
 *
 * # Out of scope for this step
 *
 * - Sending snapshots over IPC (steps 6–8 of the parent card).
 * - Removing the kernel sync (steps 10–12).
 * - Changing kernel behavior (steps 2–5).
 *
 * No existing call sites are changed. The registry is purely additive in
 * step 1.
 */

import { createContext, useContext } from "react";
import type {
  FocusOverrides,
  FullyQualifiedMoniker,
  NavSnapshot,
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
export class LayerScopeRegistry {
  /**
   * The layer this registry is scoped to. Used by `buildSnapshot` so
   * callers don't have to thread the layer FQM through every call site.
   */
  readonly layerFq: FullyQualifiedMoniker;

  private readonly store: Map<FullyQualifiedMoniker, ScopeEntry> = new Map();

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
  }

  /**
   * Remove the entry for `fq`. No-op if `fq` is not registered, so the
   * cleanup path is safe to call unconditionally from a `useEffect`
   * cleanup function.
   */
  delete(fq: FullyQualifiedMoniker): void {
    this.store.delete(fq);
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
