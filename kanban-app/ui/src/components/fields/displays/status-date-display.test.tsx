import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { StatusDateDisplay } from "./status-date-display";
import type { DisplayProps } from "./text-display";

function makeProps(
  value: unknown,
  mode: "compact" | "full" = "compact",
): DisplayProps {
  return {
    field: {
      id: "f1",
      name: "status_date",
      type: { kind: "computed" },
    } as DisplayProps["field"],
    value,
    entity: { entity_type: "task", id: "t1", moniker: "task:t1", fields: {} },
    mode,
  };
}

/**
 * The display does not accept an injected "now" — it reads `Date.now()`
 * internally via `new Date()`. Tests use timestamps relative to the test's
 * real clock so they stay correct regardless of when they run. The specific
 * magnitude number (e.g. "5 days") is not asserted; tests only verify that
 * the kind-specific phrasing + icon appear.
 */

function daysAgo(n: number): string {
  const d = new Date(Date.now() - n * 24 * 60 * 60 * 1000);
  return d.toISOString();
}

function daysFromNow(n: number): string {
  const d = new Date(Date.now() + n * 24 * 60 * 60 * 1000);
  return d.toISOString();
}

describe("StatusDateDisplay", () => {
  describe("kind: completed", () => {
    it("renders CheckCircle icon and 'Completed' phrasing", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "completed", timestamp: daysAgo(3) })}
        />,
      );
      expect(container.textContent).toContain("Completed");
      // Lucide icons render as inline SVGs with a data-testid or class.
      // CheckCircle carries the lucide-circle-check-big / lucide-check-circle
      // class name; the exact name varies by Lucide version but always starts
      // with "lucide-" and contains "check".
      const svg = container.querySelector("svg");
      expect(svg).toBeTruthy();
      expect(svg!.getAttribute("class")).toMatch(
        /lucide-(circle-check|check-circle)/,
      );
    });
  });

  describe("kind: overdue", () => {
    it("renders AlertTriangle icon and 'Overdue by' phrasing", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "overdue", timestamp: daysAgo(5) })}
        />,
      );
      expect(container.textContent).toContain("Overdue by");
      const svg = container.querySelector("svg");
      expect(svg).toBeTruthy();
      // AlertTriangle renders as lucide-triangle-alert (or lucide-alert-triangle
      // in older versions).
      expect(svg!.getAttribute("class")).toMatch(
        /lucide-(triangle-alert|alert-triangle)/,
      );
    });
  });

  describe("kind: started", () => {
    it("renders Play icon and 'Started' phrasing", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "started", timestamp: daysAgo(2) })}
        />,
      );
      expect(container.textContent).toContain("Started");
      const svg = container.querySelector("svg");
      expect(svg).toBeTruthy();
      expect(svg!.getAttribute("class")).toMatch(/lucide-play/);
    });
  });

  describe("kind: scheduled", () => {
    it("renders Clock icon and 'Scheduled' phrasing with 'in' suffix for future", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "scheduled", timestamp: daysFromNow(7) })}
        />,
      );
      expect(container.textContent).toContain("Scheduled");
      // Future timestamps render as "in <magnitude>" rather than "<magnitude> ago".
      expect(container.textContent).toContain("in ");
      const svg = container.querySelector("svg");
      expect(svg).toBeTruthy();
      expect(svg!.getAttribute("class")).toMatch(/lucide-clock/);
    });
  });

  describe("kind: created", () => {
    it("renders PlusCircle icon and 'Created' phrasing", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "created", timestamp: daysAgo(30) })}
        />,
      );
      expect(container.textContent).toContain("Created");
      const svg = container.querySelector("svg");
      expect(svg).toBeTruthy();
      expect(svg!.getAttribute("class")).toMatch(
        /lucide-(circle-plus|plus-circle)/,
      );
    });
  });

  describe("invalid shapes return null", () => {
    it("returns empty for null value", () => {
      const { container } = render(<StatusDateDisplay {...makeProps(null)} />);
      expect(container.innerHTML).toBe("");
    });

    it("returns empty for non-object primitive", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps("2026-04-10T00:00:00Z")} />,
      );
      expect(container.innerHTML).toBe("");
    });

    it("returns empty for array value", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps([1, 2, 3])} />,
      );
      expect(container.innerHTML).toBe("");
    });

    it("returns empty for unknown kind", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "mystery", timestamp: daysAgo(1) })}
        />,
      );
      expect(container.innerHTML).toBe("");
    });

    it("returns empty when timestamp is missing", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps({ kind: "completed" })} />,
      );
      expect(container.innerHTML).toBe("");
    });

    it("returns empty when kind is missing", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps({ timestamp: daysAgo(1) })} />,
      );
      expect(container.innerHTML).toBe("");
    });
  });

  describe("mode: full", () => {
    it("exposes the absolute ISO timestamp via title tooltip", () => {
      const ts = daysAgo(2);
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "completed", timestamp: ts }, "full")}
        />,
      );
      // The outer <span> carries the title — it's the top-level element we render.
      const rootSpan = container.querySelector("span[title]");
      expect(rootSpan).toBeTruthy();
      expect(rootSpan!.getAttribute("title")).toBe(ts);
    });
  });

  describe("mode: compact", () => {
    it("exposes the absolute ISO timestamp via title tooltip", () => {
      const ts = daysAgo(2);
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "completed", timestamp: ts }, "compact")}
        />,
      );
      const rootSpan = container.querySelector("span[title]");
      expect(rootSpan).toBeTruthy();
      expect(rootSpan!.getAttribute("title")).toBe(ts);
    });
  });

  describe("bare calendar date timestamps", () => {
    it("accepts YYYY-MM-DD form (from due/scheduled fields)", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "overdue", timestamp: "2020-01-01" }, "full")}
        />,
      );
      // Valid overdue row: renders the prefix and the tooltip carries the
      // original calendar-date string untouched.
      expect(container.textContent).toContain("Overdue by");
      const rootSpan = container.querySelector("span[title]");
      expect(rootSpan!.getAttribute("title")).toBe("2020-01-01");
    });
  });
});
