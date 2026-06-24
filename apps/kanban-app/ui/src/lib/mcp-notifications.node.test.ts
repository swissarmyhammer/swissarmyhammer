/**
 * Unit coverage for the `subscribeFocusChanged` public seam.
 *
 * `subscribeFocusChanged` is the plugin / MCP-client subscriber for the
 * `notifications/focus/changed` plane. Unlike the data/UI-state subscribers, it
 * has no in-app production consumer (the `SpatialFocusProvider` `listen`s
 * directly for synchronous registration), so it would otherwise be uncovered.
 * This pins its contract: it registers a Tauri listener for the
 * `FOCUS_CHANGED_EVENT` method name and forwards each notification's `params`
 * to the caller.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";

/** Captured `listen(event, cb)` registrations, keyed by event name. */
const listenHandlers: Record<string, (event: { payload: unknown }) => void> =
  {};

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (event: string, handler: (e: { payload: unknown }) => void) => {
      listenHandlers[event] = handler;
      return Promise.resolve(() => {
        delete listenHandlers[event];
      });
    },
  ),
}));

import {
  subscribeFocusChanged,
  FOCUS_CHANGED_EVENT,
  type FocusChanged,
  subscribeDragStarted,
  subscribeDragCancelled,
  subscribeDragCompleted,
  DRAG_STARTED_EVENT,
  DRAG_CANCELLED_EVENT,
  DRAG_COMPLETED_EVENT,
  type DragStarted,
  type DragCancelled,
  type DragCompleted,
} from "./mcp-notifications";

beforeEach(() => {
  for (const key of Object.keys(listenHandlers)) delete listenHandlers[key];
});

describe("subscribeFocusChanged", () => {
  it("registers a listener for the focus/changed bridge event and forwards the payload", async () => {
    const received: FocusChanged[] = [];
    const unsubscribe = await subscribeFocusChanged((change) =>
      received.push(change),
    );

    // The seam targets the bridge event name (the notification method), never
    // the legacy direct `focus-changed` Tauri event.
    expect(FOCUS_CHANGED_EVENT).toBe("notifications/focus/changed");
    expect(listenHandlers[FOCUS_CHANGED_EVENT]).toBeDefined();
    expect(listenHandlers["focus-changed"]).toBeUndefined();

    const payload: FocusChanged = {
      window_label: "main",
      prev_fq: "/main/a",
      next_fq: "/main/b",
      next_segment: "b",
    };
    listenHandlers[FOCUS_CHANGED_EVENT]({ payload });

    expect(received).toEqual([payload]);

    unsubscribe();
    expect(listenHandlers[FOCUS_CHANGED_EVENT]).toBeUndefined();
  });
});

describe("drag lifecycle subscribers", () => {
  it("subscribeDragStarted targets the bridge event, never the legacy Tauri event", async () => {
    const received: DragStarted[] = [];
    const unsubscribe = await subscribeDragStarted((s) => received.push(s));

    expect(DRAG_STARTED_EVENT).toBe("notifications/ui_state/drag_started");
    expect(listenHandlers[DRAG_STARTED_EVENT]).toBeDefined();
    expect(listenHandlers["drag-session-active"]).toBeUndefined();

    const payload: DragStarted = {
      session_id: "sess-1",
      source_board_path: "/board/a",
      source_window_label: "main",
      task_id: "task-1",
      task_fields: {},
      copy_mode: false,
      started_at_ms: 7,
      from: { kind: "focus_chain", entity_id: "task-1" },
    } as unknown as DragStarted;
    listenHandlers[DRAG_STARTED_EVENT]({ payload });
    expect(received).toEqual([payload]);

    unsubscribe();
    expect(listenHandlers[DRAG_STARTED_EVENT]).toBeUndefined();
  });

  it("subscribeDragCancelled targets the bridge event, never the legacy Tauri event", async () => {
    const received: DragCancelled[] = [];
    const unsubscribe = await subscribeDragCancelled((s) => received.push(s));

    expect(DRAG_CANCELLED_EVENT).toBe("notifications/ui_state/drag_cancelled");
    expect(listenHandlers[DRAG_CANCELLED_EVENT]).toBeDefined();
    expect(listenHandlers["drag-session-cancelled"]).toBeUndefined();

    const payload: DragCancelled = { session_id: "sess-1" };
    listenHandlers[DRAG_CANCELLED_EVENT]({ payload });
    expect(received).toEqual([payload]);

    unsubscribe();
    expect(listenHandlers[DRAG_CANCELLED_EVENT]).toBeUndefined();
  });

  it("subscribeDragCompleted targets the bridge event, never the legacy Tauri event", async () => {
    const received: DragCompleted[] = [];
    const unsubscribe = await subscribeDragCompleted((s) => received.push(s));

    expect(DRAG_COMPLETED_EVENT).toBe("notifications/ui_state/drag_completed");
    expect(listenHandlers[DRAG_COMPLETED_EVENT]).toBeDefined();
    expect(listenHandlers["drag-session-completed"]).toBeUndefined();

    const payload: DragCompleted = { session_id: "sess-1", success: true };
    listenHandlers[DRAG_COMPLETED_EVENT]({ payload });
    expect(received).toEqual([payload]);

    unsubscribe();
    expect(listenHandlers[DRAG_COMPLETED_EVENT]).toBeUndefined();
  });
});
