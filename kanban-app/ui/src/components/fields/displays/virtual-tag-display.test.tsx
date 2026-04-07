import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { VirtualTagDisplay } from "./virtual-tag-display";

/** Wrap in TooltipProvider since VirtualTagDisplay uses Tooltip. */
function renderDisplay(value: unknown) {
  return render(
    <TooltipProvider>
      <VirtualTagDisplay value={value} />
    </TooltipProvider>,
  );
}

describe("VirtualTagDisplay", () => {
  it("renders READY and BLOCKING pills with correct text", () => {
    renderDisplay(["READY", "BLOCKING"]);
    expect(screen.getByText("READY")).toBeTruthy();
    expect(screen.getByText("BLOCKING")).toBeTruthy();
  });

  it("renders all three virtual tags", () => {
    renderDisplay(["READY", "BLOCKED", "BLOCKING"]);
    expect(screen.getByText("READY")).toBeTruthy();
    expect(screen.getByText("BLOCKED")).toBeTruthy();
    expect(screen.getByText("BLOCKING")).toBeTruthy();
  });

  it("applies correct colors to pills", () => {
    renderDisplay(["READY", "BLOCKED", "BLOCKING"]);

    const ready = screen.getByText("READY");
    const blocked = screen.getByText("BLOCKED");
    const blocking = screen.getByText("BLOCKING");

    // Browsers normalize hex colors to rgb() in computed style
    expect(ready.style.color).toBe("rgb(14, 138, 22)");
    expect(blocked.style.color).toBe("rgb(227, 98, 9)");
    expect(blocking.style.color).toBe("rgb(215, 58, 74)");
  });

  it("renders nothing for empty array", () => {
    const { container } = renderDisplay([]);
    // No child elements should be rendered
    expect(container.querySelector(".flex")).toBeNull();
  });

  it("renders nothing for undefined value", () => {
    const { container } = renderDisplay(undefined);
    expect(container.querySelector(".flex")).toBeNull();
  });

  it("renders nothing for non-array value", () => {
    const { container } = renderDisplay("not-an-array");
    expect(container.querySelector(".flex")).toBeNull();
  });

  it("skips unknown virtual tag slugs", () => {
    renderDisplay(["READY", "UNKNOWN_TAG"]);
    expect(screen.getByText("READY")).toBeTruthy();
    expect(screen.queryByText("UNKNOWN_TAG")).toBeNull();
  });

  it("renders pills with rounded-full styling", () => {
    renderDisplay(["READY"]);
    const pill = screen.getByText("READY");
    expect(pill.classList.contains("rounded-full")).toBe(true);
  });
});
