import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("cua")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { TaskCard } from "./task-card";
import { KeymapProvider } from "@/lib/keymap-context";
import type { Task } from "@/types/kanban";

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: "task-1",
    title: "Hello **world**",
    description: "",
    tags: [],
    assignees: [],
    depends_on: [],
    position: { column: "col-1", ordinal: "a0" },
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
    ...overrides,
  };
}

function renderWithProvider(ui: React.ReactElement) {
  return render(<KeymapProvider>{ui}</KeymapProvider>);
}

describe("TaskCard", () => {
  it("renders title as markdown (bold text)", () => {
    renderWithProvider(<TaskCard task={makeTask()} />);
    const strong = screen.getByText("world");
    expect(strong.tagName).toBe("STRONG");
  });

  it("clicking the title does not trigger card onClick", () => {
    const onClick = vi.fn();
    renderWithProvider(<TaskCard task={makeTask()} onClick={onClick} />);
    // Click on the markdown-rendered title area
    const titleEl = screen.getByText("world");
    fireEvent.click(titleEl);
    expect(onClick).not.toHaveBeenCalled();
  });

  it("clicking the card body (outside title) triggers onClick", () => {
    const onClick = vi.fn();
    const { container } = renderWithProvider(
      <TaskCard task={makeTask()} onClick={onClick} />
    );
    // Click the card's outer div (the progress area or card itself)
    const card = container.querySelector(".rounded-md")!;
    fireEvent.click(card);
    expect(onClick).toHaveBeenCalledWith(makeTask());
  });

  it("calls onUpdateTitle when title is edited", () => {
    const onUpdateTitle = vi.fn();
    const { container } = renderWithProvider(
      <TaskCard task={makeTask()} onUpdateTitle={onUpdateTitle} />
    );
    // Click title to enter edit mode
    const titleEl = screen.getByText("world");
    fireEvent.click(titleEl);
    // Should now have a CodeMirror editor
    expect(container.querySelector(".cm-editor")).toBeTruthy();
  });

  it("double-clicking the title triggers card onClick (opens inspector)", () => {
    const onClick = vi.fn();
    renderWithProvider(<TaskCard task={makeTask()} onClick={onClick} />);
    const titleEl = screen.getByText("world");
    fireEvent.doubleClick(titleEl);
    expect(onClick).toHaveBeenCalledWith(makeTask());
  });

  it("double-clicking the title exits edit mode (blurs editor)", () => {
    const { container } = renderWithProvider(
      <TaskCard task={makeTask()} onClick={() => {}} />
    );
    // First click enters edit mode
    const titleEl = screen.getByText("world");
    fireEvent.click(titleEl);
    expect(container.querySelector(".cm-editor")).toBeTruthy();
    // Double-click should blur the editor, returning to display mode
    fireEvent.doubleClick(container.querySelector(".cm-editor")!);
    expect(container.querySelector(".cm-editor")).toBeNull();
  });

  describe("progress bar", () => {
    it("shows progress bar when description has checkboxes", () => {
      const { container } = renderWithProvider(
        <TaskCard
          task={makeTask({
            description: "- [x] done\n- [ ] pending\n- [ ] also pending",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("33");
    });

    it("shows 0% progress when no checkboxes are checked", () => {
      const { container } = renderWithProvider(
        <TaskCard
          task={makeTask({
            description: "- [ ] first\n- [ ] second",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("0");
      expect(container.textContent).toContain("0%");
    });

    it("shows 100% progress when all checkboxes are checked", () => {
      const { container } = renderWithProvider(
        <TaskCard
          task={makeTask({
            description: "- [x] done\n- [x] also done",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeTruthy();
      expect(progressBar!.getAttribute("aria-valuenow")).toBe("100");
    });

    it("does not show progress bar when description has no checkboxes", () => {
      const { container } = renderWithProvider(
        <TaskCard
          task={makeTask({
            description: "Just some plain text",
          })}
        />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });

    it("does not show progress bar when description is empty", () => {
      const { container } = renderWithProvider(
        <TaskCard task={makeTask()} />
      );
      const progressBar = container.querySelector('[role="progressbar"]');
      expect(progressBar).toBeNull();
    });
  });
});
