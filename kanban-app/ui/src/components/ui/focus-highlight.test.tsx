import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { FocusHighlight } from "./focus-highlight";

describe("FocusHighlight", () => {
  it("sets data-focused when focused=true", () => {
    const { container } = render(
      <FocusHighlight focused={true}>content</FocusHighlight>,
    );
    const el = container.firstElementChild!;
    expect(el.hasAttribute("data-focused")).toBe(true);
  });

  it("does not set data-focused when focused=false", () => {
    const { container } = render(
      <FocusHighlight focused={false}>content</FocusHighlight>,
    );
    const el = container.firstElementChild!;
    expect(el.hasAttribute("data-focused")).toBe(false);
  });

  it("renders children", () => {
    const { getByText } = render(
      <FocusHighlight focused={false}>hello world</FocusHighlight>,
    );
    expect(getByText("hello world")).toBeTruthy();
  });

  it("renders as specified tag", () => {
    const { container } = render(
      <FocusHighlight focused={false} as="section">
        content
      </FocusHighlight>,
    );
    expect(container.firstElementChild!.tagName).toBe("SECTION");
  });

  it("passes through additional props", () => {
    const { container } = render(
      <FocusHighlight focused={false} data-testid="test" className="custom">
        content
      </FocusHighlight>,
    );
    const el = container.firstElementChild!;
    expect(el.getAttribute("data-testid")).toBe("test");
    expect(el.className).toContain("custom");
  });
});
