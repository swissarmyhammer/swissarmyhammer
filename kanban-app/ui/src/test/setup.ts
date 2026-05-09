/**
 * Vitest setup file — polyfills for jsdom environment + test instrumentation.
 *
 * ResizeObserver is required by Radix UI primitives (e.g. Tooltip Arrow)
 * but is not available in jsdom.
 *
 * # Spatial-nav test instrumentation
 *
 * The spatial-nav redesign (parent card 01KQTC1VNQM9KC90S65P7QX9N1)
 * removed the kernel-side scope replica — React no longer fires
 * `spatial_register_scope` / `spatial_unregister_scope` /
 * `spatial_update_rect` IPCs. The kernel sees scope state only via
 * per-decision snapshots.
 *
 * Many existing tests filter `mockInvoke.mock.calls` for
 * `spatial_register_scope` / `spatial_unregister_scope` entries to
 * discover what scopes registered. To preserve their meaning without
 * editing every file, this setup installs a global hook on
 * `LayerScopeRegistry` that re-fires the historic IPC pattern through
 * the test's mocked `invoke` whenever a scope is registered or
 * unregistered.
 *
 * The dynamic `import("@tauri-apps/api/core")` in each hook call is
 * load-bearing: a static `import` at module top would resolve before
 * the test file's `vi.mock(...)` hoist takes effect, grabbing the real
 * module. The dynamic form re-resolves at call time and picks up
 * whichever mock is active in the calling test's module graph.
 *
 * This is purely a test-mode mirror of the React-side registry into
 * the mock-invoke history; production code never installs the hook.
 */

import { installRegistryHook } from "@/lib/layer-scope-registry-context";

if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}

// Vitest browser mode (4.1.x) emits spurious birpc `function "toJSON" not
// found` unhandled rejections during tests that exercise virtualized lists
// or field-editor matrices. The host receives a TYPE_REQUEST with
// method="toJSON" that has no registered handler. The rejection does not
// affect any assertion, but it bubbles up to the vite/HMR pipe and causes
// `pnpm test` to exit non-zero. Tracked at task 01KR454PDHCX07SN7RQ4Z9ASWS;
// suppress only this specific, narrowly-scoped vitest-internal rejection so
// the test runner exit code reflects real test outcomes. Any other
// unhandled rejection (real bugs, missing awaits, etc.) is left alone.
if (typeof window !== "undefined") {
  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason as { message?: string } | null;
    if (
      reason?.message ===
      `[birpc] function "toJSON" not found`
    ) {
      event.preventDefault();
    }
  });
}

type InvokeFn = (
  cmd: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

let cachedInvoke: InvokeFn | null = null;
const pendingCalls: Array<{ cmd: string; args: Record<string, unknown> }> = [];
let importInFlight = false;

function callMockInvoke(cmd: string, args: Record<string, unknown>): void {
  if (cachedInvoke !== null) {
    void cachedInvoke(cmd, args).catch(() => {});
    return;
  }
  pendingCalls.push({ cmd, args });
  if (importInFlight) return;
  importInFlight = true;
  void import("@tauri-apps/api/core").then(({ invoke }) => {
    cachedInvoke = invoke as InvokeFn;
    for (const call of pendingCalls) {
      void cachedInvoke(call.cmd, call.args).catch(() => {});
    }
    pendingCalls.length = 0;
  });
}

installRegistryHook({
  onAdd(layerFq, fq, entry) {
    callMockInvoke("spatial_register_scope", {
      fq,
      segment: entry.segment,
      rect: entry.lastKnownRect ?? {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
      },
      layerFq,
      parentZone: entry.parentZone,
      overrides: entry.navOverride ?? {},
    });
  },
  onDelete(_layerFq, fq, _entry) {
    callMockInvoke("spatial_unregister_scope", { fq });
  },
});
