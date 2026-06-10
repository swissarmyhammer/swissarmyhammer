import { describe, it, expect } from "vitest";
import { icons } from "lucide-react";
import { iconByName } from "./icon-name";

describe("iconByName", () => {
  it("resolves a single-word icon name to the matching lucide component", () => {
    expect(iconByName("table")).toBe(icons.Table);
    expect(iconByName("kanban")).toBe(icons.Kanban);
  });

  it("resolves a multi-word kebab-case icon name to the PascalCase lucide component", () => {
    expect(iconByName("file-text")).toBe(icons.FileText);
    expect(iconByName("arrow-up-down")).toBe(icons.ArrowUpDown);
  });

  it("returns null when the name does not resolve to a lucide component", () => {
    expect(iconByName("not-a-real-icon")).toBeNull();
  });

  it("returns null for an empty, null, or undefined name", () => {
    expect(iconByName("")).toBeNull();
    expect(iconByName(null)).toBeNull();
    expect(iconByName(undefined)).toBeNull();
  });
});
