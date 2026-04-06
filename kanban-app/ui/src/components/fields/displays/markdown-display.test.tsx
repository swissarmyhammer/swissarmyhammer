import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { MarkdownDisplay } from "./markdown-display";
import type { DisplayProps } from "./text-display";

/**
 * Minimal wrapper that provides the React contexts MarkdownDisplay needs.
 * MarkdownFull uses useSchema and useEntityStore — we mock them at the module level.
 */

// Mock schema context — MarkdownFull calls useSchema().mentionableTypes
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({ mentionableTypes: [] }),
  useSchemaOptional: () => undefined,
}));

// Mock entity store context — MarkdownFull calls useEntityStore().getEntities
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

function makeProps(
  value: unknown,
  mode: "compact" | "full" = "full",
  onCommit?: (value: string) => void,
) {
  return {
    field: {
      id: "f1",
      name: "body",
      type: { kind: "text" },
    } as DisplayProps["field"],
    value,
    entity: { entity_type: "task", id: "t1", moniker: "task:t1", fields: {} },
    mode,
    onCommit,
  };
}

describe("MarkdownDisplay checkbox toggling", () => {
  it("toggles unchecked → checked and calls onCommit", () => {
    const onCommit = vi.fn();
    const md = "- [ ] task\n- [x] done";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full", onCommit)} />,
    );

    const checkboxes = container.querySelectorAll('input[type="checkbox"]');
    expect(checkboxes).toHaveLength(2);

    // Click first checkbox (unchecked → checked)
    fireEvent.click(checkboxes[0]);
    fireEvent.change(checkboxes[0], { target: { checked: true } });

    expect(onCommit).toHaveBeenCalledWith("- [x] task\n- [x] done");
  });

  it("toggles checked → unchecked and calls onCommit", () => {
    const onCommit = vi.fn();
    const md = "- [ ] task\n- [x] done";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full", onCommit)} />,
    );

    const checkboxes = container.querySelectorAll('input[type="checkbox"]');

    // Click second checkbox (checked → unchecked)
    fireEvent.click(checkboxes[1]);
    fireEvent.change(checkboxes[1], { target: { checked: false } });

    expect(onCommit).toHaveBeenCalledWith("- [ ] task\n- [ ] done");
  });

  it("toggles only the 3rd of 5 checkboxes", () => {
    const onCommit = vi.fn();
    const md = ["- [ ] a", "- [ ] b", "- [ ] c", "- [x] d", "- [ ] e"].join(
      "\n",
    );
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full", onCommit)} />,
    );

    const checkboxes = container.querySelectorAll('input[type="checkbox"]');
    expect(checkboxes).toHaveLength(5);

    // Click the 3rd checkbox
    fireEvent.click(checkboxes[2]);
    fireEvent.change(checkboxes[2], { target: { checked: true } });

    expect(onCommit).toHaveBeenCalledWith(
      ["- [ ] a", "- [ ] b", "- [x] c", "- [x] d", "- [ ] e"].join("\n"),
    );
  });

  it("stopPropagation prevents parent onClick", () => {
    const onCommit = vi.fn();
    const parentClick = vi.fn();
    const md = "- [ ] task";
    const { container } = render(
      <div onClick={parentClick}>
        <MarkdownDisplay {...makeProps(md, "full", onCommit)} />
      </div>,
    );

    const checkbox = container.querySelector('input[type="checkbox"]')!;
    fireEvent.click(checkbox);

    expect(parentClick).not.toHaveBeenCalled();
  });

  it("does not call onCommit when onCommit is not provided", () => {
    const md = "- [ ] task";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full")} />,
    );

    const checkbox = container.querySelector('input[type="checkbox"]')!;
    // Should not throw
    fireEvent.change(checkbox, { target: { checked: true } });
  });
});
