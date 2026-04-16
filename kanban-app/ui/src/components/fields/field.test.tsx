import { describe, it, expect } from "vitest";
import { CheckCircle, AlertTriangle } from "lucide-react";
import {
  registerDisplay,
  getDisplayIsEmpty,
  getDisplayIconOverride,
  getDisplayTooltipOverride,
  type FieldDisplayProps,
} from "./field";

function DummyDisplay(_: FieldDisplayProps) {
  return null;
}

describe("registerDisplay / getDisplayIsEmpty", () => {
  it("accepts a registration without options (backwards compatibility)", () => {
    registerDisplay("display-no-options", DummyDisplay);
    expect(getDisplayIsEmpty("display-no-options")).toBeUndefined();
  });

  it("stores and returns an isEmpty predicate when provided", () => {
    const predicate = (v: unknown): boolean => typeof v === "number" && v === 0;
    registerDisplay("display-with-options", DummyDisplay, {
      isEmpty: predicate,
    });
    expect(getDisplayIsEmpty("display-with-options")).toBe(predicate);
  });

  it("returns undefined for a display name that has never been registered", () => {
    expect(getDisplayIsEmpty("nonexistent-display-name")).toBeUndefined();
  });

  it("invokes the stored predicate with the supplied value", () => {
    registerDisplay("display-invoke", DummyDisplay, {
      isEmpty: (v) => v === "empty",
    });
    const isEmpty = getDisplayIsEmpty("display-invoke");
    expect(isEmpty).toBeDefined();
    expect(isEmpty!("empty")).toBe(true);
    expect(isEmpty!("full")).toBe(false);
  });
});

describe("registerDisplay / getDisplayIconOverride", () => {
  it("returns undefined for a display name that has never been registered", () => {
    expect(getDisplayIconOverride("nonexistent-icon-override")).toBeUndefined();
  });

  it("returns the iconOverride function when registered", () => {
    const override = (v: unknown) =>
      v === "check" ? CheckCircle : AlertTriangle;
    registerDisplay("display-with-icon-override", DummyDisplay, {
      iconOverride: override,
    });
    expect(getDisplayIconOverride("display-with-icon-override")).toBe(override);
  });

  it("returns undefined when registered without an iconOverride", () => {
    registerDisplay("display-no-icon-override", DummyDisplay);
    expect(getDisplayIconOverride("display-no-icon-override")).toBeUndefined();
  });

  it("invokes the override with the current value and returns the icon", () => {
    registerDisplay("display-icon-invoke", DummyDisplay, {
      iconOverride: (v: unknown) => (v === "check" ? CheckCircle : null),
    });
    const override = getDisplayIconOverride("display-icon-invoke");
    expect(override).toBeDefined();
    expect(override!("check")).toBe(CheckCircle);
    expect(override!("other")).toBeNull();
  });
});

describe("registerDisplay / getDisplayTooltipOverride", () => {
  it("returns undefined for a display name that has never been registered", () => {
    expect(
      getDisplayTooltipOverride("nonexistent-tooltip-override"),
    ).toBeUndefined();
  });

  it("returns the tooltipOverride function when registered", () => {
    const override = (v: unknown) =>
      v === "done" ? "Completed 3 days ago" : null;
    registerDisplay("display-with-tooltip-override", DummyDisplay, {
      tooltipOverride: override,
    });
    expect(getDisplayTooltipOverride("display-with-tooltip-override")).toBe(
      override,
    );
  });

  it("returns undefined when registered without a tooltipOverride", () => {
    registerDisplay("display-no-tooltip-override", DummyDisplay);
    expect(
      getDisplayTooltipOverride("display-no-tooltip-override"),
    ).toBeUndefined();
  });

  it("invokes the override with the current value and returns a string or null", () => {
    registerDisplay("display-tooltip-invoke", DummyDisplay, {
      tooltipOverride: (v: unknown) =>
        v === "done" ? "Completed 3 days ago" : null,
    });
    const override = getDisplayTooltipOverride("display-tooltip-invoke");
    expect(override).toBeDefined();
    expect(override!("done")).toBe("Completed 3 days ago");
    expect(override!("other")).toBeNull();
  });
});
