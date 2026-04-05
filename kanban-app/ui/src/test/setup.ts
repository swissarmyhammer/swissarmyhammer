/**
 * Vitest setup file — polyfills for jsdom environment.
 *
 * ResizeObserver is required by Radix UI primitives (e.g. Tooltip Arrow)
 * but is not available in jsdom.
 */

if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}
