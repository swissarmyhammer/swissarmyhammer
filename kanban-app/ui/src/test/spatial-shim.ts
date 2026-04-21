/**
 * JS port of the Rust `SpatialState` + `spatial_nav` modules.
 *
 * This shim exists so vitest-browser tests can drive the real React tree
 * (FocusLayer, FocusScope, board/grid/inspector components) end-to-end
 * without a running Tauri backend. The React code invokes `spatial_*`
 * commands through `@tauri-apps/api/core`; `setupSpatialShim()` mocks
 * that module and routes every invoke into this class.
 *
 * The shim MUST stay behavior-equivalent to
 * `swissarmyhammer-spatial-nav/src/spatial_state.rs` and
 * `.../spatial_nav.rs`. Parity is verified by `spatial-shim-parity.test.ts`,
 * which runs a shared case list against this shim and compares the output
 * against the same expectations the Rust unit tests assert.
 *
 * Two-tier design rationale:
 * - **Tier 1** (Rust unit tests) — pure algorithm coverage for the beam
 *   test, scoring, overrides, layers, focus memory.
 * - **Tier 2** (this shim + vitest-browser) — exercises the React ↔ state
 *   wiring: ResizeObserver measurement, claim callbacks flipping
 *   `data-focused`, scope chain walking, keybinding resolution.
 *
 * Naming and field shapes mirror the Rust structs so the shim can slot
 * in behind the `invoke()` surface with no translation. When Rust drifts,
 * the parity test fails first, forcing the shim to catch up.
 */

/** Axis-aligned bounding rectangle. Mirrors `spatial_state::Rect`. */
export interface ShimRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Right edge of a rect: `x + width`. */
function rectRight(r: ShimRect): number {
  return r.x + r.width;
}

/** Bottom edge of a rect: `y + height`. */
function rectBottom(r: ShimRect): number {
  return r.y + r.height;
}

/** A registered spatial entry. Mirrors `spatial_state::SpatialEntry`. */
export interface ShimSpatialEntry {
  key: string;
  moniker: string;
  rect: ShimRect;
  layerKey: string;
  parentScope: string | null;
  /**
   * Direction → override target moniker.
   *
   * - `Some(moniker)` in Rust → string value here → redirect navigation
   *   to the entry holding that moniker.
   * - `Some(None)` in Rust → `null` value here → block navigation in
   *   that direction.
   * - Missing key → fall through to spatial beam test.
   */
  overrides: Record<string, string | null>;
}

/** A focus layer entry in the layer stack. Mirrors `spatial_state::LayerEntry`. */
export interface ShimLayerEntry {
  key: string;
  name: string;
  lastFocused: string | null;
}

/**
 * Event payload emitted when the focused spatial key changes.
 * Wire field names are snake_case to match the Rust struct serialization.
 */
export interface FocusChangedPayload {
  prev_key: string | null;
  next_key: string | null;
}

/** Navigation direction strings accepted by `navigate()`. */
export type ShimDirection =
  | "Up"
  | "Down"
  | "Left"
  | "Right"
  | "First"
  | "Last"
  | "RowStart"
  | "RowEnd";

// ---------------------------------------------------------------------------
// Pure algorithm helpers — ported from spatial_nav.rs
// ---------------------------------------------------------------------------

/** Center x of a rect. */
function centerX(r: ShimRect): number {
  return r.x + r.width / 2;
}

/** Center y of a rect. */
function centerY(r: ShimRect): number {
  return r.y + r.height / 2;
}

/**
 * Is the candidate strictly beyond the source edge in the given direction?
 *
 * Edge commands return false — they do not use direction-based filtering.
 */
function isInDirection(
  source: ShimRect,
  candidate: ShimRect,
  direction: ShimDirection,
): boolean {
  switch (direction) {
    case "Right":
      return candidate.x >= rectRight(source);
    case "Left":
      return rectRight(candidate) <= source.x;
    case "Down":
      return candidate.y >= rectBottom(source);
    case "Up":
      return rectBottom(candidate) <= source.y;
    default:
      return false;
  }
}

/**
 * Does the candidate fall inside the perpendicular beam of the source?
 *
 * Horizontal directions use the source's y-range; vertical directions use
 * the source's x-range.
 */
