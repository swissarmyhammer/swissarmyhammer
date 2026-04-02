/**
 * Perspective filter/sort evaluation.
 *
 * Filter expressions are JS strings compiled via `new Function()` and cached
 * by expression text. Sort entries use multi-level comparison with locale-aware
 * string ordering.
 *
 * All functions are pure — no React dependencies — so they can be called from
 * `useMemo` in view components.
 */

import type { Entity } from "@/types/kanban";
import type { PerspectiveSortEntry } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Filter evaluation
// ---------------------------------------------------------------------------

/** Compiled filter function: receives entity fields as a flat object, returns boolean. */
type FilterFn = (fields: Record<string, unknown>) => boolean;

/** Cache of compiled filter functions keyed by expression string. */
const filterCache = new Map<string, FilterFn | null>();

/**
 * Wrap a plain object in a Proxy that returns `undefined` for missing keys
 * instead of letting `with()` fall through to the outer scope and throw
 * a ReferenceError.
 */
function permissiveProxy(
  obj: Record<string, unknown>,
): Record<string, unknown> {
  return new Proxy(obj, {
    has() {
      // Tell `with()` that every identifier is "in" this object so it never
      // escapes to the enclosing scope.
      return true;
    },
    get(target, prop) {
      if (typeof prop === "string") return target[prop];
      return undefined;
    },
  });
}

/**
 * Compile a JS expression string into a filter function.
 *
 * The expression runs with entity field names as local variables via
 * `with()`. A Proxy ensures missing fields resolve to `undefined` rather
 * than throwing ReferenceError. Returns null if the expression is
 * syntactically invalid.
 */
function compileFilter(expression: string): FilterFn | null {
  const cached = filterCache.get(expression);
  if (cached !== undefined) return cached;

  try {
    // Build: (fields) => { with(fields) { return (expression); } }
    // Using `with` keeps field access ergonomic: `Status === "open"` just works.
    // eslint-disable-next-line no-new-func
    const fn = new Function(
      "fields",
      `with(fields) { return (${expression}); }`,
    ) as FilterFn;
    filterCache.set(expression, fn);
    return fn;
  } catch (err) {
    console.warn(
      `[perspective-eval] Failed to compile filter expression: ${expression}`,
      err,
    );
    filterCache.set(expression, null);
    return null;
  }
}

/**
 * Filter entities using a JS expression string.
 *
 * Returns all entities if the filter is undefined, empty, fails to compile,
 * or throws at runtime. Errors are logged via console.warn.
 */
export function evaluateFilter(
  filter: string | undefined,
  entities: Entity[],
): Entity[] {
  if (!filter) return entities;

  const fn = compileFilter(filter);
  if (!fn) return entities;

  try {
    const result: Entity[] = [];
    for (const entity of entities) {
      try {
        if (fn(permissiveProxy(entity.fields))) result.push(entity);
      } catch (err) {
        // Runtime error means the expression is broken — return all entities.
        console.warn(
          `[perspective-eval] Filter expression threw at runtime: ${filter}`,
          err,
        );
        return entities;
      }
    }
    return result;
  } catch (err) {
    console.warn(
      `[perspective-eval] Filter expression threw at runtime: ${filter}`,
      err,
    );
    return entities;
  }
}

/** Clear the compiled filter cache (useful in tests). */
export function clearFilterCache(): void {
  filterCache.clear();
}

// ---------------------------------------------------------------------------
// Sort evaluation
// ---------------------------------------------------------------------------

/**
 * Compare two field values for sorting.
 *
 * Handles string (locale-aware), number, and fallback toString comparison.
 * Missing/undefined values sort before defined values.
 */
function compareValues(a: unknown, b: unknown): number {
  // Both missing — equal
  if (a == null && b == null) return 0;
  // Missing sorts before defined
  if (a == null) return -1;
  if (b == null) return 1;

  // Number comparison
  if (typeof a === "number" && typeof b === "number") {
    return a - b;
  }

  // String comparison (locale-aware)
  const sa = String(a);
  const sb = String(b);
  return sa.localeCompare(sb);
}

/**
 * Sort entities by multiple fields with asc/desc direction.
 *
 * Returns a new array — does not mutate the input. Ties on the first sort
 * entry are broken by subsequent entries.
 */
export function evaluateSort(
  sort: readonly PerspectiveSortEntry[],
  entities: Entity[],
): Entity[] {
  if (sort.length === 0) return entities;

  return [...entities].sort((a, b) => {
    for (const entry of sort) {
      const va = a.fields[entry.field];
      const vb = b.fields[entry.field];
      let cmp = compareValues(va, vb);
      if (entry.direction === "desc") cmp = -cmp;
      if (cmp !== 0) return cmp;
    }
    return 0;
  });
}
