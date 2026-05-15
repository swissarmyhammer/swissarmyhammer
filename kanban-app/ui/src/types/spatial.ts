/**
 * TypeScript branded types mirroring the Rust newtypes from
 * `swissarmyhammer-focus/src/types.rs`.
 *
 * Each branded type is a string with a phantom `__brand` property, which
 * means a plain `string` cannot be passed where a `SegmentMoniker` is expected
 * (and vice versa for `FullyQualifiedMoniker`) — the structural compatibility
 * that normally makes TypeScript loose around primitives is broken on
 * purpose. The brand symbol field is never actually present at runtime; the
 * wire shape is still a bare string thanks to `#[serde(transparent)]` on the
 * Rust side.
 *
 * # Path-monikers identifier model
 *
 * The kernel uses **one** identifier shape per primitive — the
 * `FullyQualifiedMoniker`. The path through the focus hierarchy IS the
 * spatial key. Consumers declare a relative `SegmentMoniker` when
 * constructing a `<FocusLayer>` / `<FocusZone>` / `<FocusScope>` and the
 * FQM is composed by parent/child nesting on the React side via
 * `FullyQualifiedMonikerContext` before being passed to the kernel.
 *
 * There is no UUID-based `SpatialKey` and no flat `Moniker`. Path is the
 * key, the key is exact-match. The two newtypes are deliberately distinct
 * — the type system rejects passing a `SegmentMoniker` where a
 * `FullyQualifiedMoniker` is expected. That is the safety net the
 * path-monikers refactor relies on.
 *
 * Use the `as*` helpers to convert from a plain string when you receive
 * a value from outside the typed surface (e.g. a DOM dataset attribute or
 * a serialized URL parameter). The helpers do not validate — they encode
 * the intent that the caller has asserted the value is well-formed.
 *
 * The `Direction` and `FocusChangedPayload` types encode the wire shapes
 * that cross the Tauri IPC boundary; updating either one in Rust requires
 * matching the change here.
 */

// ---------------------------------------------------------------------------
// Branded primitive types
// ---------------------------------------------------------------------------

/**
 * Tauri window label — which window a scope/layer lives in.
 *
 * Mirrors `swissarmyhammer_focus::types::WindowLabel`. The frontend derives
 * this from `appWindow.label` (the Tauri-supplied window handle); never
 * construct one from a user-supplied string.
 */
export type WindowLabel = string & { readonly __brand: "WindowLabel" };

/**
 * Relative path segment declared by a consumer.
 *
 * Mirrors `swissarmyhammer_focus::types::SegmentMoniker`. Examples:
 * `"window"`, `"inspector"`, `"card:T1"`, `"field:T1.title"`. Composed
 * with a parent FQM via `composeFq` to form a canonical key.
 *
 * The brand is distinct from `FullyQualifiedMoniker` so the type system
 * rejects passing a segment where a fully-qualified path is expected
 * (e.g. `setFocus(segment)` is a compile error).
 */
export type SegmentMoniker = string & { readonly __brand: "SegmentMoniker" };

/**
 * Canonical path through the focus hierarchy.
 *
 * Mirrors `swissarmyhammer_focus::types::FullyQualifiedMoniker`. The FQM
 * IS the spatial key — used as the registry key, the focus identity, and
 * the wire-format identifier on every spatial-nav IPC. Examples:
 * `"/window"`, `"/window/inspector"`, `"/window/inspector/field:T1.title"`.
 *
 * Construct via `composeFq(parent, segment)` (descendants of a primitive)
 * or `fqRoot(segment)` (layer roots). The brand is distinct from
 * `SegmentMoniker` so accidentally passing a segment to a kernel call
 * site that expects a fully-qualified path is a compile error.
 */
export type FullyQualifiedMoniker = string & {
  readonly __brand: "FullyQualifiedMoniker";
};

/**
 * Layer role tag — `"window"`, `"inspector"`, `"dialog"`, `"palette"`.
 *
 * Mirrors `swissarmyhammer_focus::types::LayerName`. This is metadata
 * attached to a layer via `spatial_push_layer`; it is NOT the layer's
 * spatial identity (the layer's FQM serves that role).
 */
export type LayerName = string & { readonly __brand: "LayerName" };

/**
 * Logical pixels for spatial-nav rect math.
 *
 * Mirrors `swissarmyhammer_focus::types::Pixels`. The `number` brand keeps
 * `Pixels` from accidentally trading places with other numeric values
 * (zoom factors, ordinal indices) in spatial-nav code.
 */
