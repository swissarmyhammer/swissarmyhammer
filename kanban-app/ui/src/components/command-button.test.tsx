/**
 * Tests for `<CommandButton>` — the generic tab-button affordance that
 * renders any `tab_button`-tagged YAML CommandDef.
 *
 * The component reads `command.tab_button.icon`, looks it up in the
 * frontend icon registry, wraps a Radix `<Pressable>` for spatial-nav
 * reachability, and on activation either dispatches immediately (when
 * every param's value is resolved from `args | scope`) or opens a
 * `<CommandPopover>` for any `shape`-bearing param.
 */

import type React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, screen, act } from "@testing-library/react";

// Tauri mocks before any imports that pull command-scope.
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
import { CommandButton } from "./command-button";
import type { CommandDef } from "@/types/kanban";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

/**
 * Mount `<CommandButton>` inside the minimum provider stack required for
 * `<Pressable>` (which internally mounts a `<FocusScope>` against the
 * spatial-nav context).
 */
function renderButton(ui: React.ReactElement) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>{ui}</FocusLayer>
    </SpatialFocusProvider>,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("CommandButton", () => {
  it("renders_icon_from_tab_button_metadata — looks up the icon name in the registry", () => {
    const command: CommandDef = {
      id: "perspective.focusFilter",
      name: "Focus filter",
      tab_button: { icon: "filter" },
    };
    renderButton(
      <CommandButton
        command={command}
        surface="perspective_tab"
        surfaceId="p1"
      />,
    );

    const button = screen.getByRole("button", { name: "Focus filter" });
    // Lucide's Filter renders an <svg> with class names containing
    // `lucide-funnel` (the visual is a funnel — Filter is an alias).
    const svg = button.querySelector("svg");
    expect(svg).toBeTruthy();
    expect(svg?.getAttribute("class") ?? "").toMatch(/lucide-funnel/);
  });

  it("renders_fallback_icon_for_unknown_name — unknown icon renders HelpCircle without crashing", () => {
    const command: CommandDef = {
      id: "demo.unknown",
      name: "Unknown",
      tab_button: { icon: "no-such-icon" },
    };
    renderButton(
      <CommandButton
        command={command}
        surface="perspective_tab"
        surfaceId="p1"
      />,
    );

    const button = screen.getByRole("button", { name: "Unknown" });
    // Lucide's HelpCircle renders with the underlying name
    // `lucide-circle-question-mark`.
    const svg = button.querySelector("svg");
    expect(svg).toBeTruthy();
    expect(svg?.getAttribute("class") ?? "").toMatch(
      /lucide-circle-question-mark/,
    );
  });

  it("dispatches_immediately_when_no_pickable_params — click sends args + scope-resolved values via dispatch_command", async () => {
    const command: CommandDef = {
      id: "perspective.clearFilter",
      name: "Clear filter",
      tab_button: { icon: "filter" },
      params: [{ name: "perspective_id", from: "scope_chain" }],
    };
    renderButton(
      <CommandButton
        command={command}
        surface="perspective_tab"
        surfaceId="p1"
      />,
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Clear filter" }));
      await Promise.resolve();
    });

    // No popover should have opened — popover content has role=dialog.
    expect(screen.queryByRole("dialog")).toBeNull();

    // The backend dispatch_command should have been called with the command id.
    const dispatchCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "dispatch_command",
    );
    expect(dispatchCalls).toHaveLength(1);
    expect(dispatchCalls[0][1]).toMatchObject({
      cmd: "perspective.clearFilter",
    });
  });

  it("isActive_highlights_icon — isActive=true paints text-primary and fills the icon", () => {
    // Migrations rely on the `isActive` indicator producing the same
    // highlight the legacy `<FilterFocusButton>` / `<GroupPopoverButton>`
    // affordances render. Lock the visual contract: the button carries
    // `text-primary`, and the icon SVG has `fill="currentColor"` so it
    // adopts the primary accent.
    const command: CommandDef = {
      id: "perspective.focusFilter",
      name: "Focus filter",
      tab_button: { icon: "filter" },
    };
    renderButton(
      <CommandButton
        command={command}
        surface="perspective_tab"
        surfaceId="p1"
        isActive
      />,
    );

    const button = screen.getByRole("button", { name: "Focus filter" });
    expect(button.className).toMatch(/text-primary/);
    const svg = button.querySelector("svg");
    expect(svg).toBeTruthy();
    expect(svg?.getAttribute("fill")).toBe("currentColor");
  });

  it("isActive_false_does_not_highlight — default isActive renders the muted style", () => {
    // Contract counterpoint: with isActive omitted, the button must NOT
    // carry the primary highlight, and the icon's fill must be `none` so
    // it renders as the legacy muted glyph.
    const command: CommandDef = {
      id: "perspective.focusFilter",
      name: "Focus filter",
      tab_button: { icon: "filter" },
    };
    renderButton(
      <CommandButton
        command={command}
        surface="perspective_tab"
        surfaceId="p1"
      />,
    );

    const button = screen.getByRole("button", { name: "Focus filter" });
    expect(button.className).not.toMatch(/text-primary/);
    const svg = button.querySelector("svg");
    expect(svg?.getAttribute("fill")).toBe("none");
  });

  it("throws_when_surfaceId_is_empty — empty surfaceId would collide in spatial-nav moniker", () => {
    // Empty surfaceId is type-legal but produces an ambiguous moniker
    // (`${surface}.${command.id}:`). Two callers that both pass "" would
    // register at the same spatial-nav coordinate. The runtime guard
    // converts that silent collision into a visible crash.
    const command: CommandDef = {
      id: "perspective.focusFilter",
      name: "Focus filter",
      tab_button: { icon: "filter" },
    };
    // Suppress React's expected error log for the render failure.
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() =>
      renderButton(
        <CommandButton
          command={command}
          surface="perspective_tab"
          surfaceId=""
        />,
      ),
    ).toThrow(/surfaceId must be a non-empty string/);
    errorSpy.mockRestore();
  });

  it("opens_popover_when_command_has_pickable_param — click opens the CommandPopover", async () => {
    const command: CommandDef = {
      id: "perspective.setSort",
      name: "Set sort",
      tab_button: { icon: "arrow-up-down" },
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
    renderButton(
      <CommandButton
        command={command}
        surface="perspective_tab"
        surfaceId="p1"
      />,
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Set sort" }));
      await Promise.resolve();
    });

    // Popover content should be visible (Radix renders the content with role=dialog).
    expect(screen.getByRole("dialog")).toBeTruthy();

    // dispatch_command should NOT yet have been called — args still need picking.
    const dispatchCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "dispatch_command",
    );
    expect(dispatchCalls).toHaveLength(0);
  });
});
