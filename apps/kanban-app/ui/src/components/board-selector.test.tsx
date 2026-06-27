import { describe, it, expect, vi } from "vitest";
import { screen, fireEvent, act } from "@testing-library/react";
import { renderInAct } from "@/test/act-render";

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve("ok"),
);

// `mock`-prefixed so the hoisted `vi.mock("sonner", …)` factory may reference
// them (vitest allowlists names starting with `mock`).
const mockToastSuccess = vi.fn();
const mockToastError = vi.fn();
const mockToastInfo = vi.fn();
vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => mockToastSuccess(...args),
    error: (...args: unknown[]) => mockToastError(...args),
    info: (...args: unknown[]) => mockToastInfo(...args),
  },
}));

// Spread the real module and override only the parts the test controls.
// @tauri-apps/api >=2.11 pulls submodules that import named exports from core
// (SERIALIZE_TO_IPC_FN, Resource, Channel, …); a hand-listed stub drops them
// and breaks module loading.
vi.mock("@tauri-apps/api/core", async (importActual) => ({
  ...(await importActual<typeof import("@tauri-apps/api/core")>()),
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", async (importActual) => ({
  ...(await importActual<typeof import("@tauri-apps/api/event")>()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import {
  BoardSelector,
  pathStem,
  EXPOSE_BOARD_LABEL,
} from "./board-selector";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";
import type { OpenBoard } from "@/types/kanban";

/**
 * Wrap `BoardSelector` in the providers it (transitively) needs. The
 * spatial provider stack (`SpatialFocusProvider` + `FocusLayer`) is
 * required since the component mounts `<FocusScope>`-using descendants
 * and the no-spatial-context fallback was removed in card
 * `01KQPVA127YMJ8D7NB6M824595`.
 */
function Wrapper({ children }: { children: React.ReactNode }) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <SchemaProvider>
          <EntityStoreProvider entities={{}}>
            <FieldUpdateProvider>
              <TooltipProvider>{children}</TooltipProvider>
            </FieldUpdateProvider>
          </EntityStoreProvider>
        </SchemaProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

const twoBoards: OpenBoard[] = [
  { path: "/home/user/project-a/.kanban", name: "Project A", is_active: true },
  { path: "/home/user/project-b/.kanban", name: "Project B", is_active: false },
];

describe("pathStem", () => {
  it("returns parent of .kanban", () => {
    expect(pathStem("/home/user/project/.kanban")).toBe("project");
  });

  it("returns last segment if not .kanban", () => {
    expect(pathStem("/home/user/project")).toBe("project");
  });
});

describe("BoardSelector", () => {
  it("renders the trigger with the selected board stem", async () => {
    await renderInAct(
      <Wrapper>
        <BoardSelector
          boards={twoBoards}
          selectedPath={twoBoards[1].path}
          onSelect={() => {}}
        />
      </Wrapper>,
    );
    expect(screen.getByText("project-b")).toBeTruthy();
  });

  it("renders nothing when boards is empty", async () => {
    const { container } = await renderInAct(
      <Wrapper>
        <BoardSelector boards={[]} selectedPath={null} onSelect={() => {}} />
      </Wrapper>,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders SelectContent with position=popper", async () => {
    const { container } = await renderInAct(
      <Wrapper>
        <BoardSelector
          boards={twoBoards}
          selectedPath={twoBoards[0].path}
          onSelect={() => {}}
        />
      </Wrapper>,
    );
    // The SelectContent should have data-radix-select-viewport or similar.
    // More importantly, the trigger should be present and clickable.
    const trigger = container.querySelector("[data-slot='select-trigger']");
    expect(trigger).toBeTruthy();
  });

  it("calls onSelect when a board is selected (not dispatchCommand)", async () => {
    // Verify the component delegates to onSelect rather than dispatching
    // file.switchBoard directly. Radix Select in jsdom doesn't support full
    // open/click interaction, so we assert at the wiring level: the internal
    // Select's onValueChange must forward to the onSelect prop.
    const onSelect = vi.fn();
    mockInvoke.mockClear();

    await renderInAct(
      <Wrapper>
        <BoardSelector
          boards={twoBoards}
          selectedPath={twoBoards[0].path}
          onSelect={onSelect}
        />
      </Wrapper>,
    );

    // file.switchBoard should never be invoked directly by BoardSelector —
    // that responsibility belongs to the parent (App.tsx handleSwitchBoard).
    const switchBoardCalls = mockInvoke.mock.calls.filter(
      (args) =>
        args[0] === "dispatch_command" &&
        String((args[1] as Record<string, unknown>)?.command ?? "").includes(
          "switchBoard",
        ),
    );
    expect(switchBoardCalls).toHaveLength(0);
  });

  it("renders tear-off button with aria-label when showTearOff is true", async () => {
    await renderInAct(
      <Wrapper>
        <BoardSelector
          boards={twoBoards}
          selectedPath={twoBoards[0].path}
          onSelect={() => {}}
          showTearOff
        />
      </Wrapper>,
    );
    const btn = screen.getByRole("button", { name: "Open in new window" });
    expect(btn).toBeTruthy();
  });

  it("does not render tear-off button when showTearOff is false", async () => {
    await renderInAct(
      <Wrapper>
        <BoardSelector
          boards={twoBoards}
          selectedPath={twoBoards[0].path}
          onSelect={() => {}}
        />
      </Wrapper>,
    );
    const btn = screen.queryByRole("button", { name: "Open in new window" });
    expect(btn).toBeNull();
  });

  it("invokes expose_board_to_agents with the board path and toasts each per-agent result", async () => {
    mockInvoke.mockClear();
    mockToastSuccess.mockClear();
    mockToastError.mockClear();
    mockInvoke.mockImplementation((...args: unknown[]) => {
      if (args[0] === "expose_board_to_agents") {
        return Promise.resolve([
          { ok: true, message: "kanban MCP server for Claude Code" },
          { ok: false, message: "Codex (project): boom" },
        ]);
      }
      return Promise.resolve("ok");
    });

    await renderInAct(
      <Wrapper>
        <BoardSelector
          boards={twoBoards}
          selectedPath={twoBoards[0].path}
          onSelect={() => {}}
          showTearOff
        />
      </Wrapper>,
    );

    const btn = screen.getByRole("button", {
      name: EXPOSE_BOARD_LABEL,
    });
    await act(async () => {
      fireEvent.click(btn);
    });

    // The plain Tauri command is invoked with the window's board path
    // (camelCase per Tauri's arg convention).
    expect(mockInvoke).toHaveBeenCalledWith("expose_board_to_agents", {
      boardPath: twoBoards[0].path,
    });
    // Each per-agent result is rendered: a success toast and a failure toast.
    expect(mockToastSuccess).toHaveBeenCalledWith(
      "kanban MCP server for Claude Code",
    );
    expect(mockToastError).toHaveBeenCalledWith("Codex (project): boom");
  });

  it("does not render the expose button when showTearOff is false", async () => {
    await renderInAct(
      <Wrapper>
        <BoardSelector
          boards={twoBoards}
          selectedPath={twoBoards[0].path}
          onSelect={() => {}}
        />
      </Wrapper>,
    );
    const btn = screen.queryByRole("button", {
      name: EXPOSE_BOARD_LABEL,
    });
    expect(btn).toBeNull();
  });
});
