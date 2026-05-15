import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { Filter, Group, Plus, ArrowUpDown, HelpCircle } from "lucide-react";
import { commandIconFor } from "./command-icon-registry";

describe("commandIconFor", () => {
  it("returns the Filter icon for the 'filter' name", () => {
    const Icon = commandIconFor("filter");
    expect(Icon).toBe(Filter);
  });

  it("returns the Group icon for the 'group' name", () => {
    const Icon = commandIconFor("group");
    expect(Icon).toBe(Group);
  });

  it("returns the Plus icon for the 'plus' name", () => {
    const Icon = commandIconFor("plus");
    expect(Icon).toBe(Plus);
  });

  it("returns the ArrowUpDown icon for the 'arrow-up-down' name (kebab-case)", () => {
    const Icon = commandIconFor("arrow-up-down");
    expect(Icon).toBe(ArrowUpDown);
  });

  it("returns HelpCircle as the fallback for an unknown name", () => {
    const Icon = commandIconFor("no-such-icon");
    expect(Icon).toBe(HelpCircle);
  });

  it("renders the resolved icon as a React component", () => {
    const Icon = commandIconFor("filter");
    const { container } = render(<Icon data-testid="rendered-icon" />);
    expect(container.querySelector("svg")).toBeTruthy();
  });
});
