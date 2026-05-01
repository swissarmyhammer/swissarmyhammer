import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { ProgressDisplay } from "./progress-display";
import type { DisplayProps } from "./text-display";

/**
 * Build props for {@link ProgressDisplay}. Matches the contract the Field
 * registry adapter supplies: a minimal `field`, the value under test, a
 * dummy entity, and the presentation mode.
 */
function makeProps(
  value: unknown,
  mode: "compact" | "full" = "compact",
): DisplayProps {
  return {
    field: {
      id: "f1",
      name: "progress",
      type: { kind: "computed" },
    } as DisplayProps["field"],
    value,
    entity: { entity_type: "task", id: "t1", moniker: "task:t1", fields: {} },
    mode,
  };
}

describe("ProgressDisplay", () => {
  // Exact production shape from the bug report:
  // `{completed: 14, percent: 100, total: 14}` — all checkboxes complete.
  // The clipboard showed this value but the bar rendered as 0.
  it("renders 100% bar for production shape { completed, total, percent }", () => {
    const { container } = render(
      <ProgressDisplay
        {...makeProps({ completed: 14, percent: 100, total: 14 })}
      />,
    );
    const bar = container.querySelector('[role="progressbar"]');
    expect(bar).toBeTruthy();
    expect(bar!.getAttribute("aria-valuenow")).toBe("100");
    expect(container.textContent).toContain("100%");
  });

  it("renders partial bar in compact mode", () => {
    const { container } = render(
      <ProgressDisplay
        {...makeProps({ completed: 1, total: 3, percent: 33 })}
      />,
    );
    const bar = container.querySelector('[role="progressbar"]');
    expect(bar).toBeTruthy();
    expect(bar!.getAttribute("aria-valuenow")).toBe("33");
    expect(container.textContent).toContain("33%");
  });

  it("renders completed/total in full mode", () => {
    const { container } = render(
      <ProgressDisplay
        {...makeProps({ completed: 14, total: 14, percent: 100 }, "full")}
      />,
    );
    expect(container.textContent).toContain("14/14");
  });

  /**
   * In compact mode, every display must emit a fixed-height
   * CompactCellWrapper (even for empty/invalid values) so the row
   * honors the `DataTable` virtualizer's `ROW_HEIGHT` contract. The
   * empty branch wraps a muted dash with no progress bar.
   */
  function expectEmptyCompactWrapper(container: HTMLElement) {
    const wrapper = container.querySelector("[data-compact-cell='true']");
    expect(wrapper).toBeTruthy();
    expect(wrapper!.querySelector('[role="progressbar"]')).toBeNull();
    expect(wrapper!.textContent).toBe("-");
  }

  it("renders an empty wrapper when total is 0 (compact)", () => {
    const { container } = render(
      <ProgressDisplay
        {...makeProps({ completed: 0, total: 0, percent: 0 })}
      />,
    );
    expectEmptyCompactWrapper(container);
  });

  it("renders an empty wrapper for null value (compact)", () => {
    const { container } = render(<ProgressDisplay {...makeProps(null)} />);
    expectEmptyCompactWrapper(container);
  });

  it("renders an empty wrapper for non-object value (compact)", () => {
    const { container } = render(<ProgressDisplay {...makeProps(42)} />);
    expectEmptyCompactWrapper(container);
  });

  it("returns null in full mode for invalid values (no wrapper, row collapses)", () => {
    const { container } = render(
      <ProgressDisplay {...makeProps(null, "full")} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("returns null in full mode when total is 0 (no wrapper, row collapses)", () => {
    const { container } = render(
      <ProgressDisplay
        {...makeProps({ completed: 0, total: 0, percent: 0 }, "full")}
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  // The backend uses u32 for `percent` and usize for total/completed.
  // Through serde_json these all arrive as JSON numbers — no string coercion.
  // If percent ever arrived as a string it would silently fall back to 0.
  it("falls back to 0 percent for a string percent value (defensive)", () => {
    const { container } = render(
      <ProgressDisplay
        {...makeProps({ completed: 14, total: 14, percent: "100" })}
      />,
    );
    const bar = container.querySelector('[role="progressbar"]');
    // Bar still renders because total is a number > 0, but the percent text
    // shows 0% — this defensive fallback is why a type regression in the
    // pipeline (e.g. percent serialised as a string) would produce the
    // "empty bar" symptom reported in the bug.
    expect(bar).toBeTruthy();
    expect(bar!.getAttribute("aria-valuenow")).toBe("0");
  });
});
