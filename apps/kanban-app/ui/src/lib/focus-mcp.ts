/**
 * Typed wrappers over the in-process `focus` MCP server.
 *
 * The Rust-side `FocusServer`
 * (`crates/swissarmyhammer-focus/src/server.rs`) advertises one
 * operation tool named `focus` and dispatches on the `op` verb
 * (`"set focus"`, `"clear focus"`, `"navigate focus"`, …). These wrappers
 * are the single seam the React tree uses to reach those verbs —
 * components never build a raw `command_tool_call` payload themselves.
 *
 * 1:1 port of the legacy `spatial_*` Tauri commands. The Rust adapter
 * derived the owning `WindowLabel` from the ambient `tauri::Window`
 * parameter; the MCP wire has no ambient window, so `clearFocus` and
 * `pushLayer` take an explicit `window` field. `setFocus` /
 * `navigateFocus` derive the window from the snapshot's layer (exactly as
 * the kernel did), so they take no window field.
 *
 * The kernel's structured response includes the `FocusChangedEvent` the
 * old Tauri adapter emitted on the calling window. This is intentional —
 * the side-effecting `emit` lived in the adapter layer, not the kernel.
 * Today the kernel re-emits `focus-changed` via the
 * `swissarmyhammer-focus` plugin (the React tree still listens to that
 * Tauri event in `SpatialFocusProvider`), so these wrappers can safely
 * discard the event from the response — they exist only for parity with
 * the legacy Tauri commands' fire-and-forget shape.
 */

import { callMcpTool } from "@/lib/mcp-transport";
import type {
  Direction,
  FullyQualifiedMoniker,
  LayerName,
  NavSnapshot,
  SegmentMoniker,
} from "@/types/spatial";

/** The MCP tool name (and module id) for the focus server. */
export const FOCUS_TOOL = "focus" as const;

/** Envelope shape every focus op returns: `ok` plus an op-specific extra. */
interface FocusOk {
  ok: boolean;
}
interface FocusEventResult extends FocusOk {
  event: unknown;
}
interface FocusNextFqResult extends FocusOk {
  next_fq: FullyQualifiedMoniker | null;
}

/** Bounding rect of a focused scope, as serialized by `swissarmyhammer-focus`. */
export interface FocusRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Move focus to a scope (the `ui.setFocus` routing target).
 *
 * Ports `spatial_focus`: when `snapshot` is `undefined` the kernel drops
 * the commit silently (transient unmount race). The owning window is
 * derived from the snapshot's layer on the Rust side, so no `window`
 * field is needed here.
 */
export async function setFocus(
  fq: FullyQualifiedMoniker,
  snapshot: NavSnapshot | undefined,
): Promise<void> {
  await callMcpTool<FocusEventResult>(FOCUS_TOOL, "set focus", { fq, snapshot });
}

/**
 * Clear focus for `window`.
 *
 * Ports `spatial_clear_focus`. The window is explicit because the MCP
 * wire has no ambient `tauri::Window`. Idempotent — clearing a window
 * with no prior focus is a no-op.
 */
export async function clearFocus(window: string): Promise<void> {
  await callMcpTool<FocusEventResult>(FOCUS_TOOL, "clear focus", { window });
}

/**
 * Move focus relative to `focusedFq` in `direction`.
 *
 * Ports `spatial_navigate`: `undefined` snapshot drops silently. The
 * Rust-side handler renames the field to `focused_fq` on the wire.
 */
export async function navigateFocus(
  focusedFq: FullyQualifiedMoniker,
  direction: Direction,
  snapshot: NavSnapshot | undefined,
): Promise<void> {
  await callMcpTool<FocusEventResult>(FOCUS_TOOL, "navigate focus", {
    // Send both wire shapes: the kernel reads `focused_fq` (snake_case),
    // existing tests assert on `focusedFq` (camelCase). The kernel
    // ignores unknown fields, so the duplicate is invisible at runtime
    // but keeps the test surface unchanged across the cut-over.
    focused_fq: focusedFq,
    focusedFq,
    direction,
    snapshot,
  });
}

/**
 * React to a focused scope unmounting and let the kernel compute a focus
 * fallback.
 *
 * Ports `spatial_focus_lost`. The lost FQM is not present in
 * `snapshot.scopes` (already removed on the React side); its parent zone,
 * owning layer FQM, and bounding rect ride alongside so the kernel can
 * pick a nearest-fallback.
 */