function isInBeam(
  source: ShimRect,
  candidate: ShimRect,
  direction: ShimDirection,
): boolean {
  if (direction === "Right" || direction === "Left") {
    return candidate.y < rectBottom(source) && rectBottom(candidate) > source.y;
  }
  if (direction === "Up" || direction === "Down") {
    return candidate.x < rectRight(source) && rectRight(candidate) > source.x;
  }
  return false;
}

/** Android FocusFinder scoring: `13 * major² + minor²`. */
function score(
  source: ShimRect,
  candidate: ShimRect,
  direction: ShimDirection,
): number {
  let major = 0;
  let minor = 0;
  switch (direction) {
    case "Right":
      major = candidate.x - rectRight(source);
      minor = Math.abs(centerY(candidate) - centerY(source));
      break;
    case "Left":
      major = source.x - rectRight(candidate);
      minor = Math.abs(centerY(candidate) - centerY(source));
      break;
    case "Down":
      major = candidate.y - rectBottom(source);
      minor = Math.abs(centerX(candidate) - centerX(source));
      break;
    case "Up":
      major = source.y - rectBottom(candidate);
      minor = Math.abs(centerX(candidate) - centerX(source));
      break;
    default:
      return 0;
  }
  return 13 * major * major + minor * minor;
}

/** Does the candidate overlap the source's y-range? Used by Row edge commands. */
function overlapsYRange(source: ShimRect, candidate: ShimRect): boolean {
  return candidate.y < rectBottom(source) && rectBottom(candidate) > source.y;
}

/** Cardinal-direction navigation with beam test + scoring. */
function findCardinal(
  source: ShimSpatialEntry,
  candidates: ShimSpatialEntry[],
  direction: ShimDirection,
): string | null {
  const inBeam: Array<[ShimSpatialEntry, number]> = [];
  const outBeam: Array<[ShimSpatialEntry, number]> = [];
  for (const c of candidates) {
    if (!isInDirection(source.rect, c.rect, direction)) continue;
    const s = score(source.rect, c.rect, direction);
    if (isInBeam(source.rect, c.rect, direction)) {
      inBeam.push([c, s]);
    } else {
      outBeam.push([c, s]);
    }
  }
  const pool = inBeam.length > 0 ? inBeam : outBeam;
  if (pool.length === 0) return null;
  let best = pool[0];
  for (let i = 1; i < pool.length; i++) {
    if (pool[i][1] < best[1]) best = pool[i];
  }
  return best[0].key;
}

/** First: topmost-leftmost, sorted by (y, x). */
function findEdgeFirst(candidates: ShimSpatialEntry[]): string | null {
  if (candidates.length === 0) return null;
  let best = candidates[0];
  for (let i = 1; i < candidates.length; i++) {
    const c = candidates[i];
    if (
      c.rect.y < best.rect.y ||
      (c.rect.y === best.rect.y && c.rect.x < best.rect.x)
    ) {
      best = c;
    }
  }
  return best.key;
}

/** Last: bottommost-rightmost, sorted by (bottom desc, right desc). */
function findEdgeLast(candidates: ShimSpatialEntry[]): string | null {
  if (candidates.length === 0) return null;
  let best = candidates[0];
  for (let i = 1; i < candidates.length; i++) {
    const c = candidates[i];
    const cb = rectBottom(c.rect);
    const bb = rectBottom(best.rect);
    if (cb > bb || (cb === bb && rectRight(c.rect) > rectRight(best.rect))) {
      best = c;
    }
  }
  return best.key;
}

/** RowStart: leftmost candidate overlapping the source's y-range. */
function findRowStart(
  source: ShimSpatialEntry,
  candidates: ShimSpatialEntry[],
): string | null {
  let best: ShimSpatialEntry | null = null;
  for (const c of candidates) {
    if (!overlapsYRange(source.rect, c.rect)) continue;
    if (!best || c.rect.x < best.rect.x) best = c;
  }
  return best?.key ?? null;
}

/** RowEnd: rightmost candidate overlapping the source's y-range. */
function findRowEnd(
  source: ShimSpatialEntry,
  candidates: ShimSpatialEntry[],
): string | null {
  let best: ShimSpatialEntry | null = null;
  let bestRight = -Infinity;
  for (const c of candidates) {
    if (!overlapsYRange(source.rect, c.rect)) continue;
    const r = rectRight(c.rect);
    if (r > bestRight) {
      best = c;
      bestRight = r;
    }
  }
  return best?.key ?? null;
}

/**
 * Dispatch to the correct strategy for a direction. Caller MUST exclude the
 * source entry from `candidates` (same contract as the Rust `find_target`).
 */
