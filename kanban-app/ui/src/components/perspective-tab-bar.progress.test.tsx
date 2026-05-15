/**
 * Progress-bar coverage for the `perspective.switch` round-trip.
 *
 * Pinned for 01KP3ERHEDP86C2JYYR7NM1593: when the user clicks an inactive
 * perspective tab, the dispatch is the real filter work (server-side), so
 * the indeterminate progress bar in the nav bar must appear for the
 * duration of the round-trip — `CommandBusyProvider.inflightCount`
 * increments on dispatch and decrements on settle. The legacy
 * `perspective.set` command did not exercise this path (it had no
 * meaningful backend work), so this regression guard was deliberately
 * absent before the switch migration.
 *
 * Test shape: mount a synthetic tab that clicks through the production
 * `useDispatchCommand("perspective.switch")` wire path. Mock
 * `dispatch_command` to pend, then assert the progress bar appears.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Mock Tauri APIs before importing any modules that use them.
const mockInvoke = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (..._args: any[]): Promise<unknown> => Promise.resolve(null),
);
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

import {
  CommandBusyProvider,
  useCommandBusy,
  CommandScopeProvider,
  ActiveBoardPathProvider,
  useDispatchCommand,
} from "@/lib/command-scope";

/**
 * Probe that emits `role="progressbar"` while the inflight counter is
 * non-zero — mirrors the production wiring in `NavBar.tsx`.
 */
function BusyProbe() {
  const busy = useCommandBusy();
  if (!busy.isBusy) return null;
  return <div role="progressbar" aria-label="Command in progress" />;
}

/**
 * Stand-in for the production `PerspectiveTab` click handler.
 *
 * The real `ScopedPerspectiveTab` (in `perspective-tab-bar.tsx`) binds
 * `useDispatchCommand("perspective.switch")` and invokes it on click —
 * exactly what this synthetic button does. Using a synthetic tab keeps
 * this test focused on the busy-counter contract instead of mounting
 * the full spatial + focus + entity provider stack.
 */
function PerspectiveSwitchButton({ id }: { id: string }) {
  const dispatch = useDispatchCommand("perspective.switch");
  return (
    <button
      type="button"
      onClick={() => {
        void dispatch({ args: { perspective_id: id } });
      }}
    >
      Switch to {id}
    </button>
  );
}

function renderWithBusy() {
  return render(
    <CommandBusyProvider>
      <CommandScopeProvider commands={[]} moniker="window:main">
        <ActiveBoardPathProvider value="/tmp/test/.kanban">
          <BusyProbe />
          <PerspectiveSwitchButton id="p2" />
        </ActiveBoardPathProvider>
      </CommandScopeProvider>
    </CommandBusyProvider>,
  );
}

describe("PerspectiveTab click — progress bar during perspective.switch", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows role=progressbar while the dispatch is pending", async () => {
    // Make `dispatch_command` pend so we can observe the busy state
    // mid-flight. Resolve the dispatch only after asserting the bar
    // is visible.
    let resolveDispatch: ((value: unknown) => void) | null = null;
    const pendingDispatch = new Promise((resolve) => {
      resolveDispatch = resolve;
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "dispatch_command") return pendingDispatch;
      return Promise.resolve(null);
    });

    renderWithBusy();

    // Pre-click: no progress bar.
    expect(screen.queryByRole("progressbar")).toBeNull();

    // Click triggers `perspective.switch` dispatch via the real
    // `useDispatchCommand` wrapper, which increments the inflight
    // counter synchronously inside `useState` setter.
    await act(async () => {
      fireEvent.click(screen.getByText("Switch to p2"));
      // Let React flush the state update from the dispatch's
      // synchronous `setInflightCount((c) => c + 1)` call.
      await Promise.resolve();
    });

    // The bar must be visible while the dispatch is pending.
    expect(screen.getByRole("progressbar")).toBeTruthy();
    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.switch",
        args: { perspective_id: "p2" },
      }),
    );

    // Resolve the dispatch — counter decrements, bar disappears.
    await act(async () => {
      resolveDispatch?.(null);
      await pendingDispatch;
    });

    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("removes the progress bar even when the dispatch rejects", async () => {
    // Rejection path: the wrapper must still decrement on settle, so a
    // failed `perspective.switch` does not leave the bar stuck on.
    let rejectDispatch: ((e: Error) => void) | null = null;
    const pendingDispatch = new Promise((_resolve, reject) => {
      rejectDispatch = reject;
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "dispatch_command") return pendingDispatch;
      return Promise.resolve(null);
    });

    renderWithBusy();

    await act(async () => {
      fireEvent.click(screen.getByText("Switch to p2"));
      await Promise.resolve();
    });
    expect(screen.getByRole("progressbar")).toBeTruthy();

    await act(async () => {
      rejectDispatch?.(new Error("boom"));
      await pendingDispatch.catch(() => undefined);
    });

    expect(screen.queryByRole("progressbar")).toBeNull();
  });
});
