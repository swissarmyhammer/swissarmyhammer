/**
 * Browser tests pinning the `nav.focus` command — the single auditable
 * focus-claim choke point introduced by card
 * `01KR7CDEFWWVF4WH0BCHE8Y21J`.
 *
 * Four guarantees (Card G single-source shape):
 *
 *   1. `nav.focus` is DEFINED only by the `nav-commands` builtin plugin;
 *      the webview's execution leg is a webview-bus handler registered by
 *      `<SpatialFocusProvider>` (`registerWebviewCommandHandler`), NOT a
 *      scope-chain `CommandDef`.
 *   2. Dispatching `nav.focus({ args: { fq } })` from a descendant
 *      claims focus on the kernel via `spatial_focus(fq, snapshot)` —
 *      and the commit CARRIES the snapshot (the kernel silently drops a
 *      snapshot-less commit, so losing the snapshot breaks click-to-focus
 *      everywhere).
 *   3. `<FocusScope>`'s click handler dispatches `nav.focus` (not
 *      `setFocus(fq)` or `spatial.focus(fq)` directly), so every
 *      focus claim flows through that one closure.
 *   4. The bus handler is live whenever `<SpatialFocusProvider>` is
 *      mounted — spatial-only harnesses (no `<EntityFocusProvider>`)
 *      keep clicks routed through `spatial_focus`.
 *
 * The source-level guarantee — that no production component file calls
 * `setFocus(<non-null>)` directly — runs as a static text scan in
 * `nav-focus.source-guard.node.test.ts`; the no-CommandDef guarantee is
 * `inspect-and-focus-commands.plugin-owned.node.test.ts`.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API spies. Mirrors the pattern used by every other
// `*.browser.test.tsx` file in this repo so command dispatches and
// kernel IPC calls are observable as `mockInvoke.mock.calls`.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen, listeners } = await vi.hoisted(async () => {
  const { setupSpatialMocks } = await import("@/test/spatial-nav-harness");
  return setupSpatialMocks();
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports — after the mocks above have hoisted into module init order.
// ---------------------------------------------------------------------------

import { FocusScope } from "@/components/focus-scope";
import { FocusLayer } from "@/components/focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { useDispatchCommand } from "@/lib/command-scope";
import {
  hasWebviewCommandHandler,
  resetWebviewCommandBusForTest,
} from "@/lib/webview-command-bus";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WINDOW_LAYER_NAME = asSegment("window");

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flush() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Collect every `spatial_focus` invocation, in order. */
function spatialFocusCalls() {
  return mockInvoke.mock.calls
    .filter(
      (c) =>
        c[0] === "spatial_focus" ||
        (c[0] === "command_tool_call" &&
          (c[1] as any)?.tool === "focus" &&
          (c[1] as any)?.op === "set focus"),
    )
    .map((c) => {
      const outer = c[1] as Record<string, unknown>;
      const args = (outer?.params ?? outer) as {
        fq: string;
        snapshot?: { layer_fq?: string; scopes?: Array<{ fq?: string }> };
        window?: string;
      };
      return args;
    });
}

/** Collect every `dispatch_command` invocation, in order. */
function backendDispatchCalls() {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as { cmd: string });
}

/**
 * Render `<FocusScope>` inside the production-shaped provider stack
 * (Spatial > Entity > Layer). The spatial provider's `nav.focus`
 * webview-bus handler is exactly what production would mount.
 */
