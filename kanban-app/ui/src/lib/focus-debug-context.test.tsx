import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { FocusDebugProvider, useFocusDebug } from "./focus-debug-context";

/**
 * Tiny consumer that prints the boolean flag returned by `useFocusDebug()`
 * so each test can inspect the rendered text.
 */
function DebugReader() {
  const enabled = useFocusDebug();
  return <span data-testid="debug-flag">{String(enabled)}</span>;
}

describe("useFocusDebug", () => {
  it("use_focus_debug_returns_true_when_provider_enabled", () => {
    render(
      <FocusDebugProvider enabled>
        <DebugReader />
      </FocusDebugProvider>,
    );

    expect(screen.getByTestId("debug-flag").textContent).toBe("true");
  });

  it("use_focus_debug_returns_false_when_provider_disabled", () => {
    render(
      <FocusDebugProvider enabled={false}>
        <DebugReader />
      </FocusDebugProvider>,
    );

    expect(screen.getByTestId("debug-flag").textContent).toBe("false");
  });

  it("use_focus_debug_returns_false_with_no_provider", () => {
    render(<DebugReader />);

    expect(screen.getByTestId("debug-flag").textContent).toBe("false");
  });
});
