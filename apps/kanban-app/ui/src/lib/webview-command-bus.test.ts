/**
 * Unit tests for the generic webview command handler bus
 * (`webview-command-bus.ts`).
 *
 * The bus is the reusable generalization of the AI panel's module-bus
 * (`ai/commands.ts`): a module-level registry keyed by **plugin command id**
 * that lets a presentation-only command behavior register a live handler the
 * dispatch path runs instead of the backend `execute command`. These tests
 * pin the contracts the dispatch path and the registering components depend
 * on:
 *
 *   - a registered handler is returned by the lookup and reported by the
 *     has-check;
 *   - `registerWebviewCommandHandler` returns a scoped cleanup that clears
 *     only the slot it still owns;
 *   - a remount registration of the same id is not wiped by an older cleanup
 *     (the StrictMode / HMR double-mount guard);
 *   - the test-reset clears every registration.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  getWebviewCommandHandler,
  hasWebviewCommandHandler,
  registerWebviewCommandHandler,
  resetWebviewCommandBusForTest,
} from "./webview-command-bus";

describe("webview command bus", () => {
  beforeEach(() => {
    resetWebviewCommandBusForTest();
  });

  it("looks up a registered handler by command id", () => {
    const handler = vi.fn();
    registerWebviewCommandHandler("nav.jump", handler);

    expect(hasWebviewCommandHandler("nav.jump")).toBe(true);
    expect(getWebviewCommandHandler("nav.jump")).toBe(handler);
  });

  it("reports no handler for an unregistered id", () => {
    expect(hasWebviewCommandHandler("nav.jump")).toBe(false);
    expect(getWebviewCommandHandler("nav.jump")).toBeUndefined();
  });

  it("install replaces the handler for an id", () => {
    const first = vi.fn();
    const second = vi.fn();
    registerWebviewCommandHandler("grid.edit", first);
    registerWebviewCommandHandler("grid.edit", second);

    expect(getWebviewCommandHandler("grid.edit")).toBe(second);
  });

  it("cleanup clears only the slot it installed", () => {
    const jump = vi.fn();
    const edit = vi.fn();
    const cleanupJump = registerWebviewCommandHandler("nav.jump", jump);
    registerWebviewCommandHandler("grid.edit", edit);

    cleanupJump();

    expect(hasWebviewCommandHandler("nav.jump")).toBe(false);
    // The unrelated id remains intact.
    expect(getWebviewCommandHandler("grid.edit")).toBe(edit);
  });

  it("a later registration of the same id survives an earlier cleanup", () => {
    const first = vi.fn();
    const second = vi.fn();
    const cleanupFirst = registerWebviewCommandHandler("nav.jump", first);
    // A remount registers a fresh handler for the same id.
    registerWebviewCommandHandler("nav.jump", second);
    // The stale cleanup must not wipe the newer handler.
    cleanupFirst();

    expect(hasWebviewCommandHandler("nav.jump")).toBe(true);
    expect(getWebviewCommandHandler("nav.jump")).toBe(second);
  });

  it("resetWebviewCommandBusForTest clears every registration", () => {
    registerWebviewCommandHandler("nav.jump", vi.fn());
    registerWebviewCommandHandler("grid.edit", vi.fn());

    resetWebviewCommandBusForTest();

    expect(hasWebviewCommandHandler("nav.jump")).toBe(false);
    expect(hasWebviewCommandHandler("grid.edit")).toBe(false);
  });
});
