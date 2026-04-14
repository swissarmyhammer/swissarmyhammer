/**
 * Tests for DateDisplay — the muted-dash / formatted-date display for date
 * fields. When a field is unset, DateDisplay renders the field's
 * `description` (from the YAML def) as muted help text. When no description
 * is defined, it falls back to the classic `-`.
 *
 * When a value is set, the visible text comes from
 * {@link ../../../lib/format-date.ts formatDateForDisplay} (calendar-aware
 * phrasing like `"yesterday"` or `"Jun 15, 2025"`) and the raw ISO string is
 * exposed via the native `title` tooltip.
 */

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";

import { DateDisplay } from "./date-display";
import type { Entity, FieldDef } from "@/types/kanban";

/** Task entity used as the host of date fields in tests. */
const taskEntity: Entity = {
  id: "task-1",
  entity_type: "task",
  moniker: "task:task-1",
  fields: {},
};

/** Date field with a description — mirrors a builtin like `due`. */
const dueField: FieldDef = {
  id: "00000000000000000000000011",
  name: "due",
  description: "Hard deadline date",
  type: { kind: "date" },
  editor: "date",
  display: "date",
} as unknown as FieldDef;

/** Date field with no description — fallback-path fixture. */
const bareDateField: FieldDef = {
  id: "00000000000000000000000099",
  name: "bare_date",
  type: { kind: "date" },
  editor: "date",
  display: "date",
} as unknown as FieldDef;

describe("DateDisplay", () => {
  it("renders the field description (muted) when the value is empty", () => {
    const { container } = render(
      <DateDisplay field={dueField} value="" entity={taskEntity} mode="full" />,
    );

    const muted = container.querySelector("span.text-muted-foreground\\/50");
    expect(muted).toBeTruthy();
    expect(muted?.textContent).toBe("Hard deadline date");
  });

  it("renders a dash (muted) when the value is empty and no description is set", () => {
    const { container } = render(
      <DateDisplay
        field={bareDateField}
        value=""
        entity={taskEntity}
        mode="full"
      />,
    );

    const muted = container.querySelector("span.text-muted-foreground\\/50");
    expect(muted).toBeTruthy();
    expect(muted?.textContent).toBe("-");
  });

  it("renders formatted visible text and exposes the raw ISO via title", () => {
    // A far-past date (year 2020) is guaranteed to be >30 days from any
    // realistic `Date.now()`, so the format-date helper falls into the
    // localized-date branch and emits "Jun 15, 2020" — never the raw ISO.
    const { container } = render(
      <DateDisplay
        field={dueField}
        value="2020-06-15"
        entity={taskEntity}
        mode="full"
      />,
    );

    const span = container.querySelector("span.tabular-nums");
    expect(span).toBeTruthy();
    // Visible text is formatted — NOT the raw ISO.
    expect(span?.textContent).not.toBe("2020-06-15");
    expect(span?.textContent).toContain("2020");
    expect(span?.textContent).toMatch(/Jun/);
    // Raw ISO string is preserved on the title attribute for hover.
    expect(span?.getAttribute("title")).toBe("2020-06-15");
  });
});
