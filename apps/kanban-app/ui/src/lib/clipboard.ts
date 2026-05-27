/**
 * Shared clipboard helper for the AI chat copy buttons.
 *
 * A plain `navigator.clipboard.writeText` is not enough for vim users. In
 * `@replit/codemirror-vim` (the app's vim mode, wired in `cm-keymap.ts`), a
 * bare `p` pastes **synchronously** from the register named by the paste
 * action — `undefined` for a bare `p`, which resolves to the **unnamed**
 * register (`"`). Only `"+p` reads the OS clipboard (`navigator.clipboard
 * .readText()`); the unnamed register is populated by in-editor yanks/deletes,
 * never by an external `writeText`. So after a chat Copy, a bare `p` would see
 * an empty/stale register and paste nothing.
 *
 * {@link copyText} therefore does both: it writes the OS clipboard (keeping
 * CUA/emacs `Cmd/Ctrl+V` and system paste working) *and* mirrors the text into
 * the vim registers a bare `p` reads, so the copied text lands wherever vim
 * pastes.
 */

import { Vim } from "@replit/codemirror-vim";

/**
 * Mirror `text` into the vim registers a paste reads.
 *
 * Sets the unnamed (`"`) and yank (`0`) registers — which a bare `p` reads —
 * plus the `+` clipboard register so `"+p` stays consistent with the OS write.
 * Best-effort: if the vim global state has not been initialised (no vim editor
 * has mounted yet), `getRegisterController` throws and we silently skip — the
 * OS clipboard write has already happened.
 */
function mirrorToVimRegisters(text: string): void {
  try {
    const controller = Vim.getRegisterController();
    controller.getRegister('"').setText(text);
    controller.getRegister("0").setText(text);
    controller.getRegister("+").setText(text);
  } catch {
    // Vim global state not ready (e.g. no vim editor mounted) — the OS
    // clipboard write already succeeded, so there is nothing more to do.
  }
}

/**
 * Copy `text` to the OS clipboard and mirror it into the vim registers.
 *
 * The vim-register mirror runs FIRST, before the OS write. The mirror is the
 * load-bearing half for the reported bug (a bare `p` paste) and does not depend
 * on the OS clipboard, whereas `writeText` is the part that can be denied
 * (restricted clipboard permission). Mirroring first means a bare `p` still
 * pastes the copied text even when the OS write is refused; the write error is
 * still re-thrown afterward so callers' existing logging fires.
 *
 * @param text - The text to copy.
 * @throws Re-throws any error from `navigator.clipboard.writeText` (callers
 *   keep their existing `.catch` / `try-catch` logging). The vim-register
 *   mirror never throws.
 */
export async function copyText(text: string): Promise<void> {
  mirrorToVimRegisters(text);
  await navigator.clipboard.writeText(text);
}
