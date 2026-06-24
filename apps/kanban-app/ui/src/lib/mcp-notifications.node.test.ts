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