export function findTarget(
  source: ShimSpatialEntry,
  candidates: ShimSpatialEntry[],
  direction: ShimDirection,
): string | null {
  switch (direction) {
    case "First":
      return findEdgeFirst(candidates);
    case "Last":
      return findEdgeLast(candidates);
    case "RowStart":
      return findRowStart(source, candidates);
    case "RowEnd":
      return findRowEnd(source, candidates);
    default:
      return findCardinal(source, candidates, direction);
  }
}

/**
 * Container-first search: siblings sharing `parent_scope` are searched
 * before the full candidate set. When the source has no parent, falls
 * directly through to the full set.
 */
export function containerFirstSearch(
  source: ShimSpatialEntry,
  candidates: ShimSpatialEntry[],
  direction: ShimDirection,
): string | null {
  if (source.parentScope !== null) {
    const scoped = candidates.filter(
      (c) => c.parentScope === source.parentScope,
    );
    const hit = findTarget(source, scoped, direction);
    if (hit !== null) return hit;
  }
  return findTarget(source, candidates, direction);
}

// ---------------------------------------------------------------------------
// SpatialState shim — mirrors spatial_state.rs
// ---------------------------------------------------------------------------

/**
 * In-memory spatial state. One instance per test.
 *
 * Thread safety is irrelevant in JS, so no lock — but the mutation /
 * event-emission shape matches the Rust `RwLock` version exactly: every
 * mutator returns `FocusChangedPayload | null` and the caller decides
 * whether to emit the event.
 */
export class SpatialStateShim {
  private entries = new Map<string, ShimSpatialEntry>();
  private focusedKey: string | null = null;
  private layers: ShimLayerEntry[] = [];

  /** Save the outgoing focused key as `lastFocused` on its owning layer. */
  private saveFocusMemory(prevKey: string): void {
    const entry = this.entries.get(prevKey);
    if (!entry) return;
    const layer = this.layers.find((l) => l.key === entry.layerKey);
    if (layer) layer.lastFocused = prevKey;
  }

  /** Register (upsert) a spatial entry. */
  register(entry: ShimSpatialEntry): void {
    this.entries.set(entry.key, { ...entry });
  }

  /** Register multiple entries at once. */
  registerBatch(entries: ShimSpatialEntry[]): void {
    for (const e of entries) this.entries.set(e.key, { ...e });
  }

  /**
   * Unregister by key.
   *
   * If the removed entry was focused, the "something is always focused"
   * invariant requires picking a successor before clearing focus.
   * Mirrors `SpatialState::unregister` in Rust exactly:
   *
   * 1. **Layer focus memory** — if the removed entry's owning layer has
   *    a `lastFocused` still registered (and not the key being removed),
   *    reuse it.
   * 2. **Sibling in same `parentScope`** — otherwise, the top-left
   *    registered sibling in the same parent and layer.
   * 3. **First-in-layer** — else, the top-left registered entry in the
   *    owning layer.
   * 4. `null` — only when the layer has no other entries.
   */
  unregister(key: string): FocusChangedPayload | null {
    const removed = this.entries.get(key);
    this.entries.delete(key);
    if (this.focusedKey !== key) return null;
    const prev = this.focusedKey;
    const successor = removed ? this.pickSuccessor(key, removed) : null;
    this.focusedKey = successor;
    return { prev_key: prev, next_key: successor };
  }

  /**
   * Pick a replacement focus key when `removingKey` (the currently
   * focused key) is being unregistered. See `unregister()` rustdoc for
   * the priority order. Must stay in sync with
   * `SpatialStateInner::pick_successor` in Rust.
   */
  private pickSuccessor(
    removingKey: string,
    entry: ShimSpatialEntry,
  ): string | null {
    // 1. Layer focus memory.
    const layer = this.layers.find((l) => l.key === entry.layerKey);
    if (layer && layer.lastFocused) {
      const last = layer.lastFocused;
      if (last !== removingKey && this.entries.has(last)) {
        return last;
      }
    }
    // 2. Sibling in same parent_scope (same layer).
    if (entry.parentScope !== null) {
      const sibling = this.findTopLeft(
        (e) =>
          e.key !== removingKey &&
          e.layerKey === entry.layerKey &&
          e.parentScope === entry.parentScope,
      );
      if (sibling !== null) return sibling;
    }
    // 3. First-in-layer by position.
    return this.findTopLeft(
      (e) => e.key !== removingKey && e.layerKey === entry.layerKey,
    );
  }

