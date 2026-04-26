import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import {
  AlertTriangle,
  CheckCircle,
  Clock,
  Play,
  PlusCircle,
} from "lucide-react";
import {
  StatusDateDisplay,
  statusDateIconOverride,
  statusDateTooltipOverride,
} from "./status-date-display";
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
    it("renders 'Completed' phrasing without an inline icon", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "completed", timestamp: daysAgo(3) })}
        />,
      );
      expect(container.textContent).toContain("Completed");
      // The display delegates its icon to the parent layout via iconOverride;
      // it should no longer render an inline SVG icon itself.
      const svg = container.querySelector("svg");
      expect(svg).toBeNull();
    });
  });

  describe("kind: overdue", () => {
    it("renders 'Overdue by' phrasing without an inline icon", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "overdue", timestamp: daysAgo(5) })}
        />,
      );
      expect(container.textContent).toContain("Overdue by");
      const svg = container.querySelector("svg");
      expect(svg).toBeNull();
    });
  });

  describe("kind: started", () => {
    it("renders 'Started' phrasing without an inline icon", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "started", timestamp: daysAgo(2) })}
        />,
      );
      expect(container.textContent).toContain("Started");
      const svg = container.querySelector("svg");
      expect(svg).toBeNull();
    });
  });

  describe("kind: scheduled", () => {
    it("renders 'Scheduled' phrasing with 'in' suffix for future, without an inline icon", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "scheduled", timestamp: daysFromNow(7) })}
        />,
      );
      expect(container.textContent).toContain("Scheduled");
      expect(container.textContent).toContain("in ");
      const svg = container.querySelector("svg");
      expect(svg).toBeNull();
    });
  });

  describe("kind: created", () => {
    it("renders 'Created' phrasing without an inline icon", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "created", timestamp: daysAgo(30) })}
        />,
      );
      expect(container.textContent).toContain("Created");
      const svg = container.querySelector("svg");
      expect(svg).toBeNull();
    });
  });

  describe("invalid shapes render an empty CompactCellWrapper (compact) or null (full)", () => {
    /**
     * In compact mode, every display must emit a fixed-height wrapper —
     * even when the value is unparseable — so the row honors the
     * `DataTable` virtualizer's `ROW_HEIGHT` contract. The wrapper has
     * no visible content; the assertion is structural.
     */
    function expectEmptyCompactWrapper(container: HTMLElement) {
      const wrapper = container.querySelector("[data-compact-cell='true']");
      expect(wrapper).toBeTruthy();
      expect(wrapper!.textContent).toBe("");
      // No phrase content should leak from the StatusDateDisplay branches.
      expect(container.querySelector("svg")).toBeNull();
    }

    it("renders an empty wrapper for null value (compact)", () => {
      const { container } = render(<StatusDateDisplay {...makeProps(null)} />);
      expectEmptyCompactWrapper(container);
    });

    it("renders an empty wrapper for non-object primitive (compact)", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps("2026-04-10T00:00:00Z")} />,
      );
      expectEmptyCompactWrapper(container);
    });

    it("renders an empty wrapper for array value (compact)", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps([1, 2, 3])} />,
      );
      expectEmptyCompactWrapper(container);
    });

    it("renders an empty wrapper for unknown kind (compact)", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "mystery", timestamp: daysAgo(1) })}
        />,
      );
      expectEmptyCompactWrapper(container);
    });

    it("renders an empty wrapper when timestamp is missing (compact)", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps({ kind: "completed" })} />,
      );
      expectEmptyCompactWrapper(container);
    });

    it("renders an empty wrapper when kind is missing (compact)", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps({ timestamp: daysAgo(1) })} />,
      );
      expectEmptyCompactWrapper(container);
    });

    it("returns null in full mode for invalid values (no wrapper, row collapses)", () => {
      const { container } = render(
        <StatusDateDisplay {...makeProps(null, "full")} />,
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

    it("uses DisplayText classes — `text-sm`, no `inline-flex` wrapper", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps({ kind: "completed", timestamp: daysAgo(2) }, "full")}
        />,
      );
      // The status date renders a single <span> (from DisplayText). There
      // should be no `inline-flex` wrapper holding an icon + text pair —
      // the icon has moved to the parent layout, and text styling matches
      // TextDisplay in full mode (`text-sm`).
      const flexWrapper = container.querySelector(".inline-flex");
      expect(flexWrapper).toBeNull();
      const rootSpan = container.querySelector("span[title]");
      expect(rootSpan!.className).toContain("text-sm");
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

    it("uses DisplayText classes — no `text-xs`, no `text-muted-foreground`, no `inline-flex`", () => {
      const { container } = render(
        <StatusDateDisplay
          {...makeProps(
            { kind: "completed", timestamp: daysAgo(2) },
            "compact",
          )}
        />,
      );
      // Compact status-date text must match TextDisplay compact: inherited
      // size and color, with truncation. No ad-hoc `text-xs` /
      // `text-muted-foreground` overrides, no `inline-flex` wrapper.
      const flexWrapper = container.querySelector(".inline-flex");
      expect(flexWrapper).toBeNull();
      const rootSpan = container.querySelector("span[title]");
      expect(rootSpan!.className).not.toContain("text-xs");
      expect(rootSpan!.className).not.toContain("text-muted-foreground");
      expect(rootSpan!.className).toContain("truncate");
      expect(rootSpan!.className).toContain("block");
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

describe("statusDateIconOverride", () => {
  it("returns CheckCircle for kind: completed", () => {
    expect(
      statusDateIconOverride({ kind: "completed", timestamp: daysAgo(3) }),
    ).toBe(CheckCircle);
  });

  it("returns AlertTriangle for kind: overdue", () => {
    expect(
      statusDateIconOverride({ kind: "overdue", timestamp: daysAgo(5) }),
    ).toBe(AlertTriangle);
  });

  it("returns Play for kind: started", () => {
    expect(
      statusDateIconOverride({ kind: "started", timestamp: daysAgo(2) }),
    ).toBe(Play);
  });

  it("returns Clock for kind: scheduled", () => {
    expect(
      statusDateIconOverride({ kind: "scheduled", timestamp: daysFromNow(7) }),
    ).toBe(Clock);
  });

  it("returns PlusCircle for kind: created", () => {
    expect(
      statusDateIconOverride({ kind: "created", timestamp: daysAgo(30) }),
    ).toBe(PlusCircle);
  });

  it("returns null for null input", () => {
    expect(statusDateIconOverride(null)).toBeNull();
  });

  it("returns null for non-object input", () => {
    expect(statusDateIconOverride("not an object")).toBeNull();
  });

  it("returns null for unknown kind", () => {
    expect(
      statusDateIconOverride({ kind: "mystery", timestamp: daysAgo(1) }),
    ).toBeNull();
  });
});

describe("statusDateTooltipOverride", () => {
  it("returns a 'Completed' phrase for kind: completed", () => {
    const result = statusDateTooltipOverride({
      kind: "completed",
      timestamp: daysAgo(3),
    });
    expect(result).not.toBeNull();
    expect(result).toContain("Completed");
    expect(result).toContain("ago");
  });

  it("returns an 'Overdue by' phrase for kind: overdue", () => {
    const result = statusDateTooltipOverride({
      kind: "overdue",
      timestamp: daysAgo(5),
    });
    expect(result).not.toBeNull();
    expect(result).toContain("Overdue by");
  });

  it("returns a 'Started' phrase for kind: started", () => {
    const result = statusDateTooltipOverride({
      kind: "started",
      timestamp: daysAgo(2),
    });
    expect(result).not.toBeNull();
    expect(result).toContain("Started");
  });

  it("returns a 'Scheduled' phrase for kind: scheduled", () => {
    const result = statusDateTooltipOverride({
      kind: "scheduled",
      timestamp: daysFromNow(7),
    });
    expect(result).not.toBeNull();
    expect(result).toContain("Scheduled");
    expect(result).toContain("in ");
  });

  it("returns a 'Created' phrase for kind: created", () => {
    const result = statusDateTooltipOverride({
      kind: "created",
      timestamp: daysAgo(30),
    });
    expect(result).not.toBeNull();
    expect(result).toContain("Created");
  });

  it("returns null for null input", () => {
    expect(statusDateTooltipOverride(null)).toBeNull();
  });

  it("returns null for non-object input", () => {
    expect(statusDateTooltipOverride("not an object")).toBeNull();
  });

  it("returns null for unknown kind", () => {
    expect(
      statusDateTooltipOverride({ kind: "mystery", timestamp: daysAgo(1) }),
    ).toBeNull();
  });

  it("returns the label when timestamp is unparseable", () => {
    const result = statusDateTooltipOverride({
      kind: "completed",
      timestamp: "not-a-date",
    });
    expect(result).not.toBeNull();
    expect(result).toBe("Completed");
  });
});
