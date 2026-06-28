/**
 * Tests for the strict `useDispatchCommand` mock factory
 * (`strict-dispatch-mock.ts`).
 *
 * The factory exists because dispatch-capturing test mocks used to fall back
 * to a SILENT no-op for unrecognized command ids — so a production dispatch
 * site reverting to a retired id (the `ui.inspector.close` regression caught
 * on card 01KTEBZSVGAZ881RAZZWWZXGPE's review) passed every node test. These
 * tests pin the loud-failure contract, including the permanent synthetic
 * negative: a known-RETIRED id must throw, never no-op.
 */
import { describe, it, expect, vi } from "vitest";
import { strictUseDispatchCommand } from "@/test/strict-dispatch-mock";

describe("strictUseDispatchCommand", () => {
  it("returns the registered dispatch for a known command id", async () => {
    const close = vi.fn(() => Promise.resolve("closed"));
    const hook = strictUseDispatchCommand({ "app.inspector.close": close });

    const dispatch = hook("app.inspector.close");
    await expect(dispatch()).resolves.toBe("closed");
    expect(close).toHaveBeenCalledTimes(1);
  });

  // Permanent synthetic negative: the retired `ui.*` id that the silent
  // fallback let through. A component requesting it must fail at hook time.
  it("throws at hook time for a retired command id (ui.inspector.close)", () => {
    const hook = strictUseDispatchCommand({
      "app.inspector.close": vi.fn(() => Promise.resolve()),
    });

    expect(() => hook("ui.inspector.close")).toThrowError(
      /unknown command id "ui\.inspector\.close"/,
    );
  });

  it("names the known ids in the failure message", () => {
    const hook = strictUseDispatchCommand({
      "app.dismiss": vi.fn(() => Promise.resolve()),
      "app.inspector.close": vi.fn(() => Promise.resolve()),
    });

    expect(() => hook("app.typo")).toThrowError(
      /app\.dismiss, app\.inspector\.close/,
    );
  });

  it("supports the ad-hoc overload, validating the id at call time", async () => {
    const dismiss = vi.fn(() => Promise.resolve("dismissed"));
    const hook = strictUseDispatchCommand({ "app.dismiss": dismiss });

    const adHoc = hook();
    await expect(adHoc("app.dismiss")).resolves.toBe("dismissed");
    expect(() => adHoc("ui.inspector.close")).toThrowError(
      /unknown command id "ui\.inspector\.close"/,
    );
  });
});
