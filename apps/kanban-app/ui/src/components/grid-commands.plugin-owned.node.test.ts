/**
 * Architectural guard: the eleven `grid.*` commands are DEFINED only by the
 * `grid-commands` builtin plugin — no client-built `CommandDef` list in the
 * webview may define a `grid.*` id.
 *
 * Card C of the ui-command-cleanup project moved the grid command definitions
 * (id / name / keys / scope) out of `grid-view.tsx` into
 * `builtin/plugins/grid-commands/index.ts`. The React tree's only remaining
 * role is BEHAVIOR: it registers a webview-bus handler per id
 * (`registerWebviewCommandHandler`, Card B). A `CommandDef` with a `grid.*`
 * id reappearing anywhere in `src/` would re-split the ownership this card
 * unified — definitions drifting between the plugin and React is exactly the
 * failure mode that motivated the move.
 *
 * The structural smell is an object-literal `id:` property holding a string
 * that starts with `grid.` — the shape every `CommandDef` / command-object
 * construction uses (see `@/test/plugin-owned-guard`). Bus registrations
 * (`registerWebviewCommandHandler("grid.edit", …)`) pass the id as a bare
 * call argument, never as an `id:` property, so handler-registration sites do
 * not trip the scan.
 *
 * As with `webview-command-bus.guard.node.test.ts`, the detector is
 * unit-proven against known-good and known-bad source below so the directory
 * scan is trustworthy.
 */
import { describe, it, expect } from "vitest";
import {
  definesPluginCommand,
  findCommandDefinitionOffenders,
} from "@/test/plugin-owned-guard";

/** Regex source for the guarded id family: any id starting with `grid.`. */
const GRID_ID_PATTERN = String.raw`grid\.`;

/** Whether `source` defines a command object with a `grid.*` id. */
function definesGridCommand(source: string): boolean {
  return definesPluginCommand(source, GRID_ID_PATTERN);
}

describe("grid.* command definitions are plugin-owned", () => {
  it("detects a client-side grid CommandDef (detector is sound)", () => {
    expect(
      definesGridCommand('const cmds = [{ id: "grid.edit", name: "Edit" }];'),
    ).toBe(true);
    expect(definesGridCommand("{ id: 'grid.deleteRow' }")).toBe(true);
  });

  it("does not flag bus handler registrations or unrelated ids (no false positives)", () => {
    expect(
      definesGridCommand(
        'registerWebviewCommandHandler("grid.edit", () => {});',
      ),
    ).toBe(false);
    expect(definesGridCommand('dispatch("grid.deleteRow")')).toBe(false);
    expect(definesGridCommand('{ id: "nav.jump" }')).toBe(false);
    // A comment mentioning a grid id must not trip the detector.
    expect(definesGridCommand("// the grid.edit command")).toBe(false);
  });

  it("no client source defines a grid.* CommandDef — the plugin owns the definitions", () => {
    const offenders = findCommandDefinitionOffenders(GRID_ID_PATTERN);

    // A grid.* id defined in React re-splits command ownership. Define it in
    // `builtin/plugins/grid-commands/index.ts` and register the behavior via
    // `registerWebviewCommandHandler` instead.
    expect(offenders).toEqual([]);
  });
});
