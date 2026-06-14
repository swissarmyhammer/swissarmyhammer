/**
 * Architectural guard: the three `board.*` commands are DEFINED only by the
 * `board-commands` builtin plugin — no client-built `CommandDef` list in the
 * webview may define a `board.*` id.
 *
 * Card F of the ui-command-cleanup project moved the board command
 * definitions (id / name / keys / scope) out of `board-view.tsx`
 * (`makeNewTaskCommand` / `makeNavCommand`) into
 * `builtin/plugins/board-commands/index.ts`. The React tree's only remaining
 * role is BEHAVIOR: it registers a webview-bus handler for `board.newTask`
 * (`registerWebviewCommandHandler`, Card B); `board.firstColumn` /
 * `board.lastColumn` execute server-side (focus `navigate focus`) and need no
 * handler at all. A `CommandDef` with a `board.*` id reappearing anywhere in
 * `src/` would re-split the ownership this card unified — definitions
 * drifting between the plugin and React is exactly the failure mode that
 * motivated the move.
 *
 * The structural smell is an object-literal `id:` property holding a string
 * that starts with `board.` — the shape every `CommandDef` / command-object
 * construction uses (see `@/test/plugin-owned-guard`). Bus registrations
 * (`registerWebviewCommandHandler("board.newTask", …)`) pass the id as a bare
 * call argument, never as an `id:` property, so handler-registration sites do
 * not trip the scan.
 *
 * As with `grid-commands.plugin-owned.node.test.ts`, the detector is
 * unit-proven against known-good and known-bad source below so the directory
 * scan is trustworthy.
 */
import { describe, it, expect } from "vitest";
import {
  definesPluginCommand,
  findCommandDefinitionOffenders,
} from "@/test/plugin-owned-guard";

/** Regex source for the guarded id family: any id starting with `board.`. */
const BOARD_ID_PATTERN = String.raw`board\.`;

/** Whether `source` defines a command object with a `board.*` id. */
function definesBoardCommand(source: string): boolean {
  return definesPluginCommand(source, BOARD_ID_PATTERN);
}

describe("board.* command definitions are plugin-owned", () => {
  it("detects a client-side board CommandDef (detector is sound)", () => {
    expect(
      definesBoardCommand(
        'const cmds = [{ id: "board.newTask", name: "New Task" }];',
      ),
    ).toBe(true);
    expect(definesBoardCommand("{ id: 'board.firstColumn' }")).toBe(true);
  });

  it("does not flag bus handler registrations or unrelated ids (no false positives)", () => {
    expect(
      definesBoardCommand(
        'registerWebviewCommandHandler("board.newTask", () => {});',
      ),
    ).toBe(false);
    expect(definesBoardCommand('dispatch("board.lastColumn")')).toBe(false);
    expect(definesBoardCommand('{ id: "grid.edit" }')).toBe(false);
    // A comment mentioning a board id must not trip the detector.
    expect(definesBoardCommand("// the board.newTask command")).toBe(false);
  });

  it("no client source defines a board.* CommandDef — the plugin owns the definitions", () => {
    const offenders = findCommandDefinitionOffenders(BOARD_ID_PATTERN);

    // A board.* id defined in React re-splits command ownership. Define it in
    // `builtin/plugins/board-commands/index.ts` and register the behavior via
    // `registerWebviewCommandHandler` instead.
    expect(offenders).toEqual([]);
  });
});
