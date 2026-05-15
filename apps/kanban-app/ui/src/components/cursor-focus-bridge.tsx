import { useContext, useEffect, useRef } from "react";
import { CommandScopeContext, useDispatchCommand } from "@/lib/command-scope";
import { useFocusActions } from "@/lib/entity-focus-context";
import type { FullyQualifiedMoniker } from "@/types/spatial";

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
 *
 * # Focus claim path (card `01KR7CDEFWWVF4WH0BCHE8Y21J`)
 *
 * Cursor movement → focus claim is routed through `nav.focus`, the
 * single auditable command that wraps the kernel-facing `setFocus`
 * primitive. Every focus claim in the UI flows through that one
 * closure so cross-cutting concerns (telemetry, animations,
 * scroll-on-focus) hang off it rather than off N call sites.
 */
export function CursorFocusBridge({
  moniker: fq,
}: {
  moniker: FullyQualifiedMoniker;
}) {
  const scope = useContext(CommandScopeContext);
  const { registerScope, unregisterScope } = useFocusActions();
  const dispatchNavFocus = useDispatchCommand("nav.focus");
  const prevFqRef = useRef<FullyQualifiedMoniker | null>(null);

  // Register scope — fires on any change to keep registry current
  useEffect(() => {
    if (scope) registerScope(fq, scope);
    return () => unregisterScope(fq);
  }, [fq, scope, registerScope, unregisterScope]);

  // Set focus only on cursor movement (FQM change), not on initial mount.
  // On mount, something else may already have focus (e.g. inspector).
  useEffect(() => {
    if (prevFqRef.current !== null && prevFqRef.current !== fq) {
      void dispatchNavFocus({ args: { fq } }).catch((err) =>
        console.error("[CursorFocusBridge] nav.focus dispatch failed", err),
      );
    }
    prevFqRef.current = fq;
  }, [fq, dispatchNavFocus]);

  return null;
}
