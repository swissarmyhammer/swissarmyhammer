import { describe, it, expect } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { AppModeProvider, useAppMode, type AppMode } from "./app-mode-context";
import { ModeIndicator } from "@/components/mode-indicator";

/**
 * Helper component that exposes a button to change mode,
 * so tests can exercise the context setter.
 */
function ModeChanger({ to }: { to: AppMode }) {
  const { setMode } = useAppMode();
  return (
    <button data-testid="change-mode" onClick={() => setMode(to)}>
      change
    </button>
  );
}

describe("AppModeProvider + ModeIndicator", () => {
  it("renders NORMAL mode by default", () => {
    render(
      <AppModeProvider>
        <ModeIndicator />
      </AppModeProvider>,
    );

    expect(screen.getByTestId("mode-indicator-mode").textContent).toBe(
      "-- NORMAL --",
    );
  });

  it("displays COMMAND mode when mode is changed to command", () => {
    render(
      <AppModeProvider>
        <ModeChanger to="command" />
        <ModeIndicator />
      </AppModeProvider>,
    );

    act(() => {
      screen.getByTestId("change-mode").click();
    });

    expect(screen.getByTestId("mode-indicator-mode").textContent).toBe(
      "-- COMMAND --",
    );
  });

  it("displays SEARCH mode when mode is changed to search", () => {
    render(
      <AppModeProvider>
        <ModeChanger to="search" />
        <ModeIndicator />
      </AppModeProvider>,
    );

    act(() => {
      screen.getByTestId("change-mode").click();
    });

    expect(screen.getByTestId("mode-indicator-mode").textContent).toBe(
      "-- SEARCH --",
    );
  });

  it("switches between modes correctly", () => {
    /** Renders buttons for each mode so we can cycle through them. */
    function MultiModeChanger() {
      const { setMode } = useAppMode();
      return (
        <>
          <button data-testid="to-command" onClick={() => setMode("command")}>
            command
          </button>
          <button data-testid="to-search" onClick={() => setMode("search")}>
            search
          </button>
          <button data-testid="to-normal" onClick={() => setMode("normal")}>
            normal
          </button>
        </>
      );
    }

    render(
      <AppModeProvider>
        <MultiModeChanger />
        <ModeIndicator />
      </AppModeProvider>,
    );

    const modeEl = screen.getByTestId("mode-indicator-mode");

    // Start in normal
    expect(modeEl.textContent).toBe("-- NORMAL --");

    // Switch to command
    act(() => {
      screen.getByTestId("to-command").click();
    });
    expect(modeEl.textContent).toBe("-- COMMAND --");

    // Switch to search
    act(() => {
      screen.getByTestId("to-search").click();
    });
    expect(modeEl.textContent).toBe("-- SEARCH --");

    // Back to normal
    act(() => {
      screen.getByTestId("to-normal").click();
    });
    expect(modeEl.textContent).toBe("-- NORMAL --");
  });

  it("renders placeholder slots for view name and sort/filter", () => {
    render(
      <AppModeProvider>
        <ModeIndicator />
      </AppModeProvider>,
    );

    expect(screen.getByTestId("mode-indicator-left")).toBeDefined();
    expect(screen.getByTestId("mode-indicator-right")).toBeDefined();
  });
});
