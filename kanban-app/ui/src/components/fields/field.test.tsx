import { describe, it, expect } from "vitest";
import {
  registerDisplay,
  getDisplayIsEmpty,
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
