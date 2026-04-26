/**
 * Tests for `<FocusIndicator>` — the single visible focus decorator.
 *
 * The component is intentionally minimal: render the bar when `focused`
 * is true, render nothing when `focused` is false. Visual styling is a
 * Tailwind-class concern; these tests pin the contract that:
 *
 *   - The bar is present iff `focused === true`.
 *   - It carries `aria-hidden` so screen readers don't announce a
 *     duplicate focus event (the host element is what gets focus).
 *   - It carries `pointer-events-none` so it never intercepts a click.
 */
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { FocusIndicator } from "./focus-indicator";

describe("<FocusIndicator>", () => {
  it("renders the bar when focused is true", () => {
    const { queryByTestId } = render(<FocusIndicator focused={true} />);
    const bar = queryByTestId("focus-indicator");
    expect(bar).not.toBeNull();
  });

  it("renders nothing when focused is false", () => {
    const { queryByTestId } = render(<FocusIndicator focused={false} />);
    expect(queryByTestId("focus-indicator")).toBeNull();
  });

  it("the bar is aria-hidden so screen readers don't announce it", () => {
    const { getByTestId } = render(<FocusIndicator focused={true} />);
    expect(getByTestId("focus-indicator").getAttribute("aria-hidden")).toBe(
      "true",
    );
  });

  it("the bar is pointer-events-none so it doesn't intercept clicks", () => {
    const { getByTestId } = render(<FocusIndicator focused={true} />);
    // Tailwind class — load-bearing because the bar overlays the host's
    // click target. A regression here would block click-to-focus on the
    // primitive whose indicator is showing.
    expect(getByTestId("focus-indicator").className).toMatch(
      /pointer-events-none/,
    );
  });
});
