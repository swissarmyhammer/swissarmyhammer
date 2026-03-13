import { describe, it, expect } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { AppModeProvider, useAppMode, type AppMode } from "./app-mode-context";

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

describe("AppModeProvider", () => {
  it("defaults to normal mode", () => {
    function ModeReader() {
      const { mode } = useAppMode();
      return <span data-testid="mode">{mode}</span>;
    }

    render(
      <AppModeProvider>
        <ModeReader />
      </AppModeProvider>,
    );

    expect(screen.getByTestId("mode").textContent).toBe("normal");
  });

  it("changes mode via setMode", () => {
    function ModeReader() {
      const { mode } = useAppMode();
      return <span data-testid="mode">{mode}</span>;
    }

    render(
      <AppModeProvider>
        <ModeChanger to="command" />
        <ModeReader />
      </AppModeProvider>,
    );

    act(() => {
      screen.getByTestId("change-mode").click();
    });

    expect(screen.getByTestId("mode").textContent).toBe("command");
  });

  it("switches between modes correctly", () => {
    function MultiModeChanger() {
      const { mode, setMode } = useAppMode();
      return (
        <>
          <span data-testid="current-mode">{mode}</span>
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
      </AppModeProvider>,
    );

    const modeEl = screen.getByTestId("current-mode");

    expect(modeEl.textContent).toBe("normal");

    act(() => {
      screen.getByTestId("to-command").click();
    });
    expect(modeEl.textContent).toBe("command");

    act(() => {
      screen.getByTestId("to-search").click();
    });
    expect(modeEl.textContent).toBe("search");

    act(() => {
      screen.getByTestId("to-normal").click();
    });
    expect(modeEl.textContent).toBe("normal");
  });
});
