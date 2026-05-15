/**
 * Debounced async search wrapper.
 *
 * Delays invocation by `delayMs` and cancels any previous in-flight request
 * when a new query arrives. This prevents redundant backend calls during
 * rapid typing.
 */

/** Options for creating a debounced search function. */
export interface DebouncedSearchOptions<T> {
  /** The underlying async search to debounce. */
  search: (query: string) => Promise<T[]>;
  /** Debounce delay in milliseconds. */
  delayMs: number;
}

/**
 * Create a debounced version of an async search function.
 *
 * Returns a function with the same signature that:
 * - Waits `delayMs` before invoking the underlying search
 * - Cancels any pending timer when called again
 * - Discards stale results (only the latest call's result is returned)
 *
 * @returns A debounced search function and a cancel handle.
 */
export function createDebouncedSearch<T>(
  opts: DebouncedSearchOptions<T>,
): (query: string) => Promise<T[]> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let generation = 0;
  let pendingResolve: ((v: T[]) => void) | null = null;

  return (query: string): Promise<T[]> => {
    // Cancel any pending debounce timer and resolve previous promise as empty
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
      if (pendingResolve) {
        pendingResolve([]);
        pendingResolve = null;
      }
    }

    const thisGeneration = ++generation;

    return new Promise((resolve) => {
      pendingResolve = resolve;
      timer = setTimeout(async () => {
        timer = null;
        pendingResolve = null;
        try {
          const results = await opts.search(query);
          // Only resolve if this is still the latest request
          if (thisGeneration === generation) {
            resolve(results);
          } else {
            resolve([]);
          }
        } catch {
          resolve([]);
        }
      }, opts.delayMs);
    });
  };
}
