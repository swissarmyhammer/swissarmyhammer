/**
 * Tests for {@link reportDispatchError}.
 *
 * The helper is the single hook the keybinding handler, the native-menu
 * listener, and the context-menu listener use to surface a backend
 * dispatch failure as a visible toast — without it, a paste error
 * triggered by Mod+V or a context-menu click would vanish into an
 * unhandled promise rejection. These tests pin:
 *
 * 1. The toast surfaces with `toast.error` (red/destructive variant
 *    in sonner) so the user can't miss the failure.
 * 2. The toast message names the command id so the user can correlate
 *    the failure to the action they took.
 * 3. The toast message names the specific backend failure (e.g. the
 *    `DestinationInvalid` text the paste handler produced), not a
 *    generic "paste failed".
 * 4. The Tauri "Command failed: " framing is stripped so the toast
 *    reads cleanly.
 * 5. Both `Error` instances and plain strings — Tauri's `invoke`
 *    rejects with a string payload — produce the same shape of toast.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    info: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
  },
}));

import { toast } from "sonner";
import { reportDispatchError } from "./dispatch-error";

describe("reportDispatchError", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("surfaces dispatch failures via toast.error", () => {
    reportDispatchError(
      "entity.paste",
      new Error(
        "Command failed: destination invalid: Column 'doing' no longer exists",
      ),
    );
    expect(toast.error).toHaveBeenCalledTimes(1);
  });

  it("names the failing command id in the toast", () => {
    reportDispatchError(
      "entity.paste",
      new Error("destination invalid: Column 'ghost' no longer exists"),
    );
    const arg = (toast.error as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(arg).toContain("entity.paste");
  });

  it("preserves the backend's specific failure message in the toast", () => {
    // Paste of a task into a column that was deleted between copy and
    // paste. The toast must carry the column name through so the user
    // sees what failed instead of a generic "paste failed".
    reportDispatchError(
      "entity.paste",
      new Error(
        "Command failed: destination invalid: Column 'doing' no longer exists",
      ),
    );
    const arg = (toast.error as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(arg).toContain("Column 'doing' no longer exists");
  });

  it("strips the 'Command failed: ' wrapper that Tauri adds", () => {
    // The dispatch_command tauri wrapper prepends "Command failed: "
    // for telemetry; we don't want that to leak into the user-facing
    // toast and duplicate the framing we add ourselves.
    reportDispatchError(
      "entity.paste",
      new Error(
        "Command failed: source entity missing: Tag '01XYZ' no longer exists",
      ),
    );
    const arg = (toast.error as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(arg).not.toContain("Command failed:");
    expect(arg).toContain("source entity missing");
  });

  it("accepts plain string rejections (the Tauri invoke rejection shape)", () => {
    // `invoke()` rejects with a string payload, not an Error instance.
    // Make sure the helper handles both shapes equivalently — a paste
    // failure delivered through Tauri must still produce a toast.
    reportDispatchError(
      "entity.paste",
      "Command failed: destination invalid: Board has no columns to paste a task into",
    );
    expect(toast.error).toHaveBeenCalledTimes(1);
    const arg = (toast.error as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(arg).toContain("Board has no columns");
  });

  it("handles non-Error, non-string thrown values without crashing", () => {
    // Defensive: a rejected non-string non-Error value (object, number)
    // must still produce a toast — better a generic message than a
    // silent failure or thrown TypeError.
    reportDispatchError("entity.paste", { unexpected: true });
    expect(toast.error).toHaveBeenCalledTimes(1);
  });
});