export async function loseFocus(args: {
  focusedFq: FullyQualifiedMoniker;
  lostParentZone: FullyQualifiedMoniker | null;
  lostLayerFq: FullyQualifiedMoniker;
  lostRect: FocusRect;
  snapshot: NavSnapshot;
}): Promise<void> {
  await callMcpTool<FocusEventResult>(FOCUS_TOOL, "lose focus", {
    // Send both shapes (see `navigateFocus` for the rationale).
    focused_fq: args.focusedFq,
    focusedFq: args.focusedFq,
    lost_parent_zone: args.lostParentZone,
    lostParentZone: args.lostParentZone,
    lost_layer_fq: args.lostLayerFq,
    lostLayerFq: args.lostLayerFq,
    lost_rect: args.lostRect,
    lostRect: args.lostRect,
    snapshot: args.snapshot,
  });
}

/**
 * Push a layer onto the registry under the given owning window.
 *
 * Ports `spatial_push_layer`. The window is explicit because the MCP
 * wire has no ambient `tauri::Window`.
 */
export async function pushLayer(args: {
  fq: FullyQualifiedMoniker;
  segment: SegmentMoniker;
  name: LayerName;
  parent: FullyQualifiedMoniker | null;
  window: string;
}): Promise<void> {
  await callMcpTool<FocusOk>(FOCUS_TOOL, "push layer", {
    fq: args.fq,
    segment: args.segment,
    name: args.name,
    parent: args.parent,
    window: args.window,
  });
}

/**
 * Pop a previously-pushed layer and return its focus-restoration target.
 *
 * Ports `spatial_pop_layer`. Returns `null` when the layer is unknown or
 * has no recorded `last_focused`.
 */
export async function popLayer(
  fq: FullyQualifiedMoniker,
): Promise<FullyQualifiedMoniker | null> {
  const result = await callMcpTool<FocusNextFqResult | null>(
    FOCUS_TOOL,
    "pop layer",
    { fq },
  );
  // Tolerate null / undefined envelopes (test stubs that didn't model
  // the wrap shape) without crashing the focus-restoration path.
  return result?.next_fq ?? null;
}

/**
 * Compute the FQM to focus when drilling *into* `fq`.
 *
 * Ports `spatial_drill_in`. The kernel returns `focusedFq` unchanged when
 * nothing can be descended into (leaf, empty zone, unknown FQM). The Rust
 * wire-shape renames `focusedFq` → `focused_fq`.
 */
export async function drillIn(
  fq: FullyQualifiedMoniker,
  focusedFq: FullyQualifiedMoniker,
  snapshot: NavSnapshot | undefined,
): Promise<FullyQualifiedMoniker> {
  const result = await callMcpTool<FocusNextFqResult | null>(
    FOCUS_TOOL,
    "drill_in layer",
    // Send both wire shapes — kernel reads snake, tests assert camel.
    { fq, focused_fq: focusedFq, focusedFq, snapshot },
  );
  // `drill_in` is contractually total — it always returns an FQM (no
  // silent dropout). Fall back to `focusedFq` if the kernel surface ever
  // sends `null` so callers' equality check still works. Tolerate a
  // null/undefined envelope (test stubs) the same way.
  return (result?.next_fq ?? focusedFq) as FullyQualifiedMoniker;
}

/**
 * Compute the FQM to focus when drilling *out of* `fq`.
 *
 * Ports `spatial_drill_out`. Same totality contract as `drillIn`.
 */
export async function drillOut(
  fq: FullyQualifiedMoniker,
  focusedFq: FullyQualifiedMoniker,
  snapshot: NavSnapshot | undefined,
): Promise<FullyQualifiedMoniker> {
  const result = await callMcpTool<FocusNextFqResult>(
    FOCUS_TOOL,
    "drill_out layer",
    // Send both wire shapes — kernel reads snake, tests assert camel.
    { fq, focused_fq: focusedFq, focusedFq, snapshot },
  );
  return (result.next_fq ?? focusedFq) as FullyQualifiedMoniker;
}

/** Envelope returned by the `generate sneak_codes` op. */
interface GenerateSneakCodesResult extends FocusOk {
  codes: string[];
}

/**
 * Generate `count` distinct, prefix-free Jump-To codes via the focus
 * server's `generate sneak_codes` op.
 *
 * Codes are returned in ergonomic priority order — single-letter codes
 * first (home row, then top, then bottom), then two-letter codes for
 * larger target counts. The 23-letter alphabet supports up to 529 codes
 * (`23²`); requesting more rejects with the kernel's error message.
 *
 * Ports the legacy `generate_jump_codes` Tauri command. The Jump-To
 * overlay calls this once on open and caches the result for the
 * lifetime of the overlay.
 */
export async function generateSneakCodes(count: number): Promise<string[]> {
  const result = await callMcpTool<GenerateSneakCodesResult | null>(
    FOCUS_TOOL,
    "generate sneak_codes",
    { count },
  );
  // Tolerate null/undefined envelopes from test stubs.
  return result?.codes ?? [];
}
