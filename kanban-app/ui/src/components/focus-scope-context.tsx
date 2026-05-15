import { createContext, useContext } from "react";
import type { FullyQualifiedMoniker } from "@/types/spatial";

/**
 * Carries the FQM of the nearest ancestor entity-bound scope —
 * either a `<FocusScope>` (leaf) or a `<FocusZone>` (zone). Both push
 * this context so descendants can discover their enclosing entity surface
 * without walking the command-scope chain.
 *
 * Lives in its own module so `<FocusScope>` and `<FocusZone>` can both
 * import it without forming a circular dependency.
 */
export const FocusScopeContext = createContext<FullyQualifiedMoniker | null>(
  null,
);

/**
 * Read the FQM of the nearest ancestor `<FocusScope>` or `<FocusZone>`,
 * or `null` when no entity-bound scope wraps the caller.
 *
 * Uses React context so it skips intermediate `CommandScopeProvider`
 * pushes that aren't tied to an entity FQM.
 */
export function useParentFocusScope(): FullyQualifiedMoniker | null {
  return useContext(FocusScopeContext);
}
