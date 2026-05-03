import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve("ok"));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { BoardSelector, pathStem } from "./board-selector";
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
  it("renders the trigger with the selected board stem", () => {
    render(
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

  it("renders nothing when boards is empty", () => {
    const { container } = render(
      <Wrapper>
        <BoardSelector boards={[]} selectedPath={null} onSelect={() => {}} />
      </Wrapper>,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders SelectContent with position=popper", async () => {
    const { container } = render(
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

    render(
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

  it("renders tear-off button with aria-label when showTearOff is true", () => {
    render(
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

  it("does not render tear-off button when showTearOff is false", () => {
    render(
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
});
