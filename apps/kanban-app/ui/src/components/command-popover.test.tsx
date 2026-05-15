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
  it("renders_menu_buttons_for_single_enum_param — shows one button per option, no select", () => {
    // Single-enum-param commands use the one-click menu pattern: each
    // option is a clickable button (picking IS the action), there is no
    // native <select> and no Submit affordance. The pre-migration
    // counterpart of this test asserted on a <select> — that contract
    // has moved to the multi-param form branch.
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

    const { container } = render(
      <CommandPopover command={command} onCommit={() => {}} />,
    );

    // No native <select> in the one-click menu branch.
    expect(container.querySelector("select")).toBeNull();
    // The <ul> holding the options carries the param's `aria-label`.
    const menu = screen.getByLabelText("field");
    expect(menu.tagName).toBe("UL");
    // One button per backend-supplied option, labelled by `label`.
    expect(screen.getByRole("button", { name: "Title" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Created" })).toBeTruthy();
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

  it("commits_picked_values_via_oncommit_menu — clicking an option fires onCommit with the picked arg", async () => {
    // The single-enum-param case: one click on an option button commits
    // the picker bag — there is no intermediate Submit step.
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

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "A" }));
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

  it("submit_disabled_until_required_enum_param_is_picked — multi-param form gates submit on empty enum slots", async () => {
    // Multi-param commands keep the form + Submit pattern. The Submit
    // button stays disabled until every enum slot without `clear_command`
    // has a non-empty value — the backend would reject `{ field: "" }`
    // and the gating gives a better UX than dispatching garbage.
    const command: CommandDef = {
      id: "perspective.setSort",
      name: "Set sort",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [
            { value: "a", label: "A" },
            { value: "b", label: "B" },
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

    const submitBtn = screen.getByRole("button", {
      name: "Submit",
    }) as HTMLButtonElement;
    expect(submitBtn.disabled).toBe(true);

    // Clicking the disabled button must not invoke onCommit.
    await act(async () => {
      fireEvent.click(submitBtn);
    });
    expect(onCommit).not.toHaveBeenCalled();

    // Picking values for every enum slot re-enables the button.
    const fieldSelect = screen.getByLabelText("field") as HTMLSelectElement;
    const directionSelect = screen.getByLabelText(
      "direction",
    ) as HTMLSelectElement;
    await act(async () => {
      fireEvent.change(fieldSelect, { target: { value: "a" } });
    });
    // Still disabled — `direction` is still on the empty placeholder.
    expect(submitBtn.disabled).toBe(true);
    await act(async () => {
      fireEvent.change(directionSelect, { target: { value: "asc" } });
    });
    expect(submitBtn.disabled).toBe(false);
    await act(async () => {
      fireEvent.click(submitBtn);
    });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({ field: "a", direction: "asc" });
  });

  it("enum_param_with_empty_options_renders_empty_state — options: [] shows the No options placeholder, no buttons", () => {
    // The backend resolver supplied no options and no `clear_command`
    // sentinel — there is nothing the user can pick. The menu renders
    // the disabled placeholder rather than an empty <ul>, so the user
    // sees the popover opened but knows nothing is actionable.
    const command: CommandDef = {
      id: "perspective.setSort",
      name: "Set sort",
      params: [{ name: "field", shape: "enum", options: [] }],
    };
    const onCommit = vi.fn();

    render(<CommandPopover command={command} onCommit={onCommit} />);

    expect(screen.getByLabelText("field").textContent).toBe(
      "No options available",
    );
    // No buttons to click — onCommit cannot fire.
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("enum_param_with_no_options_field_renders_empty_state — undefined options shows the No options placeholder", () => {
    const command: CommandDef = {
      id: "perspective.setSort",
      name: "Set sort",
      params: [{ name: "field", shape: "enum" }],
    };

    render(<CommandPopover command={command} onCommit={() => {}} />);

    expect(screen.getByLabelText("field").textContent).toBe(
      "No options available",
    );
    expect(screen.queryByRole("button")).toBeNull();
  });

  // ---------------------------------------------------------------------------
  // One-click menu pattern for single-enum-param commands.
  //
  // When the command exposes a single pickable enum param, the popover
  // renders each option as a button — clicking commits immediately. There
  // is no Submit affordance: picking IS the action.
  //
  // Multi-param commands keep the form + Submit pattern (need to gather N
  // values before dispatching), and that contract is pinned by the
  // pre-existing `commits_picked_values_for_multi_param_command` test.
  // ---------------------------------------------------------------------------

  it("single_enum_param_click_dispatches_and_closes — clicking an option commits and renders no Submit", async () => {
    const command: CommandDef = {
      id: "perspective.setGroup",
      name: "Set group",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [
            { value: "option1-value", label: "Option 1" },
            { value: "option2-value", label: "Option 2" },
          ],
        },
      ],
    };
    const onCommit = vi.fn();

    render(<CommandPopover command={command} onCommit={onCommit} />);

    // No Submit button — picking IS the action.
    expect(screen.queryByRole("button", { name: "Submit" })).toBeNull();

    // Each option is rendered as a clickable button labelled by `label`.
    const optionButton = screen.getByRole("button", { name: "Option 2" });
    await act(async () => {
      fireEvent.click(optionButton);
    });

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({ field: "option2-value" });
  });

  it("single_enum_param_renders_options_as_buttons_not_select — no native select element", () => {
    const command: CommandDef = {
      id: "perspective.setGroup",
      name: "Set group",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [
            { value: "a", label: "Alpha" },
            { value: "b", label: "Beta" },
          ],
        },
      ],
    };

    const { container } = render(
      <CommandPopover command={command} onCommit={() => {}} />,
    );

    // The one-click menu pattern does NOT render a <select>. Each option
    // is its own button so a single click commits.
    expect(container.querySelector("select")).toBeNull();
    expect(screen.getByRole("button", { name: "Alpha" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Beta" })).toBeTruthy();
  });

  it("clear_command_sentinel_click_dispatches_redirect — clicking (none) commits empty-string sentinel", async () => {
    // When the enum param carries `clear_command`, the popover prepends a
    // "(none)" menu item. Clicking it commits `{ <paramName>: "" }` —
    // `<CommandButton>`'s handleCommit then redirects to `clear_command`.
    // Here we pin the picker-level contract: the click commits the
    // sentinel value.
    const command: CommandDef = {
      id: "perspective.group",
      name: "Group By",
      params: [
        {
          name: "group",
          shape: "enum",
          options: [{ value: "status", label: "Status" }],
          clear_command: "perspective.clearGroup",
        },
      ],
    };
    const onCommit = vi.fn();

    render(<CommandPopover command={command} onCommit={onCommit} />);

    // The clear sentinel renders as a button labelled "(none)".
    const noneButton = screen.getByRole("button", { name: "(none)" });
    await act(async () => {
      fireEvent.click(noneButton);
    });

    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({ group: "" });
  });

  it("multi_enum_param_click_does_not_dispatch_without_submit — form pattern preserved for N-param commands", async () => {
    // Multi-param commands need to gather N values before dispatch, so the
    // form + Submit pattern is retained. Picking a single option in one of
    // the param fields must NOT fire onCommit.
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

    // Multi-param keeps the <select> + Submit pattern.
    const fieldSelect = screen.getByLabelText("field") as HTMLSelectElement;
    const directionSelect = screen.getByLabelText(
      "direction",
    ) as HTMLSelectElement;
    await act(async () => {
      fireEvent.change(fieldSelect, { target: { value: "title" } });
    });
    await act(async () => {
      fireEvent.change(directionSelect, { target: { value: "asc" } });
    });

    // Picking values has NOT dispatched yet — Submit is still required.
    expect(onCommit).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Submit" }));
    });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith({
      field: "title",
      direction: "asc",
    });
  });

  it("single_enum_param_with_text_param_keeps_form — mixed-shape commands use the form pattern", () => {
    // A command with one enum + one text param is NOT a single-enum-param
    // command — the form pattern applies because we need to gather both
    // values before dispatch.
    const command: CommandDef = {
      id: "perspective.renameAndGroup",
      name: "Rename and group",
      params: [
        {
          name: "field",
          shape: "enum",
          options: [{ value: "a", label: "Alpha" }],
        },
        { name: "newName", shape: "text" },
      ],
    };

    render(<CommandPopover command={command} onCommit={() => {}} />);

    // Form pattern: a select + a text input + a Submit button.
    expect(screen.getByLabelText("field").tagName).toBe("SELECT");
    expect(screen.getByLabelText("newName").tagName).toBe("INPUT");
    expect(screen.getByRole("button", { name: "Submit" })).toBeTruthy();
  });
});