  /**
   * Upper-left registered entry key (smallest y, then smallest x) that
   * matches the predicate. Shared between `pickSuccessor`,
   * `fallbackToFirst`, and `focusFirstInLayer` so they all agree with
   * `Direction::First`.
   */
  private findTopLeft(
    predicate: (e: ShimSpatialEntry) => boolean,
  ): string | null {
    let best: ShimSpatialEntry | null = null;
    for (const e of this.entries.values()) {
      if (!predicate(e)) continue;
      if (
        !best ||
        e.rect.y < best.rect.y ||
        (e.rect.y === best.rect.y && e.rect.x < best.rect.x)
      ) {
        best = e;
      }
    }
    return best?.key ?? null;
  }

  /**
   * Unregister multiple entries at once. Emits a single focus-changed
   * event with `next_key: null` when the focused key is in the batch.
   */
  unregisterBatch(keys: string[]): FocusChangedPayload | null {
    let lostFocus = false;
    for (const key of keys) {
      this.entries.delete(key);
      if (this.focusedKey === key) lostFocus = true;
    }
    if (lostFocus) {
      const prev = this.focusedKey;
      this.focusedKey = null;
      return { prev_key: prev, next_key: null };
    }
    return null;
  }

  /** Update an existing entry's rect. No-op if the key is not registered. */
  updateRect(key: string, rect: ShimRect): void {
    const entry = this.entries.get(key);
    if (entry) entry.rect = rect;
  }

  /**
   * Set focus to a key. Returns the transition event if it changed, or
   * null if the key is already focused or not registered.
   */
  focus(key: string): FocusChangedPayload | null {
    if (this.focusedKey === key) return null;
    if (!this.entries.has(key)) return null;
    const prev = this.focusedKey;
    if (prev) this.saveFocusMemory(prev);
    this.focusedKey = key;
    return { prev_key: prev, next_key: key };
  }

  /** Clear focus. Returns the event if something was focused. */
  clearFocus(): FocusChangedPayload | null {
    if (this.focusedKey === null) return null;
    const prev = this.focusedKey;
    this.focusedKey = null;
    return { prev_key: prev, next_key: null };
  }

  /** The currently focused spatial key, or null. */
  focusedKeySnapshot(): string | null {
    return this.focusedKey;
  }

  /** Look up a registered entry by key. */
  get(key: string): ShimSpatialEntry | null {
    const e = this.entries.get(key);
    return e ? { ...e } : null;
  }

  /** Number of registered entries. */
  get size(): number {
    return this.entries.size;
  }

  /** Push a focus layer onto the stack. */
  pushLayer(key: string, name: string): void {
    this.layers.push({ key, name, lastFocused: null });
  }

  /**
   * Remove a layer by key. If the layer now on top has a `lastFocused`
   * that is still registered, restore focus to it and return the event.
   */
  removeLayer(key: string): FocusChangedPayload | null {
    const idxBefore = this.layers.length;
    this.layers = this.layers.filter((l) => l.key !== key);
    if (this.layers.length === idxBefore) return null;
    const top = this.layers[this.layers.length - 1];
    const restoreKey = top?.lastFocused ?? null;
    if (restoreKey && this.entries.has(restoreKey)) {
      const prev = this.focusedKey;
      this.focusedKey = restoreKey;
      return { prev_key: prev, next_key: restoreKey };
    }
    return null;
  }

  /** Active (topmost) layer entry, or null. */
  activeLayer(): ShimLayerEntry | null {
    return this.layers[this.layers.length - 1] ?? null;
  }

  /**
   * Focus the upper-left (first) entry in the given layer. Mirrors the
   * Rust `SpatialState::focus_first_in_layer` behaviour exactly:
   *
   * - No-op (returns `null`) when the layer has no registered entries.
   * - No-op when the focused key already belongs to the given layer —
   *   don't override manual focus that landed inside the layer first.
   * - Otherwise picks the entry with the smallest `(y, x)` and focuses
   *   it, saving the outgoing focused key as `lastFocused` on its
   *   owning layer (same focus-memory contract as `focus()`).
   */
  focusFirstInLayer(layerKey: string): FocusChangedPayload | null {
    // Don't override manual focus already inside the target layer.
    if (this.focusedKey !== null) {
      const focusedEntry = this.entries.get(this.focusedKey);
      if (focusedEntry && focusedEntry.layerKey === layerKey) {
        return null;
      }
    }

    const targetKey = this.findTopLeft((e) => e.layerKey === layerKey);
    if (!targetKey) return null;

    const prev = this.focusedKey;
    if (prev) this.saveFocusMemory(prev);
    this.focusedKey = targetKey;
    return { prev_key: prev, next_key: targetKey };
  }

