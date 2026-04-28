/**
 * `FocusDebugContext` — single-flag context for the spatial-nav debug overlay.
 *
 * When the flag is `true`, every mounted `<FocusLayer>`, `<FocusZone>`, and
 * `<FocusScope>` renders a dashed-border + coordinate-label decorator
 * (`<FocusDebugOverlay>`) on top of its host box. The decorator is a
 * developer aid for diagnosing rect-staleness, conditional-remount, and
 * zero-rect bugs in the spatial-nav graph — it makes the live geometry
 * visible without a separate test harness.
 *
 * # Toggle path
 *
 * Production windows mount `<FocusDebugProvider enabled>` at the root of
 * their tree (App.tsx and the quick-capture window). When the spatial-nav
 * project lands and the overlay is no longer needed, flip
 * `enabled={false}` at those mount sites — or pull the provider entirely.
 * Both paths cause the overlay to render `null` and add zero DOM.
 *
 * # Default
 *
 * Outside any provider — for example, a unit test that renders a
 * `<FocusZone>` in isolation — the hook returns `false` and no overlay
 * renders. Tests that want the overlay must wrap their tree in
 * `<FocusDebugProvider enabled>` explicitly.
 */

import { createContext, useContext, type ReactNode } from "react";

/**
 * Internal context value: `true` when the debug overlay should render,
 * `false` (the default) when it should not.
 */
const FocusDebugContext = createContext<boolean>(false);

/**
 * Props for `<FocusDebugProvider>`.
 */
export interface FocusDebugProviderProps {
  /**
   * Whether the spatial-nav debug overlay is enabled.
   *
   * Defaults to `true` so a bare `<FocusDebugProvider>` (with no prop)
   * acts as "on". Production mount sites still pass `enabled` explicitly
   * (`<FocusDebugProvider enabled>` or `<FocusDebugProvider enabled={false}>`)
   * so the toggle is a one-line edit per window when the project is done
   * with the overlay.
   */
  enabled?: boolean;
  children: ReactNode;
}

/**
 * Wrap a subtree to control the `useFocusDebug()` flag.
 *
 * @param enabled - When `true`, descendants render the debug overlay.
 *   When `false`, they don't. Defaults to `true`.
 */
export function FocusDebugProvider({
  enabled = true,
  children,
}: FocusDebugProviderProps) {
  return (
    <FocusDebugContext.Provider value={enabled}>
      {children}
    </FocusDebugContext.Provider>
  );
}

/**
 * Read the spatial-nav debug-overlay flag.
 *
 * Returns `true` when wrapped in `<FocusDebugProvider enabled>` (or any
 * provider with `enabled !== false`), `false` when wrapped in
 * `<FocusDebugProvider enabled={false}>` or when no provider is mounted
 * at all.
 *
 * Consumers (`<FocusLayer>`, `<FocusZone>`, `<FocusScope>`) call this
 * during render and conditionally render `<FocusDebugOverlay>` based on
 * the result. Hooks must remain unconditional — render the overlay
 * element conditionally, do not wrap a hook call in an `if`.
 */
export function useFocusDebug(): boolean {
  return useContext(FocusDebugContext);
}
