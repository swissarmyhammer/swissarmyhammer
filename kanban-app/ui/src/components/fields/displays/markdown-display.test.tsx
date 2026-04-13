/**
 * Tests for MarkdownDisplay.
 *
 * The full-mode display now mounts CM6 (via TextViewer) with the markdown
 * language, mention extensions, and the task-list checkbox plugin. These
 * tests verify:
 *
 *   - Checkbox widgets render for `- [ ]` / `- [x]` patterns
 *   - Clicking a widget's checkbox fires `onCommit` with the toggled source
 *   - Click events stop propagating so ancestor onClick handlers do not fire
 *   - Mention pills render via CM6 widgets showing clipped display names
 *   - Compact mode still emits truncated plain text (no CM6)
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri / plugin mocks — declared before importing the component under test.
// useMentionExtensions pulls in @tauri-apps/api/core for search_mentions.
// ---------------------------------------------------------------------------

const mockInvoke = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (..._args: any[]): Promise<unknown> => Promise.resolve(undefined),
);

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Mutable mocks for schema + entity store. Tests that exercise mention
// widgets set these before rendering; checkbox-only tests leave them empty.
// ---------------------------------------------------------------------------

import type { Entity } from "@/types/kanban";
import type { MentionableType } from "@/lib/schema-context";

let mockEntities: Record<string, Entity[]> = {};
let mockMentionableTypes: MentionableType[] = [];

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
    mentionableTypes: mockMentionableTypes,
    loading: false,
  }),
  useSchemaOptional: () => undefined,
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: (type: string) => mockEntities[type] ?? [],
    getEntity: (type: string, id: string) =>
      mockEntities[type]?.find((e) => e.id === id),
  }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => null,
}));

// ---------------------------------------------------------------------------

import { MarkdownDisplay } from "./markdown-display";
import type { DisplayProps } from "./text-display";

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

/** Flush microtasks and pending CM6 mount effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

describe("MarkdownDisplay — compact mode", () => {
  beforeEach(() => {
    mockEntities = {};
    mockMentionableTypes = [];
  });

  it("renders truncated plain text (no CM6)", () => {
    const md = "hello world";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "compact")} />,
    );

    const span = container.querySelector("span.truncate");
    expect(span).toBeTruthy();
    expect(span?.textContent).toBe("hello world");

    // Compact mode must not mount a CM6 editor.
    expect(container.querySelector(".cm-editor")).toBeFalsy();
  });

  it("renders a placeholder when empty", () => {
    const { container } = render(
      <MarkdownDisplay {...makeProps("", "compact")} />,
    );
    expect(container.textContent).toBe("-");
  });
});

describe("MarkdownDisplay — full mode empty state", () => {
  beforeEach(() => {
    mockEntities = {};
    mockMentionableTypes = [];
  });

  it("renders 'Empty' placeholder when value is empty", () => {
    const { container } = render(<MarkdownDisplay {...makeProps("", "full")} />);
    expect(container.textContent).toBe("Empty");
  });
});

describe("MarkdownDisplay — checkbox toggling", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockEntities = {};
    mockMentionableTypes = [];
  });

  it("toggles unchecked → checked and calls onCommit", async () => {
    const onCommit = vi.fn();
    const md = "- [ ] task\n- [x] done";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full", onCommit)} />,
    );
    await flush();

    const checkboxes = container.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(checkboxes).toHaveLength(2);

    // Click the first checkbox — widget computes sourceIndex=0 and calls
    // onCommit with the toggled markdown source.
    checkboxes[0].click();

    expect(onCommit).toHaveBeenCalledWith("- [x] task\n- [x] done");
  });

  it("toggles checked → unchecked and calls onCommit", async () => {
    const onCommit = vi.fn();
    const md = "- [ ] task\n- [x] done";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full", onCommit)} />,
    );
    await flush();

    const checkboxes = container.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    checkboxes[1].click();

    expect(onCommit).toHaveBeenCalledWith("- [ ] task\n- [ ] done");
  });

  it("toggles only the 3rd of 5 checkboxes", async () => {
    const onCommit = vi.fn();
    const md = ["- [ ] a", "- [ ] b", "- [ ] c", "- [x] d", "- [ ] e"].join(
      "\n",
    );
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full", onCommit)} />,
    );
    await flush();

    const checkboxes = container.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(checkboxes).toHaveLength(5);

    checkboxes[2].click();

    expect(onCommit).toHaveBeenCalledWith(
      ["- [ ] a", "- [ ] b", "- [x] c", "- [x] d", "- [ ] e"].join("\n"),
    );
  });

  it("stopPropagation prevents parent onClick", async () => {
    const onCommit = vi.fn();
    const parentClick = vi.fn();
    const md = "- [ ] task";
    const { container } = render(
      <div onClick={parentClick}>
        <MarkdownDisplay {...makeProps(md, "full", onCommit)} />
      </div>,
    );
    await flush();

    const checkbox = container.querySelector<HTMLInputElement>(
      'input[type="checkbox"]',
    )!;
    checkbox.click();

    expect(parentClick).not.toHaveBeenCalled();
  });

  it("does not call onCommit when onCommit is not provided", async () => {
    const md = "- [ ] task";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full")} />,
    );
    await flush();

    const checkbox = container.querySelector<HTMLInputElement>(
      'input[type="checkbox"]',
    )!;
    // Must not throw when onCommit is absent.
    expect(() => checkbox.click()).not.toThrow();
    // And avoid a lint warning about fireEvent being unused.
    fireEvent.click(checkbox);
  });
});

describe("MarkdownDisplay — mention widgets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockResolvedValue(undefined);
    mockEntities = {
      tag: [
        {
          id: "tag-1",
          entity_type: "tag",
          moniker: "tag:tag-1",
          fields: { tag_name: "Bug Fix", color: "ff0000" },
        },
      ],
      project: [
        {
          id: "p1",
          entity_type: "project",
          moniker: "project:p1",
          fields: { name: "My Project", color: "6366f1" },
        },
      ],
      task: [
        {
          id: "t99",
          entity_type: "task",
          moniker: "task:t99",
          fields: { title: "Task Title", color: "00ff00" },
        },
      ],
    };
    mockMentionableTypes = [
      { entityType: "tag", prefix: "#", displayField: "tag_name" },
      { entityType: "project", prefix: "$", displayField: "name" },
      { entityType: "task", prefix: "^", displayField: "title" },
    ];
  });

  it("renders tag, project, and task mentions as CM6 pill widgets with display names", async () => {
    // Three known mentions on a single line (not a heading — no leading
    // `# ` with whitespace). Decoration widgets should replace the raw
    // slugs with the entity display names, clipped via `clipDisplayName`.
    const md = "intro\n\n#bug-fix $my-project ^task-title";
    const { container } = render(
      <MarkdownDisplay {...makeProps(md, "full")} />,
    );
    await flush();

    const pills = container.querySelectorAll(".cm-mention-pill");
    const texts = Array.from(pills).map((p) => p.textContent);
    expect(texts).toContain("#Bug Fix");
    expect(texts).toContain("$My Project");
    expect(texts).toContain("^Task Title");
  });
});
