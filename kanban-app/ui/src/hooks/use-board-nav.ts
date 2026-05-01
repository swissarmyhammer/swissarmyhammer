import { useState, useCallback, useMemo } from "react";

export type BoardMode = "normal" | "edit";

export interface UseBoardNavReturn {
  mode: BoardMode;
  enterEdit: () => void;
  exitEdit: () => void;
}

/**
 * Hook for managing board interaction mode (normal vs edit).
 *
 * Navigation itself is no longer driven from React. The Rust spatial-nav
 * kernel owns cursor movement; consumers invoke it via
 * `useSpatialFocusActions().navigate`, and per-direction directives (when
 * needed) are expressed as `navOverride` props on `<FocusScope>` /
 * `<FocusZone>`. This hook only tracks the mode
 * (normal/edit) for controlling field editing behaviour.
 *
 * @returns Board mode state and control functions
 */
export function useBoardNav(): UseBoardNavReturn {
  const [mode, setMode] = useState<BoardMode>("normal");

  /** Enter edit mode. */
  const enterEdit = useCallback(() => {
    setMode("edit");
  }, []);

  /** Exit edit mode, returning to normal mode. */
  const exitEdit = useCallback(() => {
    setMode("normal");
  }, []);

  return useMemo(
    () => ({
      mode,
      enterEdit,
      exitEdit,
    }),
    [mode, enterEdit, exitEdit],
  );
}
