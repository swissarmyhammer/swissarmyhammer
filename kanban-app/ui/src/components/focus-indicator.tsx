/**
 * `<FocusIndicator>` — the single visible focus decorator.
 *
 * Renders a small absolutely-positioned bar to the left of its host when
 * `focused` is true; renders nothing when `focused` is false. The host
 * primitive must establish a containing block (the spatial primitives mark
 * themselves `position: relative` so this works without consumer effort).
 *
 * This component is the ONE PLACE the focus visual lives. CSS no longer
 * reads `[data-focused]` to draw a bar — focus state flows from Rust →
 * `useFocusClaim` → React state → this component's `focused` prop →
 * className → visible decoration. The `data-focused` attribute remains on
 * the primitive's div as an output-only debugging / e2e hook; nothing
 * reads it back as state.
 *
 * Single source of truth: a regression that adds a second focus
 * decorator (a copy of the bar elsewhere, a CSS rule reading
 * `[data-focused]`) is caught by the source-level guard test in
 * `focus-architecture.guards.node.test.ts`.
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
 * Renders the visible focus bar when `focused` is true; renders nothing
 * otherwise. The bar is `pointer-events-none` so it never intercepts a
 * click; it's `aria-hidden` so screen readers don't announce a duplicate
 * focus signal.
 */
export const FocusIndicator = memo(function FocusIndicator({
  focused,
}: FocusIndicatorProps) {
  if (!focused) return null;
  return (
    <span
      data-testid="focus-indicator"
      aria-hidden="true"
      className="pointer-events-none absolute -left-2 top-0.5 bottom-0.5 w-1 rounded-full bg-primary shadow-sm"
    />
  );
});
