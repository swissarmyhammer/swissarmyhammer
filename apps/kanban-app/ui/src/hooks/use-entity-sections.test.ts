import { describe, it, expect } from "vitest";
import { resolveEntitySections } from "./use-entity-sections";
import type { FieldDef, SectionDef } from "@/types/kanban";

/** Build a minimal FieldDef with a name and `section` value. */
function field(name: string, section?: string): FieldDef {
  return {
    id: `id-${name}`,
    name,
    type: { kind: "text" },
    editor: "markdown",
    display: "text",
    section,
  };
}

describe("resolveEntitySections", () => {
  it("falls back to header/body/footer when entity omits sections", () => {
    const fields = [
      field("title", "header"),
      field("body", "body"),
      field("attachments", "footer"),
    ];
    const resolved = resolveEntitySections(undefined, fields);
    expect(resolved.map((s) => s.def.id)).toEqual(["header", "body", "footer"]);
    expect(resolved[0].fields.map((f) => f.name)).toEqual(["title"]);
    expect(resolved[1].fields.map((f) => f.name)).toEqual(["body"]);
    expect(resolved[2].fields.map((f) => f.name)).toEqual(["attachments"]);
  });

  it("falls back to default layout when sections is an empty array", () => {
    const resolved = resolveEntitySections([], [field("title", "header")]);
    expect(resolved.map((s) => s.def.id)).toEqual(["header", "body", "footer"]);
  });

  it("uses declared section order and labels", () => {
    const sections: SectionDef[] = [
      { id: "header", on_card: true },
      { id: "body" },
      { id: "dates", label: "Dates", on_card: true },
      { id: "system", label: "System" },
      { id: "footer" },
    ];
    const fields = [
      field("title", "header"),
      field("description", "body"),
      field("due", "dates"),
      field("scheduled", "dates"),
      field("created", "system"),
      field("attachments", "footer"),
    ];
    const resolved = resolveEntitySections(sections, fields);
    expect(resolved.map((s) => s.def.id)).toEqual([
      "header",
      "body",
      "dates",
      "system",
      "footer",
    ]);
    expect(resolved[2].def.label).toBe("Dates");
    expect(resolved[3].def.label).toBe("System");
    expect(resolved[2].fields.map((f) => f.name)).toEqual(["due", "scheduled"]);
    expect(resolved[3].fields.map((f) => f.name)).toEqual(["created"]);
  });

  it("drops fields with section: hidden", () => {
    const sections: SectionDef[] = [{ id: "header" }, { id: "body" }];
    const fields = [
      field("title", "header"),
      field("position_column", "hidden"),
      field("notes", "body"),
    ];
    const resolved = resolveEntitySections(sections, fields);
    const names = resolved.flatMap((s) => s.fields.map((f) => f.name));
    expect(names).toEqual(["title", "notes"]);
  });

  it("routes fields with an unknown section id into the body fallback", () => {
    const sections: SectionDef[] = [
      { id: "header" },
      { id: "body" },
      { id: "footer" },
    ];
    const fields = [
      field("title", "header"),
      field("orphan", "not-a-real-section"),
      field("notes", "body"),
    ];
    const resolved = resolveEntitySections(sections, fields);
    const body = resolved.find((s) => s.def.id === "body")!;
    // Insertion order is preserved; `orphan` arrives first, `notes` second.
    expect(body.fields.map((f) => f.name)).toEqual(["orphan", "notes"]);
  });

  it("treats absent section on a field as body", () => {
    const sections: SectionDef[] = [{ id: "header" }, { id: "body" }];
    const fields = [field("title", "header"), field("unsectioned", undefined)];
    const resolved = resolveEntitySections(sections, fields);
    const body = resolved.find((s) => s.def.id === "body")!;
    expect(body.fields.map((f) => f.name)).toEqual(["unsectioned"]);
  });

  it("returns empty buckets for sections with no fields", () => {
    const sections: SectionDef[] = [
      { id: "header" },
      { id: "body" },
      { id: "dates", label: "Dates" },
    ];
    const fields = [field("title", "header"), field("notes", "body")];
    const resolved = resolveEntitySections(sections, fields);
    const dates = resolved.find((s) => s.def.id === "dates")!;
    expect(dates.fields).toEqual([]);
  });

  it("falls back to the first non-header/footer section when body is not declared", () => {
    const sections: SectionDef[] = [
      { id: "header" },
      { id: "main" },
      { id: "footer" },
    ];
    const fields = [
      field("title", "header"),
      field("orphan", "unknown"),
      field("attached", "footer"),
    ];
    const resolved = resolveEntitySections(sections, fields);
    const main = resolved.find((s) => s.def.id === "main")!;
    expect(main.fields.map((f) => f.name)).toEqual(["orphan"]);
  });
});
