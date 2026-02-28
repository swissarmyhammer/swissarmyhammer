import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { SubtaskProgress, checkboxProgress } from "./subtask-progress";

describe("checkboxProgress", () => {
  it("returns null for empty description", () => {
    expect(checkboxProgress("")).toBeNull();
    expect(checkboxProgress(undefined)).toBeNull();
  });

  it("returns null when no checkboxes present", () => {
    expect(checkboxProgress("Just some text")).toBeNull();
  });

  it("counts checked and total checkboxes", () => {
    expect(checkboxProgress("- [x] done\n- [ ] todo")).toEqual({
      checked: 1,
      total: 2,
    });
  });

  it("counts uppercase X as checked", () => {
    expect(checkboxProgress("- [X] done")).toEqual({ checked: 1, total: 1 });
  });

  it("handles all unchecked", () => {
    expect(checkboxProgress("- [ ] a\n- [ ] b\n- [ ] c")).toEqual({
      checked: 0,
      total: 3,
    });
  });

  it("handles all checked", () => {
    expect(checkboxProgress("- [x] a\n- [x] b")).toEqual({
      checked: 2,
      total: 2,
    });
  });
});

describe("SubtaskProgress", () => {
  it("renders nothing when no checkboxes", () => {
    const { container } = render(<SubtaskProgress description="no tasks" />);
    expect(container.querySelector('[role="progressbar"]')).toBeNull();
  });

  it("renders progress bar with correct percentage", () => {
    const { container } = render(
      <SubtaskProgress description="- [x] a\n- [ ] b\n- [ ] c\n- [ ] d" />
    );
    const bar = container.querySelector('[role="progressbar"]');
    expect(bar).toBeTruthy();
    expect(bar!.getAttribute("aria-valuenow")).toBe("25");
  });

  it("shows 0% when none checked", () => {
    const { container } = render(
      <SubtaskProgress description="- [ ] a\n- [ ] b" />
    );
    const bar = container.querySelector('[role="progressbar"]');
    expect(bar!.getAttribute("aria-valuenow")).toBe("0");
    expect(container.textContent).toContain("0%");
  });

  it("shows 100% when all checked", () => {
    const { container } = render(
      <SubtaskProgress description="- [x] a\n- [x] b" />
    );
    const bar = container.querySelector('[role="progressbar"]');
    expect(bar!.getAttribute("aria-valuenow")).toBe("100");
  });

  it("applies custom className", () => {
    const { container } = render(
      <SubtaskProgress description="- [ ] a" className="mt-4" />
    );
    const wrapper = container.firstElementChild;
    expect(wrapper?.className).toContain("mt-4");
  });
});
