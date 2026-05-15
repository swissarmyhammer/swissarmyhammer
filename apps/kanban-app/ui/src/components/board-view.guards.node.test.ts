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

  it("wraps the board content in <FocusScope moniker={asSegment(board.board.moniker)}>", () => {
    const src = readBoardViewSource();
    // Post-`8232b25cc`, the redundant `ui:board` chrome scope was
    // dropped. The board content now mounts directly under the outer
    // `<FocusScope moniker={asSegment(board.board.moniker)}>` (i.e.
    // the `board:<id>` entity scope). Pin that the entity-moniker
    // wrapping is still in place.
    expect(src).toMatch(
      /<FocusScope\s+moniker={asSegment\(board\.board\.moniker\)}/,
    );
    // And the dropped `ui:board` chrome must stay gone — a regression
    // that re-introduces it would re-create the same-rect overlap warning
    // that motivated its removal.
    expect(src).not.toMatch(/asSegment\("ui:board"\)/);
  });
});
