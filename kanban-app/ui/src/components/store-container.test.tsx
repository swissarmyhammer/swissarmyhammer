import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useContext } from "react";
import { CommandScopeContext, scopeChainFromScope } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
  transformCallback: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("ulid", () => {
  let counter = 0;
  return { ulid: vi.fn(() => "01TEST" + String(++counter).padStart(20, "0")) };
});

// Import after mocks
import { StoreContainer } from "./store-container";

/**
 * Probe component that reads the scope chain from CommandScopeContext
 * and renders it as text so tests can assert on it.
 */
function ScopeChainProbe() {
  const scope = useContext(CommandScopeContext);
  const chain = scopeChainFromScope(scope);
  return <span data-testid="scope-chain">{chain.join(" > ")}</span>;
}

describe("StoreContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("provides a store:{path} moniker in the scope chain", () => {
    render(
      <EntityFocusProvider>
        <StoreContainer path="/Users/me/.kanban">
          <ScopeChainProbe />
        </StoreContainer>
      </EntityFocusProvider>,
    );

    const chain = screen.getByTestId("scope-chain").textContent;
    expect(chain).toContain("store:/Users/me/.kanban");
  });

  it("children can read the store moniker from the scope chain", () => {
    render(
      <EntityFocusProvider>
        <StoreContainer path="/some/board/.kanban">
          <ScopeChainProbe />
        </StoreContainer>
      </EntityFocusProvider>,
    );

    const chain = screen.getByTestId("scope-chain").textContent!;
    expect(chain).toMatch(/^store:\/some\/board\/\.kanban/);
  });

  it("renders children", () => {
    render(
      <EntityFocusProvider>
        <StoreContainer path="/board">
          <span data-testid="child">hello</span>
        </StoreContainer>
      </EntityFocusProvider>,
    );

    expect(screen.getByTestId("child").textContent).toBe("hello");
  });

  it("does not render a wrapping container div (renderContainer=false)", () => {
    const { container } = render(
      <EntityFocusProvider>
        <StoreContainer path="/board">
          <span data-testid="child">hello</span>
        </StoreContainer>
      </EntityFocusProvider>,
    );

    // FocusScope with renderContainer=false should not add a wrapping `<div>`.
    // The child should be directly inside the provider, not wrapped in an
    // extra div.
    expect(container.querySelector("[data-moniker]")).toBeNull();
  });
});