function renderProductionShape(child: React.ReactElement) {
  return render(
    <SpatialFocusProvider>
      <EntityFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>{child}</FocusLayer>
      </EntityFocusProvider>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("nav.focus command", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    resetWebviewCommandBusForTest();
  });

  afterEach(() => {
    vi.clearAllMocks();
    resetWebviewCommandBusForTest();
  });

  it("mounting <SpatialFocusProvider> registers the nav.focus webview-bus handler (single execution source)", async () => {
    // Card G: `nav.focus` is DEFINED once, in the `nav-commands` plugin.
    // The webview's execution leg is a bus handler registered by
    // `<SpatialFocusProvider>` — not a scope-chain `CommandDef` (those two
    // context-file definitions were the duplication this card removed).
    expect(hasWebviewCommandHandler("nav.focus")).toBe(false);

    const { unmount } = render(
      <SpatialFocusProvider>
        <span />
      </SpatialFocusProvider>,
    );
    await flush();

    expect(
      hasWebviewCommandHandler("nav.focus"),
      "SpatialFocusProvider must register the nav.focus webview-bus handler",
    ).toBe(true);

    unmount();
    expect(
      hasWebviewCommandHandler("nav.focus"),
      "unmount must clear the nav.focus bus handler (ownership-guarded cleanup)",
    ).toBe(false);
  });

  it("dispatching nav.focus({ args: { fq } }) calls spatial_focus(fq) on the kernel", async () => {
    let dispatcherRef:
      | ((
          cmd: string,
          opts?: { args?: Record<string, unknown> },
        ) => Promise<unknown>)
      | null = null;

    function DispatchProbe() {
      // Read the dispatcher from inside the provider stack so it
      // resolves the `nav.focus` registration the providers emit.
      const dispatch = useDispatchCommand();
      dispatcherRef = dispatch as typeof dispatcherRef;
      return null;
    }

    renderProductionShape(<DispatchProbe />);
    await flush();

    expect(dispatcherRef).not.toBeNull();
    mockInvoke.mockClear();

    await act(async () => {
      await dispatcherRef!("nav.focus", { args: { fq: "/window/foo" } });
    });

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe("/window/foo");

    // The frontend execute closure must claim the dispatch — no
    // backend `dispatch_command` IPC for `nav.focus`.
    const backendCalls = backendDispatchCalls().filter(
      (c) => c.cmd === "nav.focus",
    );
    expect(backendCalls).toHaveLength(0);
  });

  it("clicking a <FocusScope> dispatches nav.focus, which fires spatial_focus(fq) — not a backend dispatch_command", async () => {
    const { container } = renderProductionShape(
      <FocusScope moniker={asSegment("ui:nav-focus-test")}>
        <span data-testid="probe">click me</span>
      </FocusScope>,
    );
    await flush();

    const node = container.querySelector(
      "[data-segment='ui:nav-focus-test']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    mockInvoke.mockClear();

    fireEvent.click(node!);
    await flush();

    // The click must reach the kernel via `spatial_focus` — the
    // webview-bus handler for `nav.focus` runs client-side and calls into
    // the spatial provider, which dispatches `spatial_focus` IPC.
    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe("/window/ui:nav-focus-test");

    // LOAD-BEARING: the commit must CARRY the geometry snapshot. The
    // kernel silently drops a snapshot-less commit (its transient-unmount
    // contract), so a consolidation that routed clicks to the host-side
    // plugin execute (which has no geometry) would make every
    // click-to-focus a silent no-op. The snapshot names the owning layer
    // and includes the clicked scope.
    const snapshot = focusCalls[0].snapshot;
    expect(
      snapshot,
      "click-to-focus must commit WITH a snapshot — a snapshot-less commit is dropped by the kernel",
    ).toBeTruthy();
    expect(snapshot!.layer_fq).toBe("/window");
    expect(
      (snapshot!.scopes ?? []).some(
        (s) => s.fq === "/window/ui:nav-focus-test",
      ),
      "the snapshot must include the clicked scope",
    ).toBe(true);

    // No backend `dispatch_command` IPC for `nav.focus` — that
    // would mean the webview handler was never registered, or that
    // someone bypassed the focus subsystem and reached the IPC layer
    // directly.
    const backendCalls = backendDispatchCalls().filter(
      (c) => c.cmd === "nav.focus",
    );
    expect(backendCalls).toHaveLength(0);
  });

  it("nav.focus also resolves with only <SpatialFocusProvider> mounted (no EntityFocusProvider)", async () => {
    // Tests like `nav-bar.spatial-nav.test.tsx` mount only the
    // spatial provider. The spatial-level `nav.focus` registration
    // must keep clicks routed through `spatial_focus`.
    const { container } = render(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <FocusScope moniker={asSegment("ui:spatial-only-leaf")}>
            <span>x</span>
          </FocusScope>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flush();

    const node = container.querySelector(
      "[data-segment='ui:spatial-only-leaf']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();

    mockInvoke.mockClear();

    fireEvent.click(node!);
    await flush();

    const focusCalls = spatialFocusCalls();
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0].fq).toBe("/window/ui:spatial-only-leaf");
  });
});

// The source-level guard for "no direct setFocus(<non-null>) outside the
// allowlist" lives in the companion node-mode test
// `nav-focus.source-guard.node.test.ts` — `node:fs` and `process.cwd()`
// are not available in browser-mode test environments.
