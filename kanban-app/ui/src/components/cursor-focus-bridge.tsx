import { useContext, useEffect, useRef } from "react";
import { CommandScopeContext } from "@/lib/command-scope";
import { useFocusActions } from "@/lib/entity-focus-context";

/**
 * Renderless component that bridges a navigation cursor to entity focus.
 *
 * Must be rendered inside a CommandScopeProvider so it picks up the correct
 * scope (including view-specific nav commands). Uses two separate effects:
 * one for scope registration (fires on scope changes) and one for focus
 * (fires only when the moniker changes, i.e. cursor movement).
 *
 * Shared by BoardView and GridView — both need identical cursor-to-focus
 * bridging behaviour.
 */
export function CursorFocusBridge({ moniker: mk }: { moniker: string }) {
  const scope = useContext(CommandScopeContext);
  const { setFocus, registerScope, unregisterScope } = useFocusActions();
  const prevMonikerRef = useRef<string | null>(null);

  // Register scope — fires on any change to keep registry current
  useEffect(() => {
    if (scope) registerScope(mk, scope);
    return () => unregisterScope(mk);
  }, [mk, scope, registerScope, unregisterScope]);

  // Set focus only on cursor movement (moniker change), not on initial mount.
  // On mount, something else may already have focus (e.g. inspector).
  useEffect(() => {
    if (prevMonikerRef.current !== null && prevMonikerRef.current !== mk) {
      setFocus(mk);
    }
    prevMonikerRef.current = mk;
  }, [mk, setFocus]);

  return null;
}
