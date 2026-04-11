import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

// `virtual-tag-display.tsx` pulls metadata from `useBoardData()` which is
// backed by `window-container.tsx`. That module calls `getCurrentWindow()`
// at load time, so we stub the whole transitive chain here.
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// Stub `useBoardData` to provide the virtual-tag metadata that used to live
// in a hardcoded map on the frontend. These values mirror the Rust
// `DEFAULT_REGISTRY` in `virtual_tags.rs`. Avoid `vi.importActual` here:
// re-entering the real `window-container.tsx` pulls in the rest of the
// container chain (RustEngine, CommandScope, TooltipProvider, etc.) which
// hangs the test runner in browser mode. `VirtualTagDisplay` only ever
// touches `useBoardData`, so a minimal stub is sufficient.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({
    board: {
      id: "stub-board",
      entity_type: "board",
      moniker: "board:stub",
      fields: {},
    },
    columns: [],
    tags: [],
    virtualTagMeta: [
      {
        slug: "READY",
        color: "0e8a16",
        description: "Task has no unmet dependencies",
      },
      {
        slug: "BLOCKED",
        color: "e36209",
        description: "Task has at least one unmet dependency",
      },
      {
        slug: "BLOCKING",
        color: "d73a4a",
        description: "Other tasks depend on this one",
      },
    ],
    summary: {
      total: 0,
      by_column: {},
      by_tag: {},
    },
  }),
}));

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
  it("renders READY and BLOCKING pills with # prefix", () => {
    renderDisplay(["READY", "BLOCKING"]);
    expect(screen.getByText("#READY")).toBeTruthy();
    expect(screen.getByText("#BLOCKING")).toBeTruthy();
  });

  it("renders all three virtual tags with # prefix", () => {
    renderDisplay(["READY", "BLOCKED", "BLOCKING"]);
    expect(screen.getByText("#READY")).toBeTruthy();
    expect(screen.getByText("#BLOCKED")).toBeTruthy();
    expect(screen.getByText("#BLOCKING")).toBeTruthy();
  });

  it("applies correct colors to pills", () => {
    renderDisplay(["READY", "BLOCKED", "BLOCKING"]);

    const ready = screen.getByText("#READY");
    const blocked = screen.getByText("#BLOCKED");
    const blocking = screen.getByText("#BLOCKING");

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
    expect(screen.getByText("#READY")).toBeTruthy();
    expect(screen.queryByText("#UNKNOWN_TAG")).toBeNull();
    expect(screen.queryByText("UNKNOWN_TAG")).toBeNull();
  });

  it("renders pills with rounded-full styling", () => {
    renderDisplay(["READY"]);
    const pill = screen.getByText("#READY");
    expect(pill.classList.contains("rounded-full")).toBe(true);
  });
});
