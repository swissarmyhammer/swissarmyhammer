import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve("cua")),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { EditableMarkdown } from "./editable-markdown";
import { KeymapProvider } from "@/lib/keymap-context";

function renderWithProvider(ui: React.ReactElement) {
  return render(<KeymapProvider>{ui}</KeymapProvider>);
}

describe("EditableMarkdown", () => {
  describe("display mode", () => {
    it("renders markdown content", () => {
      renderWithProvider(
        <EditableMarkdown value="Hello **world**" onCommit={() => {}} />
      );
      expect(screen.getByText("world")).toBeTruthy();
      // "world" should be inside a <strong> tag
      const strong = screen.getByText("world");
      expect(strong.tagName).toBe("STRONG");
    });

    it("renders multiline markdown with GFM", () => {
      renderWithProvider(
        <EditableMarkdown
          value={"# Heading\n\n- item 1\n- item 2"}
          onCommit={() => {}}
          multiline
        />
      );
      expect(screen.getByText("Heading")).toBeTruthy();
      expect(screen.getByText("item 1")).toBeTruthy();
      expect(screen.getByText("item 2")).toBeTruthy();
    });

    it("shows placeholder when value is empty", () => {
      renderWithProvider(
        <EditableMarkdown
          value=""
          onCommit={() => {}}
          placeholder="Add description..."
        />
      );
      expect(screen.getByText("Add description...")).toBeTruthy();
    });

    it("applies className to display container", () => {
      const { container } = renderWithProvider(
        <EditableMarkdown
          value="test"
          onCommit={() => {}}
          className="custom-class"
        />
      );
      const el = container.querySelector(".custom-class");
      expect(el).toBeTruthy();
    });
  });

  describe("edit mode", () => {
    it("switches to editor on click", () => {
      const { container } = renderWithProvider(
        <EditableMarkdown value="Hello" onCommit={() => {}} />
      );
      // Click the display div
      const display = container.querySelector(".cursor-text");
      expect(display).toBeTruthy();
      fireEvent.click(display!);
      // Should now have a CodeMirror editor (cm-editor class)
      const editor = container.querySelector(".cm-editor");
      expect(editor).toBeTruthy();
    });

    it("switches to editor on click for multiline", () => {
      const { container } = renderWithProvider(
        <EditableMarkdown value="Some text" onCommit={() => {}} multiline />
      );
      fireEvent.click(container.querySelector(".cursor-text")!);
      expect(container.querySelector(".cm-editor")).toBeTruthy();
    });

    it("switches to editor when clicking placeholder", () => {
      const { container } = renderWithProvider(
        <EditableMarkdown
          value=""
          onCommit={() => {}}
          placeholder="Add description..."
        />
      );
      fireEvent.click(screen.getByText("Add description..."));
      expect(container.querySelector(".cm-editor")).toBeTruthy();
    });
  });

  describe("checkbox toggling", () => {
    it("toggles unchecked checkbox to checked", () => {
      const onCommit = vi.fn();
      renderWithProvider(
        <EditableMarkdown
          value={"- [ ] todo item\n- [x] done item"}
          onCommit={onCommit}
          multiline
        />
      );
      // Find the first checkbox (unchecked)
      const checkboxes = screen.getAllByRole("checkbox");
      expect(checkboxes).toHaveLength(2);
      expect((checkboxes[0] as HTMLInputElement).checked).toBe(false);
      expect((checkboxes[1] as HTMLInputElement).checked).toBe(true);

      // Click the first checkbox
      fireEvent.click(checkboxes[0]);
      expect(onCommit).toHaveBeenCalledWith("- [x] todo item\n- [x] done item");
    });

    it("toggles checked checkbox to unchecked", () => {
      const onCommit = vi.fn();
      renderWithProvider(
        <EditableMarkdown
          value={"- [ ] todo item\n- [x] done item"}
          onCommit={onCommit}
          multiline
        />
      );
      const checkboxes = screen.getAllByRole("checkbox");
      fireEvent.click(checkboxes[1]);
      expect(onCommit).toHaveBeenCalledWith("- [ ] todo item\n- [ ] done item");
    });

    it("does not enter edit mode when clicking checkbox", () => {
      const onCommit = vi.fn();
      const { container } = renderWithProvider(
        <EditableMarkdown
          value={"- [ ] todo"}
          onCommit={onCommit}
          multiline
        />
      );
      const checkbox = screen.getByRole("checkbox");
      fireEvent.click(checkbox);
      // Should NOT have entered edit mode (no cm-editor)
      expect(container.querySelector(".cm-editor")).toBeNull();
    });

    it("toggles the correct checkbox among many subtasks", () => {
      const onCommit = vi.fn();
      const value =
        "- [ ] first\n- [ ] second\n- [ ] third\n- [ ] fourth\n- [ ] fifth";
      renderWithProvider(
        <EditableMarkdown value={value} onCommit={onCommit} multiline />
      );
      const checkboxes = screen.getAllByRole("checkbox");
      expect(checkboxes).toHaveLength(5);

      // Toggle the third checkbox (index 2)
      fireEvent.click(checkboxes[2]);
      expect(onCommit).toHaveBeenCalledWith(
        "- [ ] first\n- [ ] second\n- [x] third\n- [ ] fourth\n- [ ] fifth"
      );
    });

    it("toggles the last checkbox among many subtasks", () => {
      const onCommit = vi.fn();
      const value =
        "- [x] first\n- [ ] second\n- [x] third\n- [ ] fourth\n- [ ] fifth";
      renderWithProvider(
        <EditableMarkdown value={value} onCommit={onCommit} multiline />
      );
      const checkboxes = screen.getAllByRole("checkbox");
      expect(checkboxes).toHaveLength(5);

      // Toggle the fifth checkbox (index 4)
      fireEvent.click(checkboxes[4]);
      expect(onCommit).toHaveBeenCalledWith(
        "- [x] first\n- [ ] second\n- [x] third\n- [ ] fourth\n- [x] fifth"
      );
    });

    it("toggles the correct checkbox when mixed with other content", () => {
      const onCommit = vi.fn();
      const value =
        "## Subtasks\n\n- [ ] alpha\n- [x] bravo\n- [ ] charlie\n\nSome notes here.";
      renderWithProvider(
        <EditableMarkdown value={value} onCommit={onCommit} multiline />
      );
      const checkboxes = screen.getAllByRole("checkbox");
      expect(checkboxes).toHaveLength(3);

      // Toggle the middle checkbox (bravo, index 1) — uncheck it
      fireEvent.click(checkboxes[1]);
      expect(onCommit).toHaveBeenCalledWith(
        "## Subtasks\n\n- [ ] alpha\n- [ ] bravo\n- [ ] charlie\n\nSome notes here."
      );
    });
  });
});
