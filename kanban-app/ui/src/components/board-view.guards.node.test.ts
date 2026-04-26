/**
 * Source-level guards for `board-view.tsx` — the legacy keyboard-nav
 * vestiges must stay deleted.
 *
 * These tests grep the shipped `board-view.tsx` source for tokens that
 * should no longer appear in it. They protect against regressions where a
 * future edit reintroduces pull-based claim machinery, neighbor-moniker
 * plumbing, or a board-level keydown listener that bypasses spatial nav.
 *
 * Node-only because they read the source file from disk; lives under the
 * `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to `board-view.tsx`, the file under guard. */
const BOARD_VIEW_PATH = resolve(__dirname, "board-view.tsx");

/** Read the actual `board-view.tsx` source as a string. */
function readBoardViewSource(): string {
  return readFileSync(BOARD_VIEW_PATH, "utf-8");
}

describe("BoardView source-level guards", () => {
  it("does not import ClaimPredicate", () => {
    const src = readBoardViewSource();
    expect(src).not.toMatch(/\bClaimPredicate\b/);
  });

  it("contains no neighbor-moniker plumbing", () => {
    const src = readBoardViewSource();
    // These names belonged to the legacy pull-based claim machinery — every
    // occurrence is a regression.
    expect(src).not.toMatch(/\bleftColumnTaskMonikers\b/);
    expect(src).not.toMatch(/\brightColumnTaskMonikers\b/);
    expect(src).not.toMatch(/\baboveTaskMonikers\b/);
    expect(src).not.toMatch(/\bbelowTaskMonikers\b/);
    expect(src).not.toMatch(/\ballBoardTaskMonikers\b/);
    expect(src).not.toMatch(/\ballBoardHeaderMonikers\b/);
    expect(src).not.toMatch(/\bcardClaimPredicates\b/);
    expect(src).not.toMatch(/\bnameFieldClaimWhen\b/);
  });

  it("does not register a board-level keyboard listener (only DnD escape stays)", () => {
    const src = readBoardViewSource();
    // No `onKeyDown` JSX prop anywhere — the spatial-nav layer owns keys.
    expect(src).not.toMatch(/\bonKeyDown\s*=/);

    // The only legitimate `keydown` listener is the DnD task-drag escape
    // cancel inside `useTaskDragEscapeCancel`, which is gated on `taskDrag`
    // and unrelated to keyboard navigation. Assert that and nothing else:
    // both (`addEventListener` and `removeEventListener`) must live inside
    // that hook.
    const matches = [...src.matchAll(/['"]keydown['"]/g)];
    expect(matches.length).toBe(2); // addEventListener + removeEventListener
    const escapeHook = src.indexOf("useTaskDragEscapeCancel");
    expect(escapeHook).toBeGreaterThan(-1);
    for (const m of matches) {
      expect(m.index!).toBeGreaterThan(escapeHook);
    }
  });

  it('wraps the board content in <FocusZone moniker={asMoniker("ui:board")}>', () => {
    const src = readBoardViewSource();
    // Look for the literal pattern that anchors the board zone. The exact
    // moniker token must be `"ui:board"` — using a different string would
    // miss the moniker convention.
    expect(src).toMatch(/<FocusZone\s+moniker={asMoniker\("ui:board"\)/);
  });
});
