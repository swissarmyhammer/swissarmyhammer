/**
 * Tests for the {@link generateSneakCodes} thin invoke wrapper.
 *
 * The algorithm itself lives in Rust (covered by
 * `swissarmyhammer-focus/src/sneak.rs` unit tests); these tests verify
 * only the wrapper contract — that `count` flows through verbatim and
 * the resolved array is returned unchanged.
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

  it("invokes generate_jump_codes with the requested count", async () => {
    mockInvoke.mockResolvedValue(["a", "s", "d"]);

    await generateSneakCodes(3);

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith("generate_jump_codes", { count: 3 });
  });

  it("returns the resolved array verbatim", async () => {
    const expected = ["a", "s", "d", "f"];
    mockInvoke.mockResolvedValue(expected);

    const result = await generateSneakCodes(4);

    expect(result).toEqual(expected);
  });

  it("passes count=0 through and returns an empty array", async () => {
    mockInvoke.mockResolvedValue([]);

    const result = await generateSneakCodes(0);

    expect(mockInvoke).toHaveBeenCalledWith("generate_jump_codes", { count: 0 });
    expect(result).toEqual([]);
  });

  it("propagates errors from the kernel as rejected promises", async () => {
    mockInvoke.mockRejectedValue("too many jump targets: 1000 exceeds capacity 529");

    await expect(generateSneakCodes(1000)).rejects.toMatch(/too many jump targets/);
  });
});
