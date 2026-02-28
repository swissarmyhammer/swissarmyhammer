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
});
