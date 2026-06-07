/**
 * Unit tests for the host→UI request responder bus
 * (`ui-request-responder.ts`).
 *
 * The host emits a `ui/request` event `{ request_id, kind, params }`; the
 * responder bus dispatches by `kind` to a registered responder, then replies
 * via `invoke("ui_request_reply", { requestId, result })`. These pin the
 * contracts:
 *
 *   - a registered responder is invoked with the request `params` and its
 *     return value is sent back under the matching `request_id`;
 *   - a request for an unregistered `kind` still replies (with `null`) so the
 *     host await never hangs waiting on a `kind` nobody handles;
 *   - registration is ownership-guarded so a StrictMode / HMR remount of the
 *     same kind is not wiped by an older cleanup.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  handleUiRequest,
  registerUiResponder,
  resetUiRespondersForTest,
} from "./ui-request-responder";

describe("ui request responder bus", () => {
  beforeEach(() => {
    resetUiRespondersForTest();
  });

  it("dispatches by kind and replies with the responder result", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    registerUiResponder("focus.geometry", (params) => ({
      rect: { x: (params as { n: number }).n * 2 },
    }));

    await handleUiRequest(
      { request_id: "REQ1", kind: "focus.geometry", params: { n: 21 } },
      invoke,
    );

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("ui_request_reply", {
      requestId: "REQ1",
      result: { rect: { x: 42 } },
    });
  });

  it("correlates each request to its own reply id", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    registerUiResponder("echo", (params) => params);

    await handleUiRequest(
      { request_id: "A", kind: "echo", params: "aye" },
      invoke,
    );
    await handleUiRequest(
      { request_id: "B", kind: "echo", params: "bee" },
      invoke,
    );

    expect(invoke).toHaveBeenNthCalledWith(1, "ui_request_reply", {
      requestId: "A",
      result: "aye",
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "ui_request_reply", {
      requestId: "B",
      result: "bee",
    });
  });

  it("replies with null for an unhandled kind so the host never hangs", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);

    await handleUiRequest(
      { request_id: "REQ2", kind: "nobody.handles.this", params: {} },
      invoke,
    );

    expect(invoke).toHaveBeenCalledWith("ui_request_reply", {
      requestId: "REQ2",
      result: null,
    });
  });

  it("awaits an async responder before replying", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    registerUiResponder("slow", async () => {
      await Promise.resolve();
      return "done";
    });

    await handleUiRequest(
      { request_id: "REQ3", kind: "slow", params: {} },
      invoke,
    );

    expect(invoke).toHaveBeenCalledWith("ui_request_reply", {
      requestId: "REQ3",
      result: "done",
    });
  });

  it("does NOT reply when the request targets a different window", async () => {
    // Regression: in a multi-window app every window's global `listen`
    // receives the host's `emit_to(target)` event, so a NON-target window
    // would otherwise answer (often with null) and win the host's request
    // correlation — breaking nav. The window guard makes a non-target window
    // stay silent.
    const invoke = vi.fn().mockResolvedValue(undefined);
    registerUiResponder("focus.geometry", () => ({ rect: { x: 1 } }));

    await handleUiRequest(
      {
        request_id: "REQW",
        kind: "focus.geometry",
        params: {},
        window: "board-OTHER",
      },
      invoke,
      "board-ME",
    );

    expect(invoke).not.toHaveBeenCalled();
  });

  it("replies when the request targets this window", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    registerUiResponder("focus.geometry", () => ({ rect: { x: 1 } }));

    await handleUiRequest(
      {
        request_id: "REQW2",
        kind: "focus.geometry",
        params: {},
        window: "board-ME",
      },
      invoke,
      "board-ME",
    );

    expect(invoke).toHaveBeenCalledWith("ui_request_reply", {
      requestId: "REQW2",
      result: { rect: { x: 1 } },
    });
  });

  it("replies when the request carries no target window (legacy/backward-compat)", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    registerUiResponder("echo", (p) => p);

    await handleUiRequest(
      { request_id: "REQW3", kind: "echo", params: "hi" },
      invoke,
      "board-ME",
    );

    expect(invoke).toHaveBeenCalledWith("ui_request_reply", {
      requestId: "REQW3",
      result: "hi",
    });
  });

  it("cleanup clears only the slot it installed", () => {
    const first = vi.fn();
    const second = vi.fn();
    const cleanupFirst = registerUiResponder("k", first);
    registerUiResponder("k", second);

    // The older cleanup must NOT wipe the newer registration (remount guard).
    cleanupFirst();

    const invoke = vi.fn().mockResolvedValue(undefined);
    return handleUiRequest(
      { request_id: "R", kind: "k", params: 1 },
      invoke,
    ).then(() => {
      expect(second).toHaveBeenCalledWith(1);
      expect(first).not.toHaveBeenCalled();
    });
  });
});
