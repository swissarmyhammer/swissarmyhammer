import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { ProgressRingDisplay } from "./progress-ring-display";
import type { DisplayProps } from "./text-display";

function makeProps(
  value: unknown,
  mode: "compact" | "full" = "compact",
): DisplayProps {
  return {
    field: {
      id: "f1",
      name: "percent_complete",
      type: { kind: "computed" },
    } as DisplayProps["field"],
    value,
    entity: { entity_type: "board", id: "b1", moniker: "board:b1", fields: {} },
    mode,
  };
}

describe("ProgressRingDisplay", () => {
  // Test with the ACTUAL backend shape for board-percent-complete
  it("renders ring for board shape { done, total, percent }", () => {
    const { container } = render(
      <ProgressRingDisplay
        {...makeProps({ done: 3, total: 5, percent: 60 })}
      />,
    );
    expect(container.querySelector("[role='progressbar']")).toBeTruthy();
    expect(container.textContent).toContain("60%");
  });

  // Test with the ACTUAL backend shape for parse-body-progress
  it("renders ring for task shape { completed, total, percent }", () => {
    const { container } = render(
      <ProgressRingDisplay
        {...makeProps({ completed: 2, total: 4, percent: 50 })}
      />,
    );
    expect(container.querySelector("[role='progressbar']")).toBeTruthy();
    expect(container.textContent).toContain("50%");
  });

  // ---------------------------------------------------------------------
  // Empty-state wrapper invariant — every compact-mode display in a
  // virtualized grid row must emit a `[data-compact-cell="true"]`
  // wrapper, so a column composed entirely of empty progress rings
  // doesn't collapse rows below `data-table.tsx::ROW_HEIGHT`. Mirrors
  // the contract on `ProgressDisplay` / `StatusDateDisplay`.
  // ---------------------------------------------------------------------

  it("compact mode wraps an empty cell in CompactCellWrapper when total is 0", () => {
    const { container } = render(
      <ProgressRingDisplay
        {...makeProps({ done: 0, total: 0, percent: 0 }, "compact")}
      />,
    );
    const wrapper = container.querySelector("[data-compact-cell='true']");
    expect(wrapper).toBeTruthy();
  });

  it("compact mode wraps an empty cell in CompactCellWrapper for null value", () => {
    const { container } = render(
      <ProgressRingDisplay {...makeProps(null, "compact")} />,
    );
    const wrapper = container.querySelector("[data-compact-cell='true']");
    expect(wrapper).toBeTruthy();
  });

  it("compact mode wraps an empty cell in CompactCellWrapper for non-object value", () => {
    const { container } = render(
      <ProgressRingDisplay {...makeProps(42, "compact")} />,
    );
    const wrapper = container.querySelector("[data-compact-cell='true']");
    expect(wrapper).toBeTruthy();
  });

  it("full mode returns null when total is 0", () => {
    const { container } = render(
      <ProgressRingDisplay
        {...makeProps({ done: 0, total: 0, percent: 0 }, "full")}
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("full mode returns null for null value", () => {
    const { container } = render(
      <ProgressRingDisplay {...makeProps(null, "full")} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("full mode returns null for non-object value", () => {
    const { container } = render(
      <ProgressRingDisplay {...makeProps(42, "full")} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders full mode with done/total text", () => {
    const { container } = render(
      <ProgressRingDisplay
        {...makeProps({ done: 2, total: 4, percent: 50 }, "full")}
      />,
    );
    expect(container.textContent).toContain("2/4");
    expect(container.textContent).toContain("50%");
  });
});
