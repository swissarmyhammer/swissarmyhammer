import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createDebouncedSearch } from "./debounced-search";

describe("createDebouncedSearch", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("delays the search call by delayMs", async () => {
    const search = vi.fn().mockResolvedValue(["a", "b"]);
    const debounced = createDebouncedSearch({ search, delayMs: 150 });

    const promise = debounced("hello");

    // Search should not have been called yet
    expect(search).not.toHaveBeenCalled();

    // Advance past the delay
    vi.advanceTimersByTime(150);

    const result = await promise;
    expect(search).toHaveBeenCalledWith("hello");
    expect(result).toEqual(["a", "b"]);
  });

  it("cancels previous call when a new query arrives before delay", async () => {
    const search = vi.fn().mockResolvedValue(["result"]);
    const debounced = createDebouncedSearch({ search, delayMs: 150 });

    // Fire two queries in quick succession
    const promise1 = debounced("hel");
    vi.advanceTimersByTime(50); // only 50ms elapsed
    const promise2 = debounced("hello");

    // Advance past the delay for the second call
    vi.advanceTimersByTime(150);

    const result2 = await promise2;

    // Only the second search should have been called
    expect(search).toHaveBeenCalledTimes(1);
    expect(search).toHaveBeenCalledWith("hello");
    expect(result2).toEqual(["result"]);

    // First promise resolves with empty (stale/cancelled)
    const result1 = await promise1;
    expect(result1).toEqual([]);
  });

  it("discards stale results when a newer query completes", async () => {
    let resolvers: Array<(v: string[]) => void> = [];
    const search = vi.fn().mockImplementation(() => {
      return new Promise<string[]>((resolve) => {
        resolvers.push(resolve);
      });
    });
    const debounced = createDebouncedSearch({ search, delayMs: 50 });

    const promise1 = debounced("a");
    vi.advanceTimersByTime(50);
    // First search is now in-flight

    const promise2 = debounced("ab");
    vi.advanceTimersByTime(50);
    // Second search is now in-flight

    // Resolve the first search after the second has started
    resolvers[0](["stale-result"]);
    // Resolve the second search
    resolvers[1](["fresh-result"]);

    const result1 = await promise1;
    const result2 = await promise2;

    // First result should be discarded (empty) since generation moved on
    expect(result1).toEqual([]);
    expect(result2).toEqual(["fresh-result"]);
  });

  it("returns empty array on search error", async () => {
    const search = vi.fn().mockRejectedValue(new Error("network error"));
    const debounced = createDebouncedSearch({ search, delayMs: 100 });

    const promise = debounced("test");
    vi.advanceTimersByTime(100);

    const result = await promise;
    expect(result).toEqual([]);
  });
});
