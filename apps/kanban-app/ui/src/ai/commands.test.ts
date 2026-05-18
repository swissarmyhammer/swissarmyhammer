/**
 * Unit tests for the AI panel command registry (`ai/commands.ts`).
 *
 * The registry is the seam between the window-layer `ai.*` commands and the
 * AI panel React tree. These tests pin the contracts the window-layer command
 * builder and the AI panel components depend on:
 *
 *   - registered handlers are dispatched by the `trigger*` functions;
 *   - a handler that is not registered makes its `trigger*` a silent no-op;
 *   - `registerAiCommandHandlers` returns a scoped cleanup that clears only
 *     the slots it installed;
 *   - the streaming flag is observable and notifies subscribers on change.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  aiStreaming,
  registerAiCommandHandlers,
  resetAiCommandsForTest,
  setAiStreaming,
  subscribeAiStreaming,
  triggerAiCancel,
  triggerAiFocus,
  triggerAiModel,
  triggerAiNewChat,
  triggerAiToggle,
} from "./commands";

describe("ai command registry", () => {
  beforeEach(() => {
    resetAiCommandsForTest();
  });

  it("dispatches registered handlers via the trigger functions", () => {
    const toggle = vi.fn();
    const focus = vi.fn();
    const newChat = vi.fn();
    const cancel = vi.fn();
    const setModel = vi.fn();
    registerAiCommandHandlers({ toggle, focus, newChat, cancel, setModel });

    triggerAiToggle();
    triggerAiFocus();
    triggerAiNewChat();
    triggerAiCancel();
    triggerAiModel("claude-code");

    expect(toggle).toHaveBeenCalledTimes(1);
    expect(focus).toHaveBeenCalledTimes(1);
    expect(newChat).toHaveBeenCalledTimes(1);
    expect(cancel).toHaveBeenCalledTimes(1);
    expect(setModel).toHaveBeenCalledExactlyOnceWith("claude-code");
  });

  it("trigger functions are silent no-ops when no handler is registered", () => {
    // No registration — every trigger must not throw.
    expect(() => {
      triggerAiToggle();
      triggerAiFocus();
      triggerAiNewChat();
      triggerAiCancel();
      triggerAiModel("claude-code");
    }).not.toThrow();
  });

  it("triggerAiModel is a no-op when no model id is supplied", () => {
    const setModel = vi.fn();
    registerAiCommandHandlers({ setModel });
    triggerAiModel(undefined);
    expect(setModel).not.toHaveBeenCalled();
  });

  it("registerAiCommandHandlers merges partial handler sets", () => {
    const toggle = vi.fn();
    const cancel = vi.fn();
    // Two components each register the subset they own.
    registerAiCommandHandlers({ toggle });
    registerAiCommandHandlers({ cancel });

    triggerAiToggle();
    triggerAiCancel();
    expect(toggle).toHaveBeenCalledOnce();
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("the cleanup clears only the handlers that registration installed", () => {
    const toggle = vi.fn();
    const cancel = vi.fn();
    const cleanupToggle = registerAiCommandHandlers({ toggle });
    registerAiCommandHandlers({ cancel });

    // Tearing down the toggle registration leaves `cancel` intact.
    cleanupToggle();
    triggerAiToggle();
    triggerAiCancel();
    expect(toggle).not.toHaveBeenCalled();
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("a later registration of the same key survives an earlier cleanup", () => {
    const first = vi.fn();
    const second = vi.fn();
    const cleanupFirst = registerAiCommandHandlers({ toggle: first });
    // A remount registers a fresh handler for the same key.
    registerAiCommandHandlers({ toggle: second });
    // The stale cleanup must not wipe the newer handler.
    cleanupFirst();

    triggerAiToggle();
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledOnce();
  });

  it("streaming defaults to false and reflects setAiStreaming", () => {
    expect(aiStreaming()).toBe(false);
    setAiStreaming(true);
    expect(aiStreaming()).toBe(true);
    setAiStreaming(false);
    expect(aiStreaming()).toBe(false);
  });

  it("notifies subscribers on a real streaming change, not on a no-op", () => {
    const notify = vi.fn();
    const unsubscribe = subscribeAiStreaming(notify);

    setAiStreaming(true);
    expect(notify).toHaveBeenCalledTimes(1);

    // Setting the same value again is a no-op — no notification.
    setAiStreaming(true);
    expect(notify).toHaveBeenCalledTimes(1);

    setAiStreaming(false);
    expect(notify).toHaveBeenCalledTimes(2);

    // After unsubscribing, further changes are not delivered.
    unsubscribe();
    setAiStreaming(true);
    expect(notify).toHaveBeenCalledTimes(2);
  });
});
