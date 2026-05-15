import { useState, useCallback, useMemo } from "react";

export type InspectorMode = "normal" | "edit";

export interface UseInspectorNavReturn {
  mode: InspectorMode;
  enterEdit: () => void;
  exitEdit: () => void;
}

/**
 * Hook for managing inspector edit mode.
 *
 * Field navigation is handled by the spatial-nav graph: each field row
 * registers as a `<FocusZone>` and beam-search rules 1 and 2
 * drive within-field and cross-field movement. This hook only manages the
 * normal/edit mode toggle.
 *
 * @returns Inspector mode state and control functions
 */
export function useInspectorNav(): UseInspectorNavReturn {
  const [mode, setMode] = useState<InspectorMode>("normal");

  /** Enter edit mode. */
  const enterEdit = useCallback(() => setMode("edit"), []);

  /** Exit edit mode, returning to normal mode. */
  const exitEdit = useCallback(() => setMode("normal"), []);

  return useMemo(
    () => ({ mode, enterEdit, exitEdit }),
    [mode, enterEdit, exitEdit],
  );
}
