/**
 * `FullyQualifiedMonikerContext` — carries the canonical path through
 * the focus hierarchy down the React tree.
 *
 * Every spatial primitive (`<FocusLayer>` / `<FocusZone>` / `<FocusScope>`)
 * provides this context to its descendants with its own composed
 * fully-qualified moniker. Descendants that mount their own primitive
 * read the parent FQM, append their declared `SegmentMoniker`, and
 * provide the resulting FQM downward in turn.
 *
 * The context value is `null` outside any primitive — `useFullyQualifiedMoniker`
 * treats that as a hard error since every spatial primitive must live
 * inside one (or be a layer root, in which case the consumer composes
 * the root via `fqRoot`). The companion `useOptionalFullyQualifiedMoniker`
 * tolerates a missing ancestor for tests that mount one component at a
 * time.
 *
 * # Why this is a separate file
 *
 * The context is read by all three primitives plus `<Inspectable>` and
 * the entity-focus bridge. Putting it in its own module keeps the
 * primitive files free of cyclic imports and lets non-primitive
 * consumers (e.g. table-row escape hatches) read the FQM without
 * reaching into a primitive's namespace.
 */

import { createContext, useContext } from "react";
import type { FullyQualifiedMoniker } from "@/types/spatial";

/**
 * The branded `FullyQualifiedMoniker` of the nearest ancestor spatial
 * primitive, or `null` outside any primitive.
 *
 * Defaults to `null` so consumers that throw on absence can use
 * `if (!fq) throw …` without an `undefined` branch.
 */
export const FullyQualifiedMonikerContext =
  createContext<FullyQualifiedMoniker | null>(null);

/**
 * Read the FQM of the enclosing spatial primitive.
 *
 * Throws when called outside any primitive — every consumer of this
 * hook must have a `<FocusLayer>` / `<FocusZone>` / `<FocusScope>`
 * ancestor that pushed a value into the context. Use the optional
 * variant when a no-primitive degraded path is genuinely needed (e.g.
 * a unit test that mounts a single component without the spatial
 * provider stack).
 */
export function useFullyQualifiedMoniker(): FullyQualifiedMoniker {
  const fq = useContext(FullyQualifiedMonikerContext);
  if (fq === null) {
    throw new Error(
      "useFullyQualifiedMoniker must be called inside a <FocusLayer>, <FocusZone>, or <FocusScope>",
    );
  }
  return fq;
}

/**
 * Read the FQM of the enclosing spatial primitive, or `null` when none.
 *
 * Use from primitives or test-friendly consumers that should silently
 * degrade outside the spatial-nav stack. Production trees always wrap
 * everything in a window-root layer, so the strict variant is the
 * right choice for production-only call sites.
 */
export function useOptionalFullyQualifiedMoniker(): FullyQualifiedMoniker | null {
  return useContext(FullyQualifiedMonikerContext);
}