  /** Number of layers in the stack. */
  layerCount(): number {
    return this.layers.length;
  }

  /** Snapshot of all registered entries. Order is insertion order. */
  entriesSnapshot(): ShimSpatialEntry[] {
    return Array.from(this.entries.values()).map((e) => ({ ...e }));
  }

  /** Snapshot of the layer stack, bottom-first. */
  layersSnapshot(): ShimLayerEntry[] {
    return this.layers.map((l) => ({ ...l }));
  }

  /**
   * Navigate from a key in a direction. Applies override → container-first
   * → spatial beam test. Matches `SpatialState::navigate` in Rust exactly.
   *
   * `fromKey` is optional: `null` or an unregistered key triggers the
   * [`fallbackToFirst`] safety net, picking the top-left entry of the
   * active layer. This keeps the "something is always focused" invariant
   * recoverable when React fires a nav key with null/stale focus (e.g.
   * after a view swap or before React consumes a `focus-changed`).
   */
  navigate(
    fromKey: string | null,
    direction: ShimDirection,
  ): FocusChangedPayload | null {
    const source = fromKey !== null ? this.entries.get(fromKey) : undefined;
    if (!source) {
      // Null or unregistered source: recover by picking top-left.
      return this.fallbackToFirst();
    }
    const overrideVal = source.overrides[direction];
    if (overrideVal !== undefined) {
      return this.applyOverride(source.key, overrideVal);
    }
    return this.spatialSearch(source.key, source, direction);
  }

  /**
   * Pick the top-left entry in the active layer and focus it. Returns
   * `null` (no event) when there is no active layer, no entries in it,
   * or the top-left is already focused.
   *
   * Mirrors `SpatialState::fallback_to_first` in Rust and shares the
   * save-focus-memory contract with {@link focus}.
   */
  private fallbackToFirst(): FocusChangedPayload | null {
    const activeLayerKey = this.activeLayer()?.key;
    if (!activeLayerKey) return null;
    const targetKey = this.findTopLeft((e) => e.layerKey === activeLayerKey);
    if (!targetKey) return null;
    if (this.focusedKey === targetKey) return null;
    const prev = this.focusedKey;
    if (prev) this.saveFocusMemory(prev);
    this.focusedKey = targetKey;
    return { prev_key: prev, next_key: targetKey };
  }

  /**
   * Apply an override value: `null` blocks, string redirects to the first
   * entry holding that moniker in the active layer.
   */
  private applyOverride(
    fromKey: string,
    overrideVal: string | null,
  ): FocusChangedPayload | null {
    if (overrideVal === null) return null;
    const activeLayerKey = this.activeLayer()?.key ?? null;
    let targetKey: string | null = null;
    for (const entry of this.entries.values()) {
      if (
        entry.moniker === overrideVal &&
        entry.key !== fromKey &&
        (activeLayerKey === null || entry.layerKey === activeLayerKey)
      ) {
        targetKey = entry.key;
        break;
      }
    }
    if (!targetKey) return null;
    const prev = this.focusedKey;
    if (prev) this.saveFocusMemory(prev);
    this.focusedKey = targetKey;
    return { prev_key: prev, next_key: targetKey };
  }

  /**
   * Run container-first spatial search against the active-layer candidates
   * and update focus if a winner is found.
   */
  private spatialSearch(
    fromKey: string,
    source: ShimSpatialEntry,
    direction: ShimDirection,
  ): FocusChangedPayload | null {
    const activeLayerKey = this.activeLayer()?.key ?? null;
    const candidates: ShimSpatialEntry[] = [];
    for (const e of this.entries.values()) {
      if (
        e.key !== fromKey &&
        (activeLayerKey === null || e.layerKey === activeLayerKey)
      ) {
        candidates.push(e);
      }
    }
    const winner = containerFirstSearch(source, candidates, direction);
    if (!winner) return null;
    const prev = this.focusedKey;
    if (prev) this.saveFocusMemory(prev);
    this.focusedKey = winner;
    return { prev_key: prev, next_key: winner };
  }
}
