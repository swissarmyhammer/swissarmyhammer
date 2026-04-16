/**
 * Tests for the `DisplayText` primitive and the `TextDisplay` wrapper that
 * builds on it.
 *
 * `DisplayText` is the shared text-rendering building block used by every
 * display that produces a string (TextDisplay, StatusDateDisplay, ...). These
 * tests pin its sizing, color, truncation, and tooltip behaviour so other
 * displays can delegate to it and stay visually consistent.
 */

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";

import { DisplayText, TextDisplay } from "./text-display";
import type { DisplayProps } from "./text-display";

/** Minimal props factory — TextDisplay only reads `value` and `mode`. */
function makeTextProps(
  value: unknown,
  mode: "compact" | "full" = "compact",
): DisplayProps {
  return {
    field: {
      id: "f1",
      name: "text",
      type: { kind: "text" },
    } as DisplayProps["field"],
    value,
    entity: { entity_type: "task", id: "t1", moniker: "task:t1", fields: {} },
    mode,
  };
}

describe("DisplayText", () => {
  describe("empty text", () => {
    it("renders a muted dash when text is the empty string", () => {
      const { container } = render(<DisplayText text="" mode="compact" />);
      const span = container.querySelector("span");
      expect(span).toBeTruthy();
      expect(span!.textContent).toBe("-");
      expect(span!.className).toContain("text-muted-foreground/50");
    });

    it("renders a muted dash in full mode too", () => {
      const { container } = render(<DisplayText text="" mode="full" />);
      const span = container.querySelector("span");
      expect(span!.textContent).toBe("-");
      expect(span!.className).toContain("text-muted-foreground/50");
    });
  });

  describe("compact mode", () => {
    it("renders with `truncate block` and inherited size/color", () => {
      const { container } = render(
        <DisplayText text="hello world" mode="compact" />,
      );
      const span = container.querySelector("span");
      expect(span).toBeTruthy();
      expect(span!.textContent).toBe("hello world");
      expect(span!.className).toContain("truncate");
      expect(span!.className).toContain("block");
      // No size override — parent controls the text size.
      expect(span!.className).not.toContain("text-xs");
      expect(span!.className).not.toContain("text-sm");
      // No muted color — parent controls the text color.
      expect(span!.className).not.toContain("text-muted-foreground");
    });

    it("passes the `title` prop through as the HTML title attribute", () => {
      const { container } = render(
        <DisplayText text="hello" mode="compact" title="tooltip text" />,
      );
      const span = container.querySelector("span");
      expect(span!.getAttribute("title")).toBe("tooltip text");
    });

    it("omits the title attribute when no `title` prop is provided", () => {
      const { container } = render(<DisplayText text="hello" mode="compact" />);
      const span = container.querySelector("span");
      expect(span!.hasAttribute("title")).toBe(false);
    });
  });

  describe("full mode", () => {
    it("renders with `text-sm` and no truncation wrapper", () => {
      const { container } = render(
        <DisplayText text="hello world" mode="full" />,
      );
      const span = container.querySelector("span");
      expect(span).toBeTruthy();
      expect(span!.textContent).toBe("hello world");
      expect(span!.className).toContain("text-sm");
      expect(span!.className).not.toContain("truncate");
    });

    it("passes the `title` prop through as the HTML title attribute", () => {
      const { container } = render(
        <DisplayText text="hello" mode="full" title="tooltip text" />,
      );
      const span = container.querySelector("span");
      expect(span!.getAttribute("title")).toBe("tooltip text");
    });
  });
});

describe("TextDisplay", () => {
  it("stringifies a string value and delegates to DisplayText (compact)", () => {
    const { container } = render(
      <TextDisplay {...makeTextProps("hello", "compact")} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent).toBe("hello");
    expect(span!.className).toContain("truncate");
    expect(span!.className).toContain("block");
  });

  it("stringifies a numeric value and delegates to DisplayText", () => {
    const { container } = render(
      <TextDisplay {...makeTextProps(42, "full")} />,
    );
    const span = container.querySelector("span");
    expect(span!.textContent).toBe("42");
    expect(span!.className).toContain("text-sm");
  });

  it("renders the muted dash for null value", () => {
    const { container } = render(<TextDisplay {...makeTextProps(null)} />);
    const span = container.querySelector("span");
    expect(span!.textContent).toBe("-");
    expect(span!.className).toContain("text-muted-foreground/50");
  });

  it("renders the muted dash for undefined value", () => {
    const { container } = render(<TextDisplay {...makeTextProps(undefined)} />);
    const span = container.querySelector("span");
    expect(span!.textContent).toBe("-");
    expect(span!.className).toContain("text-muted-foreground/50");
  });
});
