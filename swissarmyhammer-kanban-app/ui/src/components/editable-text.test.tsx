import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { EditableText } from "./editable-text";

describe("EditableText", () => {
  it("renders the value as text by default", () => {
    render(<EditableText value="Hello" onCommit={() => {}} />);
    expect(screen.getByText("Hello")).toBeTruthy();
  });

  it("switches to an input on click", () => {
    render(<EditableText value="Hello" onCommit={() => {}} />);
    fireEvent.click(screen.getByText("Hello"));
    const input = screen.getByRole("textbox");
    expect(input).toBeTruthy();
    expect((input as HTMLInputElement).value).toBe("Hello");
  });

  it("calls onCommit with new value on blur", () => {
    const onCommit = vi.fn();
    render(<EditableText value="Hello" onCommit={onCommit} />);
    fireEvent.click(screen.getByText("Hello"));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "World" } });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenCalledWith("World");
  });

  it("does not call onCommit if value is unchanged", () => {
    const onCommit = vi.fn();
    render(<EditableText value="Hello" onCommit={onCommit} />);
    fireEvent.click(screen.getByText("Hello"));
    fireEvent.blur(screen.getByRole("textbox"));
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("commits on Enter key", () => {
    const onCommit = vi.fn();
    render(<EditableText value="Hello" onCommit={onCommit} />);
    fireEvent.click(screen.getByText("Hello"));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "World" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledWith("World");
  });

  it("cancels on Escape key without committing", () => {
    const onCommit = vi.fn();
    render(<EditableText value="Hello" onCommit={onCommit} />);
    fireEvent.click(screen.getByText("Hello"));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "World" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onCommit).not.toHaveBeenCalled();
    // Should be back to text display
    expect(screen.getByText("Hello")).toBeTruthy();
  });

  describe("multiline", () => {
    it("renders a textarea when multiline and clicked", () => {
      render(<EditableText value="some text" multiline onCommit={() => {}} />);
      fireEvent.click(screen.getByText("some text"));
      const textarea = screen.getByRole("textbox");
      expect(textarea.tagName).toBe("TEXTAREA");
    });

    it("commits multiline on blur", () => {
      const onCommit = vi.fn();
      render(<EditableText value="old" multiline onCommit={onCommit} />);
      fireEvent.click(screen.getByText("old"));
      const textarea = screen.getByRole("textbox");
      fireEvent.change(textarea, { target: { value: "new\nlines" } });
      fireEvent.blur(textarea);
      expect(onCommit).toHaveBeenCalledWith("new\nlines");
    });

    it("allows Enter without committing in multiline mode", () => {
      const onCommit = vi.fn();
      render(<EditableText value="old" multiline onCommit={onCommit} />);
      fireEvent.click(screen.getByText("old"));
      const textarea = screen.getByRole("textbox");
      fireEvent.keyDown(textarea, { key: "Enter" });
      expect(onCommit).not.toHaveBeenCalled();
      // Should still be editing
      expect(screen.getByRole("textbox")).toBeTruthy();
    });

    it("cancels multiline on Escape", () => {
      const onCommit = vi.fn();
      render(<EditableText value="old" multiline onCommit={onCommit} />);
      fireEvent.click(screen.getByText("old"));
      fireEvent.change(screen.getByRole("textbox"), { target: { value: "changed" } });
      fireEvent.keyDown(screen.getByRole("textbox"), { key: "Escape" });
      expect(onCommit).not.toHaveBeenCalled();
      expect(screen.getByText("old")).toBeTruthy();
    });

    it("shows placeholder when value is empty", () => {
      render(<EditableText value="" placeholder="Add description..." onCommit={() => {}} />);
      expect(screen.getByText("Add description...")).toBeTruthy();
    });
  });
});
