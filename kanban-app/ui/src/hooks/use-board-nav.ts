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
 * Navigation is now handled by Rust spatial nav on each card and column
 * header FocusScope. This hook only tracks the mode
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