export type Pixels = number & { readonly __brand: "Pixels" };

// ---------------------------------------------------------------------------
// Brand helpers
// ---------------------------------------------------------------------------

/**
 * Tag a plain string as a `WindowLabel`.
 *
 * Used at the IPC boundary — `appWindow.label` returns a plain `string`,
 * and this helper records that the caller has asserted the value is in
 * fact a Tauri window label.
 */
export const asWindowLabel = (s: string): WindowLabel => s as WindowLabel;

/**
 * Tag a plain string as a `SegmentMoniker`.
 *
 * Used by consumers of the spatial primitives to declare the relative
 * segment for a `<FocusLayer name=...>`, `<FocusZone moniker=...>`, or
 * `<FocusScope moniker=...>`. The helper does not validate the segment;
 * callers are expected to pass values like `"card:T1"` or `"field:title"`.
 */
export const asSegment = (s: string): SegmentMoniker => s as SegmentMoniker;

/**
 * Tag a plain string as a `FullyQualifiedMoniker`.
 *
 * Used at the IPC boundary when reading a path-shaped string from a
 * non-typed source (e.g. a DOM dataset attribute or a `focus-changed`
 * event payload). Production code inside the React tree should compose
 * FQMs via `composeFq` rather than tagging raw strings — the brand
 * encodes the intent that a caller has asserted the value is canonical.
 */
export const asFq = (s: string): FullyQualifiedMoniker =>
  s as FullyQualifiedMoniker;

/** Tag a plain string as a `LayerName`. */
export const asLayerName = (s: string): LayerName => s as LayerName;

/** Tag a plain number as `Pixels`. */
export const asPixels = (n: number): Pixels => n as Pixels;

// ---------------------------------------------------------------------------
// FQM composition
// ---------------------------------------------------------------------------

/**
 * Path separator used by `composeFq` and `fqRoot`. Mirrors the Rust
 * kernel's `FQ_SEPARATOR`. Both sides agree on `'/'` — wire-format detail.
 */
const FQ_SEPARATOR = "/";

/**
 * Compose a child FQM by appending `segment` to `parent` with the path
 * separator. Result: `"<parent>/<segment>"`.
 *
 * This is the only way to construct a non-root FQM in well-formed React
 * code. The Rust kernel performs the same composition via
 * `FullyQualifiedMoniker::compose` so both sides agree on the canonical
 * path string for every primitive.
 *
 * @example
 * const window = fqRoot(asSegment("window"));
 * const inspector = composeFq(window, asSegment("inspector"));
 * // inspector === "/window/inspector"
 */
export function composeFq(
  parent: FullyQualifiedMoniker,
  segment: SegmentMoniker,
): FullyQualifiedMoniker {
  return `${parent}${FQ_SEPARATOR}${segment}` as FullyQualifiedMoniker;
}

/**
 * Construct the FQM for a **layer root** — the topmost segment in a
 * window's spatial hierarchy. Result: `"/<segment>"`.
 *
 * Layer roots are the only primitives constructed without a parent FQM
 * — every other zone or scope composes against a parent via `composeFq`.
 *
 * @example
 * fqRoot(asSegment("window")) // "/window"
 */
export function fqRoot(segment: SegmentMoniker): FullyQualifiedMoniker {
  return `${FQ_SEPARATOR}${segment}` as FullyQualifiedMoniker;
}

/**
 * Read the trailing segment of an FQM — the leaf primitive's relative
 * declaration.
 *
 * Used by display callers that want to render the leaf moniker (e.g.
 * `"field:T1.title"` rather than the full `/window/.../field:T1.title`
 * path) without adding an extra prop to thread the segment through.
 *
 * Returns the input as-is when it has no separator (a malformed FQM, or
 * an empty layer-less path — both are torn states the kernel does not
 * produce in well-formed code).
 */
export function fqLastSegment(fq: FullyQualifiedMoniker): SegmentMoniker {
  const idx = fq.lastIndexOf(FQ_SEPARATOR);
  if (idx < 0) return fq as unknown as SegmentMoniker;
  return fq.slice(idx + 1) as SegmentMoniker;
}

// ---------------------------------------------------------------------------
// Direction enum — matches the Rust `Direction` serialization
// ---------------------------------------------------------------------------

