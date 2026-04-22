/**
 * Regression fence: prove that `nav.*` keypresses flow through the
 * real `dispatch_command` pipeline, not an in-JS side channel.
 *
 * ## Why this test exists
 *
 * Before the cleanup of card `01KPTV64JF4QJE6GQK3DS0TK41`, the suite's
 * fixtures could re-implement nav commands with a local
 * `execute: () => broadcastNavCommand(id)` that short-circuited the
 * dispatcher. As long as the fixture kept that shape, every keyboard
 * test passed even when production had no dispatch-to-Rust path for
 * `nav.*`. This file locks down the contract: pressing `j` must emit
 * a literal `invoke("dispatch_command", { cmd: "nav.down" })` call —
 * if anyone ever re-adds a JS-side broadcaster, that call will not
 * fire and this test will fail.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-react";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusScope, useFocusScopeElementRef } from "@/components/focus-scope";
import { AppShell } from "@/components/app-shell";
import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";

// Wire the Tauri API mocks to the boundary stub.
vi.mock("@tauri-apps/api/core", async () => {
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  return tauriCoreMock();
});
vi.mock("@tauri-apps/api/event", async () => {
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return tauriEventMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});
// Stub modules the real AppShell transitively imports. These mocks are
// intentionally minimal — the test exercises the nav-dispatch path,
// nothing else.
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({
    keymap_mode: "vim",
    windows: {
      main: { palette_open: false, palette_mode: "command" },
    },
  }),
  UIStateProvider: ({ children }: { children: React.ReactNode }) => children,
}));
vi.mock("@/lib/app-mode-context", () => ({
  useAppMode: () => ({ setMode: () => {} }),
}));
vi.mock("@/components/command-palette", () => ({
  CommandPalette: () => null,
}));
vi.mock("@/components/perspective-tab-bar", () => ({
  triggerStartRename: () => {},
}));

/** Simple FocusScope that stamps its rect via the spatial element ref. */
function Card({ id, top, left }: { id: string; top: number; left: number }) {
  const ref = useFocusScopeElementRef();
  return (
    <FocusScope moniker={`task:${id}`} commands={[]}>
      <div
        ref={ref as React.RefObject<HTMLDivElement>}
        data-testid={`card-${id}`}
        style={{
          position: "absolute",
          top,
          left,
          width: 80,
          height: 40,
          background: "#eee",
        }}
      >
        {id}
      </div>
    </FocusScope>
  );
}

function Fixture() {
  return (
    <EntityFocusProvider>
      <AppShell>
        <div style={{ position: "relative", width: 400, height: 300 }}>
          <Card id="a" top={0} left={0} />
          <Card id="b" top={80} left={0} />
          <Card id="c" top={160} left={0} />
        </div>
      </AppShell>
    </EntityFocusProvider>
  );
}

describe("spatial nav routes through dispatch_command to Rust", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  /**
   * The regression-blocker: pressing `j` (vim "down") must result in
   * `invoke("dispatch_command", { cmd: "nav.down" })`. No bypass, no
   * JS-side broadcaster, no local `execute` handler short-circuiting
   * the dispatcher. If this call is missing, the broadcastNavCommand
   * side-channel has been re-introduced somewhere.
   */
  it("pressing j dispatches nav.down through dispatch_command", async () => {
    const screen = await render(<Fixture />);

    // Click a card to seed focus so there's a real source for the nav.
    await userEvent.click(screen.getByTestId("card-a").element());

    await userEvent.keyboard("j");

    // Wait for the dispatch to land in the stub's invoke log.
    await vi.waitFor(() => {
      const dispatched = handles.dispatchedCommands();
      expect(dispatched.some((e) => e.cmd === "nav.down")).toBe(true);
    });
  });

  it("pressing k dispatches nav.up through dispatch_command", async () => {
    const screen = await render(<Fixture />);
    await userEvent.click(screen.getByTestId("card-b").element());
    await userEvent.keyboard("k");
    await vi.waitFor(() => {
      const dispatched = handles.dispatchedCommands();
      expect(dispatched.some((e) => e.cmd === "nav.up")).toBe(true);
    });
  });

  it("pressing h dispatches nav.left through dispatch_command", async () => {
    const screen = await render(<Fixture />);
    await userEvent.click(screen.getByTestId("card-b").element());
    await userEvent.keyboard("h");
    await vi.waitFor(() => {
      const dispatched = handles.dispatchedCommands();
      expect(dispatched.some((e) => e.cmd === "nav.left")).toBe(true);
    });
  });

  it("pressing l dispatches nav.right through dispatch_command", async () => {
    const screen = await render(<Fixture />);
    await userEvent.click(screen.getByTestId("card-b").element());
    await userEvent.keyboard("l");
    await vi.waitFor(() => {
      const dispatched = handles.dispatchedCommands();
      expect(dispatched.some((e) => e.cmd === "nav.right")).toBe(true);
    });
  });
});
