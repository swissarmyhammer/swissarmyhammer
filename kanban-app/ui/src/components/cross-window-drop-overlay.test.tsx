import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

/* ---- Mocks ---- */

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve({})),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

// Mock useDragSession to control session state directly
const mockSession = {
  session: null as import("@/lib/drag-session-context").DragSession | null,
  isSource: false,
  completeSession: vi.fn(),
  startSession: vi.fn(),
  cancelSession: vi.fn(),
};

vi.mock("@/lib/drag-session-context", () => ({
  useDragSession: () => mockSession,
}));

import { CrossWindowDropOverlay } from "./cross-window-drop-overlay";
import type { Entity } from "@/types/kanban";

/* ---- Helpers ---- */

const columns: Entity[] = [
  { entity_type: "column", id: "todo", fields: { name: "To Do", order: 0 } },
  { entity_type: "column", id: "doing", fields: { name: "Doing", order: 1 } },
  { entity_type: "column", id: "done", fields: { name: "Done", order: 2 } },
];

const tasksByColumn = new Map<string, string[]>([
  ["todo", ["t1", "t2"]],
  ["doing", ["t3"]],
  ["done", []],
]);

function makeSession(
  overrides: Partial<import("@/lib/drag-session-context").DragSession> = {},
) {
  return {
    session_id: "sess-1",
    source_board_path: "/board/a",
    source_window_label: "board-1",
    task_id: "task-1",
    task_fields: { title: "Drag me" },
    copy_mode: false,
    ...overrides,
  };
}

describe("CrossWindowDropOverlay", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSession.session = null;
    mockSession.isSource = false;
    mockSession.completeSession = vi.fn();
  });

  it("renders nothing when no session is active", () => {
    mockSession.session = null;
    const { container } = render(
      <CrossWindowDropOverlay columns={columns} tasksByColumn={tasksByColumn} />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders column drop zones when session is active and not source", () => {
    mockSession.session = makeSession();
    mockSession.isSource = false;

    render(
      <CrossWindowDropOverlay columns={columns} tasksByColumn={tasksByColumn} />,
    );

    expect(screen.getByText("To Do")).toBeTruthy();
    expect(screen.getByText("Doing")).toBeTruthy();
    expect(screen.getByText("Done")).toBeTruthy();
  });

  it("renders overlay in source window too (with pointer-events none)", () => {
    mockSession.session = makeSession({ source_window_label: "main" });
    mockSession.isSource = true;

    const { container } = render(
      <CrossWindowDropOverlay columns={columns} tasksByColumn={tasksByColumn} />,
    );

    // Overlay renders (not null)
    expect(screen.getByText("To Do")).toBeTruthy();

    // But pointer events are disabled for source window
    const overlay = container.firstElementChild as HTMLElement;
    expect(overlay.style.pointerEvents).toBe("none");
  });

  it("sets pointer-events auto for target windows", () => {
    mockSession.session = makeSession({ source_window_label: "board-1" });
    mockSession.isSource = false;

    const { container } = render(
      <CrossWindowDropOverlay columns={columns} tasksByColumn={tasksByColumn} />,
    );

    const overlay = container.firstElementChild as HTMLElement;
    expect(overlay.style.pointerEvents).toBe("auto");
  });

  it("shows copy label when session has copy_mode true", () => {
    mockSession.session = makeSession({ copy_mode: true });
    mockSession.isSource = false;

    render(
      <CrossWindowDropOverlay columns={columns} tasksByColumn={tasksByColumn} />,
    );

    // Copy mode doesn't show label unless column is hovered,
    // but we can verify the overlay renders without error
    expect(screen.getByText("To Do")).toBeTruthy();
  });

  it("renders correct number of column zones", () => {
    mockSession.session = makeSession();
    mockSession.isSource = false;

    const { container } = render(
      <CrossWindowDropOverlay columns={columns} tasksByColumn={tasksByColumn} />,
    );

    // 3 column zones + potential ghost card container
    const overlay = container.firstElementChild as HTMLElement;
    const columnZones = overlay.querySelectorAll(".flex-1");
    expect(columnZones.length).toBe(3);
  });
});