/**
 * Navigation direction.
 *
 * Mirrors the lower-cased serialization of
 * `swissarmyhammer_focus::types::Direction`. The Rust enum derives
 * `#[serde(rename_all = "lowercase")]`, so the wire shape is the
 * string literal — that is what this union encodes.
 *
 * The four cardinal arrows (`up`/`down`/`left`/`right`) drive the
 * Android-style beam-search algorithm. The two edge commands focus
 * the focused scope's first / last child:
 *
 * - `first` / `last` — topmost-then-leftmost / bottommost-then-rightmost
 *   child of the focused scope. On a leaf (no children) the focused FQM
 *   is echoed (semantic no-op).
 *
 * The pre-redesign `"rowstart"` / `"rowend"` members were dropped — the
 * user model has no separate "first in row" concept, so they collapse
 * onto `first` / `last`. The Rust kernel keeps the variants behind
 * `#[deprecated]` for one release; new TypeScript code must use
 * `"first"` / `"last"` only.
 *
 * Drill-in / drill-out are separate commands, not directions.
 */
export type Direction = "up" | "down" | "left" | "right" | "first" | "last";

// ---------------------------------------------------------------------------
// Wire payloads
// ---------------------------------------------------------------------------

/**
 * Wire payload for the `focus-changed` Tauri event.
 *
 * Mirrors `swissarmyhammer_focus::state::FocusChangedEvent`. Field names
 * use snake_case to match Rust's serde defaults. The payload carries the
 * fully-qualified path on both sides of the transition plus the trailing
 * segment of the new focus (read off the registry at event-construction
 * time so React consumers do not need to look it up themselves).
 */
export interface FocusChangedPayload {
  /** Window in which focus changed. */
  readonly window_label: WindowLabel;
  /** Previously focused fully-qualified moniker, if any. */
  readonly prev_fq: FullyQualifiedMoniker | null;
  /** Newly focused fully-qualified moniker, if any. */
  readonly next_fq: FullyQualifiedMoniker | null;
  /**
   * Trailing segment of the newly focused FQM, if `next_fq` is non-null.
   *
   * Provided for display callers (and a small number of legacy entity-
   * focus consumers) that want the leaf segment without parsing the
   * path themselves.
   */
  readonly next_segment: SegmentMoniker | null;
}

/**
 * Axis-aligned rectangle in viewport pixel coordinates.
 *
 * Mirrors `swissarmyhammer_focus::types::Rect`. `x` / `y` are the
 * top-left corner; `width` / `height` are non-negative extents. `Pixels`
 * is a branded `number` so accidentally passing a zoom factor (or an
 * ordinal) where a pixel value is expected is a compile error.
 */
export interface Rect {
  readonly x: Pixels;
  readonly y: Pixels;
  readonly width: Pixels;
  readonly height: Pixels;
}

/**
 * Per-direction navigation overrides for a `<FocusScope>` or `<FocusZone>`.
 *
 * Mirrors `HashMap<Direction, Option<FullyQualifiedMoniker>>` on the Rust
 * side. Missing keys mean "fall through to beam search"; an explicit
 * `null` value is a "wall" that blocks navigation in that direction; a
 * fully-qualified moniker value is a redirect to the named target.
 */
export type FocusOverrides = Partial<
  Record<Direction, FullyQualifiedMoniker | null>
>;

/**
 * One scope's contribution to a navigation snapshot.
 *
 * Mirrors `swissarmyhammer_focus::snapshot::SnapshotScope`. Field names
 * use snake_case so the JSON payload built on the React side
 * deserializes verbatim into the Rust kernel.
 */
export interface SnapshotScope {
  /** Canonical fully-qualified path to this scope. */
  readonly fq: FullyQualifiedMoniker;
  /** Viewport rect in logical pixels at snapshot time. */
  readonly rect: Rect;
  /**
   * FQM of the immediate enclosing scope or zone, or `null` when this
   * scope is registered directly under the layer root.
   */
  readonly parent_zone: FullyQualifiedMoniker | null;
  /** Per-direction overrides; `{}` means "no overrides". */
  readonly nav_override: FocusOverrides;
}

/**
 * A snapshot of every `<FocusScope>` mounted under a single
 * `<FocusLayer>`.
 *
 * Mirrors `swissarmyhammer_focus::snapshot::NavSnapshot`. Built per
 * decision (per-nav, per-focus, per-focus-lost) and shipped to the
 * kernel inline so the kernel never has to read scope state out-of-band.
 */
export interface NavSnapshot {
  /** FQM of the layer this snapshot describes. */
  readonly layer_fq: FullyQualifiedMoniker;
  /** All scopes registered in the layer at snapshot time. */
  readonly scopes: SnapshotScope[];
}
