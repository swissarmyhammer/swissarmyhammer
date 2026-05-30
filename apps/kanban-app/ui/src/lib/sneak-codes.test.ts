/**
 * Tests for the {@link generateSneakCodes} thin MCP wrapper.
 *
 * The algorithm itself lives in Rust (covered by
 * `swissarmyhammer-focus/src/sneak.rs` unit tests); these tests verify
 * only the wrapper contract — that `count` flows through verbatim and
 * the kernel's structured result is unwrapped to the bare codes array.
 *
 * After the frontend-migration card (Stage 3 cut-over) this wrapper now
 * routes through the `focus` MCP server's `generate sneak_codes` op via
 * `command_tool_call`, instead of the dedicated `generate_jump_codes`
 * Tauri command. The kernel returns `{ ok, codes }`; the wrapper
 * unwraps `codes` so callers stay unchanged.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { generateSneakCodes } from "./sneak-codes";

describe("generateSneakCodes", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("routes through command_tool_call with the focus server's generate sneak_codes op", async () => {
    mockInvoke.mockResolvedValue({ ok: true, codes: ["a", "s", "d"] });

    await generateSneakCodes(3);

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith("command_tool_call", {
      module: "focus",
      tool: "focus",
      op: "generate sneak_codes",
      params: { count: 3 },
    });
  });

  it("unwraps the kernel envelope to the bare codes array", async () => {
    const expected = ["a", "s", "d", "f"];
    mockInvoke.mockResolvedValue({ ok: true, codes: expected });

    const result = await generateSneakCodes(4);

    expect(result).toEqual(expected);
  });

  it("passes count=0 through and returns an empty array", async () => {
    mockInvoke.mockResolvedValue({ ok: true, codes: [] });

    const result = await generateSneakCodes(0);

    expect(mockInvoke).toHaveBeenCalledWith("command_tool_call", {
      module: "focus",
      tool: "focus",
      op: "generate sneak_codes",
      params: { count: 0 },
    });
    expect(result).toEqual([]);
  });

  it("propagates errors from the kernel as rejected promises", async () => {
    mockInvoke.mockRejectedValue(
      "too many jump targets: 1000 exceeds capacity 529",
    );

    await expect(generateSneakCodes(1000)).rejects.toMatch(
      /too many jump targets/,
    );
  });
});
