/**
 * Unit tests for {@link copyText} — the shared chat-copy helper.
 *
 * `copyText` must do two things on every copy:
 *
 *   1. write the OS clipboard via `navigator.clipboard.writeText` (so CUA /
 *      emacs `Cmd/Ctrl+V` and system paste keep working), and
 *   2. mirror the text into the `@replit/codemirror-vim` registers so a bare
 *      `p` in a CM6 vim editor pastes it — vim's non-`+` paste reads the
 *      unnamed (`"`) register synchronously, which an OS `writeText` never
 *      populates.
 *
 * The vim-register mirror is best-effort: when the vim global state has not
 * been initialised (no vim editor mounted), `getRegisterController` may throw,
 * and `copyText` must still have written the OS clipboard and must not reject.
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import { Vim } from "@replit/codemirror-vim";
import { copyText } from "./clipboard";

afterEach(() => {
  vi.restoreAllMocks();
});

/** A fake register that records the last `setText` it received. */
function makeRegister() {
  return {
    text: undefined as string | undefined,
    setText(text?: string) {
      this.text = text;
    },
  };
}

describe("copyText", () => {
  it("writes the OS clipboard and mirrors text into the vim registers", async () => {
    const writeText = vi
      .spyOn(navigator.clipboard, "writeText")
      .mockResolvedValue(undefined);

    const unnamed = makeRegister();
    const zero = makeRegister();
    const plus = makeRegister();
    const getRegister = vi.fn((name?: string) => {
      switch (name) {
        case '"':
          return unnamed;
        case "0":
          return zero;
        case "+":
          return plus;
        default:
          return makeRegister();
      }
    });
    vi.spyOn(Vim, "getRegisterController").mockReturnValue({
      getRegister,
      // The controller exposes far more than copyText uses; cast through unknown.
    } as unknown as ReturnType<typeof Vim.getRegisterController>);

    await copyText("hi");

    // OS clipboard written.
    expect(writeText).toHaveBeenCalledExactlyOnceWith("hi");
    // The unnamed (`"`) and yank (`0`) registers — which a bare `p` reads —
    // now hold the copied text.
    expect(unnamed.text).toBe("hi");
    expect(zero.text).toBe("hi");
  });

  it("still writes the OS clipboard and does not throw when the register controller is unavailable", async () => {
    const writeText = vi
      .spyOn(navigator.clipboard, "writeText")
      .mockResolvedValue(undefined);
    vi.spyOn(Vim, "getRegisterController").mockImplementation(() => {
      throw new Error("vim global state not initialised");
    });

    await expect(copyText("hi")).resolves.toBeUndefined();
    expect(writeText).toHaveBeenCalledExactlyOnceWith("hi");
  });
});
