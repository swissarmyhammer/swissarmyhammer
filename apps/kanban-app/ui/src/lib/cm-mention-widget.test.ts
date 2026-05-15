/**
 * Tests for cm-mention-widget module.
 *
 * Covers clipDisplayName (pure string truncation) and MentionWidget
 * (CM6 WidgetType that renders a display-name pill).
 */

import { describe, it, expect } from "vitest";
import { clipDisplayName, MentionWidget } from "./cm-mention-widget";
import { sanitizeHexColor } from "./mention-meta";

describe("clipDisplayName", () => {
  it("returns a short name unchanged", () => {
    expect(clipDisplayName("bug")).toBe("bug");
  });

  it("returns a name exactly at maxChars unchanged", () => {
    const name = "A".repeat(24);
    expect(clipDisplayName(name)).toBe(name);
  });

  it("clips a name one char over maxChars with ellipsis", () => {
    const name = "A".repeat(25);
    expect(clipDisplayName(name)).toBe("A".repeat(24) + "\u2026");
  });

  it("clips a long name at the default 24 chars", () => {
    const name = "This is a very long display name for testing";
    expect(clipDisplayName(name)).toBe("This is a very long disp\u2026");
  });

  it("respects a custom maxChars parameter", () => {
    expect(clipDisplayName("Hello World", 5)).toBe("Hello\u2026");
  });

  it("handles empty string", () => {
    expect(clipDisplayName("")).toBe("");
  });
});

describe("sanitizeHexColor", () => {
  it("accepts a valid 6-digit hex color", () => {
    expect(sanitizeHexColor("ff0000")).toBe("ff0000");
  });

  it("accepts a valid 3-digit hex color", () => {
    expect(sanitizeHexColor("f00")).toBe("f00");
  });

  it("accepts uppercase hex colors", () => {
    expect(sanitizeHexColor("FF00AA")).toBe("FF00AA");
  });

  it("rejects a color with a leading hash", () => {
    expect(sanitizeHexColor("#ff0000")).toBe("");
  });

  it("rejects a color with CSS injection attempt", () => {
    expect(sanitizeHexColor("ff0000; background: url(evil)")).toBe("");
  });

  it("rejects non-hex characters", () => {
    expect(sanitizeHexColor("xyzxyz")).toBe("");
  });

  it("rejects wrong-length strings", () => {
    expect(sanitizeHexColor("ff")).toBe("");
    expect(sanitizeHexColor("fffff")).toBe("");
    expect(sanitizeHexColor("fffffff")).toBe("");
  });

  it("rejects empty string", () => {
    expect(sanitizeHexColor("")).toBe("");
  });
});

describe("MentionWidget", () => {
  it("toDOM() returns a span with the correct textContent", () => {
    const w = new MentionWidget("#", "bug", "Bug Report", "ff0000");
    const el = w.toDOM() as HTMLSpanElement;
    expect(el.textContent).toBe("#Bug Report");
  });

  it("toDOM() clips a long display name", () => {
    const longName = "A".repeat(30);
    const w = new MentionWidget("#", "long-slug", longName, "ff0000");
    const el = w.toDOM() as HTMLSpanElement;
    expect(el.textContent).toBe("#" + "A".repeat(24) + "\u2026");
  });

  it("toDOM() applies the cm-mention-pill class", () => {
    const w = new MentionWidget("#", "bug", "Bug", "ff0000");
    const el = w.toDOM() as HTMLSpanElement;
    expect(el.classList.contains("cm-mention-pill")).toBe(true);
  });

  it("toDOM() sets inline color styles from the color field", () => {
    const w = new MentionWidget("#", "bug", "Bug", "ff0000");
    const el = w.toDOM() as HTMLSpanElement;
    const style = el.getAttribute("style") ?? "";
    expect(style).toContain("--mention-color");
    expect(style).toContain("#ff0000");
  });

  it("eq() returns true for identical fields", () => {
    const a = new MentionWidget("#", "bug", "Bug", "ff0000");
    const b = new MentionWidget("#", "bug", "Bug", "ff0000");
    expect(a.eq(b)).toBe(true);
  });

  it("eq() returns false when any field differs", () => {
    const a = new MentionWidget("#", "bug", "Bug", "ff0000");
    expect(a.eq(new MentionWidget("@", "bug", "Bug", "ff0000"))).toBe(false);
    expect(a.eq(new MentionWidget("#", "other", "Bug", "ff0000"))).toBe(false);
    expect(a.eq(new MentionWidget("#", "bug", "Other", "ff0000"))).toBe(false);
    expect(a.eq(new MentionWidget("#", "bug", "Bug", "00ff00"))).toBe(false);
  });

  it("ignoreEvent() returns false so click events propagate", () => {
    const w = new MentionWidget("#", "bug", "Bug", "ff0000");
    expect(w.ignoreEvent()).toBe(false);
  });

  it("toDOM() drops inline style when color is not a valid hex", () => {
    // Defense in depth: if a malicious color string slips through, the
    // widget's toDOM() must not interpolate it into the style attribute.
    const injection = "ff0000; background: url(evil)";
    const w = new MentionWidget("#", "bug", "Bug", injection);
    const el = w.toDOM() as HTMLSpanElement;
    // Invalid color → sanitizer returns "" → no style attribute is set
    expect(el.hasAttribute("style")).toBe(false);
  });
});
