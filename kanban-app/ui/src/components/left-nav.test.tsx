/**
 * LeftNav focus-class wiring.
 *
 * With the global `[data-focused]` ring removed, the LeftNav buttons —
 * which live in a 10-unit-wide (`w-10`) flex strip with overflow
 * clipping — need a per-consumer override so the left-edge focus bar
 * renders inside the button rather than off its left edge. The
 * `nav-button-focus` class repositions the bar (see `index.css`).
 *
 * These tests lock the class onto the LeftNav button element so the
 * CSS override actually lands on the focused node.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import type { ViewDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri stubs — LeftNav's `FocusScope` wrappers call `spatial_register`,
// which is a Tauri invoke. Route it to a no-op so the component mounts.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
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

// ---------------------------------------------------------------------------
// Views-context mock — supplies a deterministic two-view list so the
// strip always renders two buttons.
// ---------------------------------------------------------------------------

const VIEWS: ViewDef[] = [
  { id: "board", name: "Board", kind: "board", icon: "kanban" },
  { id: "grid", name: "Grid", kind: "grid", icon: "table" },
];

vi.mock("@/lib/views-context", () => ({
  useViews: () => ({
    views: VIEWS,
    activeView: VIEWS[0],
    setActiveViewId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

// Imports after mocks.
import { LeftNav } from "./left-nav";

function renderLeftNav() {
  return render(
    <EntityFocusProvider>
      <TooltipProvider>
        <LeftNav />
      </TooltipProvider>
    </EntityFocusProvider>,
  );
}

describe("LeftNav focus class wiring", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("each view button carries `nav-button-focus`", () => {
    const { container } = renderLeftNav();
    // Buttons are identified by `data-moniker="view:<id>"` — the same
    // attribute the FocusScope writes onto each button element.
    const buttons = container.querySelectorAll("button[data-moniker^='view:']");
    expect(buttons.length).toBe(VIEWS.length);
    for (const btn of buttons) {
      expect(btn.className).toContain("nav-button-focus");
    }
  });
});
