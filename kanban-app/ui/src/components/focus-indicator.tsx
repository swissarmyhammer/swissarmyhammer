/**
 * `<FocusIndicator>` — the single visible focus decorator.
 *
 * Renders a 1px dotted border in the design-system primary color *inside*
 * the host's box when `focused` is true; renders nothing when `focused`
 * is false. The host primitive must establish a containing block (the
 * spatial primitives mark themselves `position: relative` so this works
 * without consumer effort).
 *
 * The decoration sits at `absolute inset-0` so it traces the host's
 * bounding box exactly — no left padding, no gap-to-the-left coupling,
 * no offsets that fall outside an `overflow: hidden` ancestor and get
 * clipped. `rounded-[inherit]` lets the indicator follow the host's
 * corner radius (e.g. `rounded-md` on cards). `pointer-events-none`
 * keeps clicks flowing through to the host.
 *
 * This component is the ONE PLACE the focus visual lives. CSS no longer
 * reads `[data-focused]` to draw a border — focus state flows from Rust
 * → `useFocusClaim` → React state → this component's `focused` prop →
 * className → visible decoration. The `data-focused` attribute remains
 * on the primitive's div as an output-only debugging / e2e hook;
 * nothing reads it back as state.
 *
 * Single source of truth: a regression that adds a second focus
 * decorator (a copy of the indicator elsewhere, a CSS rule reading
 * `[data-focused]`, a parallel "ring" / "outline" variant) is caught by
 * the source-level guards in `focus-architecture.guards.node.test.ts`.
 */
import { memo } from "react";

interface FocusIndicatorProps {
  /**
   * Whether the host primitive is currently focused.
   *
   * Driven by `useFocusClaim` on the primitive — true while the host's
   * `SpatialKey` is the focused key for its window, false otherwise.
   */
  focused: boolean;
}

/**
 * Renders the visible focus decoration when `focused` is true; renders
 * nothing otherwise. The decoration is `pointer-events-none` so it never
 * intercepts a click; it's `aria-hidden` so screen readers don't announce
 * a duplicate focus signal.
 */
export const FocusIndicator = memo(function FocusIndicator({
  focused,
}: FocusIndicatorProps) {
  if (!focused) return null;
  return (
    <span
      data-testid="focus-indicator"
      aria-hidden="true"
      className="pointer-events-none absolute inset-0 border border-dotted border-primary rounded-[inherit]"
    />
  );
});
