import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useContext } from "react";
import { CommandScopeContext, scopeChainFromScope } from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

// `EntityFocusProvider` now imports the optional spatial-focus actions
// hook, which pulls in `spatial-focus-context.tsx`, which uses
// `@tauri-apps/api/event`. That module re-exports `transformCallback`
// from `core` — so when `core` is mocked with just `invoke`, the real
// `event` module fails to import. Mock `event` here too so this test
// stays self-contained and never reaches the real Tauri runtime.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

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

  it("renders no DOM wrapper — CommandScopeProvider is context-only", () => {
    const { container } = render(
      <EntityFocusProvider>
        <StoreContainer path="/board">
          <span data-testid="child">hello</span>
        </StoreContainer>
      </EntityFocusProvider>,
    );

    // StoreContainer is a pure context provider (CommandScopeProvider): it
    // contributes a moniker to the scope chain but emits no DOM of its own,
    // so no element should carry a `data-moniker` attribute.
    expect(container.querySelector("[data-segment]")).toBeNull();
  });
});
