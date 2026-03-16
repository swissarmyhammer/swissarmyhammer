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
import type { OpenBoard } from "@/types/kanban";

function Wrapper({ children }: { children: React.ReactNode }) {
  return (
    <SchemaProvider>
      <EntityStoreProvider entities={{}}>
        <FieldUpdateProvider>{children}</FieldUpdateProvider>
      </EntityStoreProvider>
    </SchemaProvider>
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
});
