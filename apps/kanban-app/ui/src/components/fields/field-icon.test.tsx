import { describe, it, expect } from "vitest";
import { FileText, Users, Tag, Palette } from "lucide-react";
import { fieldIcon } from "./field-icon";
import type { FieldDef } from "@/types/kanban";

/**
 * Build a minimal FieldDef fixture with an optional icon override. The other
 * required fields are stubbed with sensible defaults since `fieldIcon` only
 * inspects the `icon` property.
 */
function makeField(icon?: string): FieldDef {
  return {
    id: "f",
    name: "field",
    type: { kind: "text" },
    icon,
  };
}

describe("fieldIcon", () => {
  it("returns null when field has no icon property", () => {
    expect(fieldIcon(makeField(undefined))).toBeNull();
  });

  it("resolves a single-word icon name to the matching lucide component", () => {
    expect(fieldIcon(makeField("users"))).toBe(Users);
    expect(fieldIcon(makeField("tag"))).toBe(Tag);
    expect(fieldIcon(makeField("palette"))).toBe(Palette);
  });

  it("resolves a multi-word kebab-case icon name to PascalCase lucide component", () => {
    expect(fieldIcon(makeField("file-text"))).toBe(FileText);
  });

  it("returns null when the icon name does not resolve to a lucide component", () => {
    expect(fieldIcon(makeField("not-a-real-icon"))).toBeNull();
    expect(fieldIcon(makeField("xyz"))).toBeNull();
  });

  it("returns null for an empty string icon", () => {
    expect(fieldIcon(makeField(""))).toBeNull();
  });
});
