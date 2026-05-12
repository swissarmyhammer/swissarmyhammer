/**
 * Tests for `<CommandPopover>` — the picker form rendered inside the
 * popover anchored to a `<CommandButton>`. Renders one field per
 * `shape`-bearing param and calls `onCommit(args)` on submit.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, screen, act } from "@testing-library/react";

// Tauri mocks — the popover doesn't dispatch on its own, but the filter
// expression branch transitively pulls command-scope (which loads the
// tauri invoke shim). Mock to keep the surface clean.
const mockInvoke = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (..._args: any[]): Promise<unknown> => Promise.resolve(null),
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
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({ mentionableTypes: [] }),
}));
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

import { CommandPopover } from "./command-popover";
import type { CommandDef } from "@/types/kanban";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("CommandPopover", () => {
  it("renders_select_for_enum_param_with_options — shows a select with both options", () => {
    const command: CommandDef = {
      id: "perspective.setGroup",
      name: "Set group",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [
            { value: "title", label: "Title" },
            { value: "created", label: "Created" },
          ],
        },
      ],
    };

    render(<CommandPopover command={command} onCommit={() => {}} />);

    const select = screen.getByLabelText("field") as HTMLSelectElement;
    expect(select.tagName).toBe("SELECT");
    expect(select.disabled).toBe(false);
    // Options including a leading "Pick…" placeholder so the user must
    // make a choice before submit — and the two backend-supplied options.
    const labels = Array.from(select.options).map((o) => o.textContent);
    expect(labels).toContain("Title");
    expect(labels).toContain("Created");
  });

  it("renders_text_input_for_text_param — shows a text input", () => {
    const command: CommandDef = {
      id: "perspective.setName",
      name: "Set name",
      params: [{ name: "name", shape: "text" }],
    };

    render(<CommandPopover command={command} onCommit={() => {}} />);

    const input = screen.getByLabelText("name") as HTMLInputElement;
    expect(input.tagName).toBe("INPUT");
    expect(input.type).toBe("text");
  });

  it("renders_filter_editor_for_expression_param — mounts a CodeMirror editor", () => {
    const command: CommandDef = {
      id: "perspective.setFilter",
      name: "Set filter",
      params: [{ name: "filter", shape: "expression" }],
    };

    const { container } = render(
      <CommandPopover command={command} onCommit={() => {}} />,
    );

    // CM6 renders a div.cm-editor when the EditorView mounts.
    expect(container.querySelector(".cm-editor")).toBeTruthy();
  });

  it("commits_picked_values_via_oncommit — submit fires onCommit with picked args", async () => {
    const command: CommandDef = {
      id: "perspective.setGroup",
      name: "Set group",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [
            { value: "a", label: "A" },
            { value: "b", label: "B" },
          ],
        },
      ],
    };
    const onCommit = vi.fn();

    render(<CommandPopover command={command} onCommit={onCommit} />);

    const select = screen.getByLabelText("field") as HTMLSelectElement;
    await act(async () => {
      fireEvent.change(select, { target: { value: "a" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Submit" }));
    });

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({ field: "a" });
  });

  it("commits_picked_values_for_multi_param_command — submit collects every shape-bearing slot", async () => {
    // Mirrors the eventual `perspective.sort.set` shape — field (enum) +
    // direction (enum). Locks the contract that the picker bag carries
    // every param's value through `onCommit`, not just the first one.
    const command: CommandDef = {
      id: "perspective.setSort",
      name: "Set sort",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [
            { value: "title", label: "Title" },
            { value: "created", label: "Created" },
          ],
        },
        {
          name: "direction",
          shape: "enum",
          options: [
            { value: "asc", label: "Ascending" },
            { value: "desc", label: "Descending" },
          ],
        },
      ],
    };
    const onCommit = vi.fn();

    render(<CommandPopover command={command} onCommit={onCommit} />);

    const fieldSelect = screen.getByLabelText("field") as HTMLSelectElement;
    const directionSelect = screen.getByLabelText(
      "direction",
    ) as HTMLSelectElement;
    await act(async () => {
      fireEvent.change(fieldSelect, { target: { value: "title" } });
    });
    await act(async () => {
      fireEvent.change(directionSelect, { target: { value: "desc" } });
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Submit" }));
    });

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({
      field: "title",
      direction: "desc",
    });
  });

  it("submit_disabled_until_required_enum_param_is_picked — empty enum slot gates submit", async () => {
    // Enum params start at "" (the "Pick…" placeholder). The backend
    // would reject `{ field: "" }`; the form should not let the dispatch
    // happen in the first place.
    const command: CommandDef = {
      id: "perspective.setGroup",
      name: "Set group",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [
            { value: "a", label: "A" },
            { value: "b", label: "B" },
          ],
        },
      ],
    };
    const onCommit = vi.fn();

    render(<CommandPopover command={command} onCommit={onCommit} />);

    const submitBtn = screen.getByRole("button", {
      name: "Submit",
    }) as HTMLButtonElement;
    expect(submitBtn.disabled).toBe(true);

    // Clicking the disabled button must not invoke onCommit.
    await act(async () => {
      fireEvent.click(submitBtn);
    });
    expect(onCommit).not.toHaveBeenCalled();

    // Picking a value re-enables the button and lets submit through.
    const select = screen.getByLabelText("field") as HTMLSelectElement;
    await act(async () => {
      fireEvent.change(select, { target: { value: "a" } });
    });
    expect(submitBtn.disabled).toBe(false);
    await act(async () => {
      fireEvent.click(submitBtn);
    });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({ field: "a" });
  });

  it("enum_param_with_empty_options_disables_the_field — options: [] renders disabled select", () => {
    const command: CommandDef = {
      id: "perspective.setSort",
      name: "Set sort",
      params: [{ name: "field", shape: "enum", options: [] }],
    };

    render(<CommandPopover command={command} onCommit={() => {}} />);

    const select = screen.getByLabelText("field") as HTMLSelectElement;
    expect(select.disabled).toBe(true);
  });

  it("enum_param_with_no_options_field_disables_the_field — undefined options renders disabled select", () => {
    const command: CommandDef = {
      id: "perspective.setSort",
      name: "Set sort",
      params: [{ name: "field", shape: "enum" }],
    };

    render(<CommandPopover command={command} onCommit={() => {}} />);

    const select = screen.getByLabelText("field") as HTMLSelectElement;
    expect(select.disabled).toBe(true);
  });
});
