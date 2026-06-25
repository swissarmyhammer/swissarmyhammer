/**
 * Tests for the pure tooltip-resolution logic extracted from
 * `createMentionTooltips`. CM6's `hoverTooltip` cannot fire in jsdom (no
 * layout → no `posAtCoords`), so the resolution path is exercised directly
 * via the exported `mentionTooltipAt`.
 */

import { describe, it, expect } from "vitest";
import { mentionTooltipAt, type MentionMeta } from "@/lib/cm-mention-tooltip";

/** A single-task meta map keyed on the 7-char short id. */
function taskMeta(): Map<string, MentionMeta> {
  return new Map<string, MentionMeta>([
    [
      "28rfp1r",
      {
        color: "00ff00",
        displayName: "28rfp1r",
        description: "Long Sentence-Like Task Title",
      },
    ],
  ]);
}

describe("mentionTooltipAt", () => {
  it("returns a payload whose dom shows both the short-id header and the title", () => {
    const meta = taskMeta();
    const result = mentionTooltipAt(
      "^28rfp1r",
      3,
      0,
      "^",
      "cm-task-tooltip",
      meta,
    );

    expect(result).not.toBeNull();
    const text = result!.dom.textContent ?? "";
    expect(text).toContain("^28rfp1r");
    expect(text).toContain("Long Sentence-Like Task Title");
  });

  it("returns null when the prefix does not match the mention", () => {
    const meta = taskMeta();
    const result = mentionTooltipAt(
      "^28rfp1r",
      3,
      0,
      "#",
      "cm-tag-tooltip",
      meta,
    );
    expect(result).toBeNull();
  });

  it("returns null when the position is off any known mention", () => {
    const meta = taskMeta();
    // Position 20 is past the end of "^28rfp1r" (length 8).
    const result = mentionTooltipAt(
      "^28rfp1r",
      20,
      0,
      "^",
      "cm-task-tooltip",
      meta,
    );
    expect(result).toBeNull();
  });
});
