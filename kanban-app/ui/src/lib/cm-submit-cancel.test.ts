import { describe, it, expect } from "vitest";
import { buildSubmitCancelExtensions } from "./cm-submit-cancel";

describe("buildSubmitCancelExtensions", () => {
  const makeRefs = () => ({
    onSubmitRef: { current: () => {} },
    onCancelRef: { current: () => {} },
    saveInPlaceRef: { current: () => {} },
  });

  it("returns non-empty extensions for vim mode", () => {
    const refs = makeRefs();
    const exts = buildSubmitCancelExtensions({ mode: "vim", ...refs });
    expect(Array.isArray(exts)).toBe(true);
    expect(exts.length).toBeGreaterThan(0);
  });

  it("returns non-empty extensions for cua mode", () => {
    const refs = makeRefs();
    const exts = buildSubmitCancelExtensions({ mode: "cua", ...refs });
    expect(Array.isArray(exts)).toBe(true);
    expect(exts.length).toBeGreaterThan(0);
  });

  it("returns non-empty extensions for emacs mode", () => {
    const refs = makeRefs();
    const exts = buildSubmitCancelExtensions({ mode: "emacs", ...refs });
    expect(Array.isArray(exts)).toBe(true);
    expect(exts.length).toBeGreaterThan(0);
  });

  it("works without saveInPlaceRef", () => {
    const exts = buildSubmitCancelExtensions({
      mode: "vim",
      onSubmitRef: { current: () => {} },
      onCancelRef: { current: () => {} },
    });
    expect(exts.length).toBeGreaterThan(0);
  });

  it("handles null ref values gracefully", () => {
    const exts = buildSubmitCancelExtensions({
      mode: "cua",
      onSubmitRef: { current: null },
      onCancelRef: { current: null },
    });
    expect(exts.length).toBeGreaterThan(0);
  });
});
