/**
 * Tests for `<FocusIndicator>` — the single visible focus decorator.
 *
 * The component is intentionally minimal: render a dotted border inside
 * the host's box when `focused` is true, render nothing when `focused`
 * is false. Visual styling is a Tailwind-class concern; these tests pin
 * the contract that:
 *
 *   - The decoration is present iff `focused === true`.
 *   - It carries `aria-hidden` so screen readers don't announce a
 *     duplicate focus event (the host element is what gets focus).
 *   - It carries `pointer-events-none` so it never intercepts a click.
 *   - It paints at `absolute inset-0` inside the host, with a 1px
 *     dotted border in the primary color. There is no second variant —
 *     one indicator visual, period.
 *   - It inherits the host's border-radius so cards / pills with
 *     rounded corners get a matching dotted outline.
 */
import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { FocusIndicator } from "./focus-indicator";

describe("<FocusIndicator>", () => {
  it("renders the decoration when focused is true", () => {
    const { queryByTestId } = render(<FocusIndicator focused={true} />);
    const indicator = queryByTestId("focus-indicator");
    expect(indicator).not.toBeNull();
  });

  it("renders nothing when focused is false", () => {
    const { queryByTestId } = render(<FocusIndicator focused={false} />);
    expect(queryByTestId("focus-indicator")).toBeNull();
  });

  it("the decoration is aria-hidden so screen readers don't announce it", () => {
    const { getByTestId } = render(<FocusIndicator focused={true} />);
    expect(getByTestId("focus-indicator").getAttribute("aria-hidden")).toBe(
      "true",
    );
  });

  it("the decoration is pointer-events-none so it doesn't intercept clicks", () => {
    const { getByTestId } = render(<FocusIndicator focused={true} />);
    // Tailwind class — load-bearing because the decoration overlays the
    // host's click target. A regression here would block click-to-focus
    // on the primitive whose indicator is showing.
    expect(getByTestId("focus-indicator").className).toMatch(
      /pointer-events-none/,
    );
  });

  it("paints inside the host's box as a dotted border in the primary color", () => {
    // The single visual contract: `absolute inset-0` so the decoration
    // traces the host's bounding box exactly, with `border border-dotted
    // border-primary` for the 1px dotted outline. There is no second
    // variant.
    const { getByTestId } = render(<FocusIndicator focused={true} />);
    const cls = getByTestId("focus-indicator").className;
    expect(cls).toMatch(/\babsolute\b/);
    expect(cls).toMatch(/\binset-0\b/);
    expect(cls).toMatch(/\bborder\b/);
    expect(cls).toMatch(/\bborder-dotted\b/);
    expect(cls).toMatch(/\bborder-primary\b/);
    // The legacy cursor-bar tokens are gone — the indicator no longer
    // lives outside the host's box, no longer paints a solid stripe,
    // and no longer fixes a width.
    expect(cls).not.toMatch(/-left-2/);
    expect(cls).not.toMatch(/\bw-1\b/);
    expect(cls).not.toMatch(/\bbg-primary\b/);
    // Architectural guard against the historic "ring" variant — the
    // decoration uses `border`, never the `ring-*` utility family.
    expect(cls).not.toMatch(/\bring-2\b/);
  });

  it("inherits the host's border-radius so it follows rounded corners", () => {
    // `rounded-[inherit]` is load-bearing for cards and pills with
    // `rounded-md` / `rounded-lg` hosts — without it the indicator
    // would draw square corners over a rounded host and look wrong.
    const { getByTestId } = render(<FocusIndicator focused={true} />);
    const cls = getByTestId("focus-indicator").className;
    expect(cls).toMatch(/rounded-\[inherit\]/);
  });

  it("paints inside an overflow:hidden ancestor without being clipped", () => {
    // Regression for the family of "focus indicator clipped by
    // overflow: hidden" bugs (e.g. the toolbar `truncate` wrapper
    // around board.name and percent_complete). With the dotted-inset
    // redesign the indicator lives at `absolute inset-0` inside the
    // host, so it occupies the host's content box exactly — by
    // construction it cannot fall outside any ancestor that contains
    // the host. Pin that contract here so a future redesign that
    // re-introduces an outside-the-box offset trips this test.
    //
    // The Vitest browser harness doesn't compile Tailwind, so the
    // indicator's `absolute inset-0` and `border` classes resolve to no
    // styling. Inject a tiny CSS shim translating the relevant
    // utilities into raw properties so `getBoundingClientRect()` returns
    // a real rect for the indicator.
    const styleEl = document.createElement("style");
    styleEl.textContent = `
      .absolute { position: absolute; }
      .inset-0 { top: 0; right: 0; bottom: 0; left: 0; }
      .border { border-width: 1px; border-style: solid; }
      .border-dotted { border-style: dotted; }
    `;
    document.head.appendChild(styleEl);

    try {
      const { getByTestId, container } = render(
        <div
          data-testid="clip-parent"
          style={{
            overflow: "hidden",
            position: "relative",
            width: "120px",
            height: "32px",
          }}
        >
          <span
            data-testid="focus-host"
            style={{
              position: "relative",
              display: "inline-block",
              padding: 4,
            }}
          >
            <FocusIndicator focused={true} />
            <span>content</span>
          </span>
        </div>,
      );

      const indicator = getByTestId("focus-indicator");
      const indicatorRect = indicator.getBoundingClientRect();
      const parent = container.querySelector(
        "[data-testid='clip-parent']",
      ) as HTMLElement;
      const parentRect = parent.getBoundingClientRect();

      expect(
        indicatorRect.width,
        "indicator must have non-zero width",
      ).toBeGreaterThan(0);
      expect(
        indicatorRect.height,
        "indicator must have non-zero height",
      ).toBeGreaterThan(0);
      // The indicator's bounding rect lies entirely within the
      // overflow:hidden parent — no edge falls outside.
      expect(indicatorRect.left).toBeGreaterThanOrEqual(parentRect.left);
      expect(indicatorRect.top).toBeGreaterThanOrEqual(parentRect.top);
      expect(indicatorRect.right).toBeLessThanOrEqual(parentRect.right);
      expect(indicatorRect.bottom).toBeLessThanOrEqual(parentRect.bottom);
    } finally {
      styleEl.remove();
    }
  });
});
