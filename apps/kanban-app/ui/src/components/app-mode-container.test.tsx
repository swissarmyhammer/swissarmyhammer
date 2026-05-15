import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mocks — Tauri APIs must be mocked before importing components.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

// Import after mocks
import { AppModeContainer, useAppMode } from "./app-mode-container";
import { CommandScopeContext } from "@/lib/command-scope";
import { useContext } from "react";

// ---------------------------------------------------------------------------
// Probes
// ---------------------------------------------------------------------------

/** Reads the mode from AppModeContainer context. */
function ModeProbe() {
  const { mode } = useAppMode();
  return <span data-testid="mode">{mode}</span>;
}

/** Reads the scope chain to verify the mode moniker is present. */
function ScopeChainProbe() {
  const scope = useContext(CommandScopeContext);
  const chain: string[] = [];
  let cur: typeof scope = scope;
  while (cur) {
    if (cur.moniker) chain.push(cur.moniker);
    cur = cur.parent;
  }
  return <span data-testid="scope-chain">{chain.join(",")}</span>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("AppModeContainer", () => {
  it("renders children", () => {
    render(
      <AppModeContainer>
        <span data-testid="child">hello</span>
      </AppModeContainer>,
    );
    expect(screen.getByTestId("child").textContent).toBe("hello");
  });

  it("provides mode context defaulting to normal", () => {
    render(
      <AppModeContainer>
        <ModeProbe />
      </AppModeContainer>,
    );
    expect(screen.getByTestId("mode").textContent).toBe("normal");
  });

  it("provides a CommandScopeProvider with mode moniker", () => {
    render(
      <AppModeContainer>
        <ScopeChainProbe />
      </AppModeContainer>,
    );
    const chain = screen.getByTestId("scope-chain").textContent ?? "";
    expect(chain).toContain("mode:normal");
  });
});
