/**
 * Architectural guard: no client source mints a `view.switch:*` command id.
 *
 * Card H of the ui-command-cleanup project removed the last client-minted
 * `view.switch:${id}` indirection from `views-container.tsx`. View switching
 * is fully canonical now:
 *
 *   - The webview dispatches `view.set` with the view id in `args.view_id`
 *     (LeftNav's `ViewButton`, plugin definition in
 *     `builtin/plugins/app-shell-commands/commands/ui.ts`).
 *   - The palette's per-view "Switch to <ViewName>" rows are emitted by Rust
 *     (`swissarmyhammer_kanban::scope_commands::emit_view_switch`) as
 *     `view.set` rows with pre-filled args — never as `view.switch:*` ids.
 *   - The dispatcher-side `view.switch:*` rewrite was retired in
 *     01KPZMXXEXKVE3RNPA4XJP0105, so a minted id would not even resolve.
 *
 * A `view.switch:*` id reappearing in a client-built `CommandDef` would
 * resurrect a command-id-as-data indirection the backend no longer
 * understands. The structural smell is an object-literal `id:` property
 * holding a string starting with `view.switch` (any quote style, including
 * template literals like `` id: `view.switch:${view.id}` `` — see
 * `@/test/plugin-owned-guard`).
 *
 * As with the sibling plugin-owned guards, the detector is unit-proven
 * against known-good and known-bad source so the directory scan stays
 * trustworthy.
 */
import { describe, it, expect } from "vitest";
import {
  definesPluginCommand,
  findCommandDefinitionOffenders,
} from "@/test/plugin-owned-guard";

/** Regex source for the retired id family: any id starting with `view.switch`. */
const VIEW_SWITCH_ID_PATTERN = String.raw`view\.switch`;

/** Whether `source` defines a command object with a `view.switch*` id. */
function definesViewSwitchCommand(source: string): boolean {
  return definesPluginCommand(source, VIEW_SWITCH_ID_PATTERN);
}

describe("view.switch:* command ids are retired", () => {
  it("detects a client-minted view.switch CommandDef (detector is sound)", () => {
    expect(
      definesViewSwitchCommand(
        'const cmds = [{ id: "view.switch:board-default", name: "View: Board" }];',
      ),
    ).toBe(true);
    // The exact shape the removed indirection used: a template-literal id.
    expect(definesViewSwitchCommand("({ id: `view.switch:${view.id}` })")).toBe(
      true,
    );
  });

  it("does not flag dispatches, comments, or unrelated ids (no false positives)", () => {
    expect(definesViewSwitchCommand('dispatch("view.switch:v1")')).toBe(false);
    expect(definesViewSwitchCommand("// the view.switch:* indirection")).toBe(
      false,
    );
    expect(definesViewSwitchCommand('{ id: "view.set" }')).toBe(false);
  });

  it("no client source defines a view.switch:* CommandDef — the indirection is gone", () => {
    const offenders = findCommandDefinitionOffenders(VIEW_SWITCH_ID_PATTERN);

    // A view.switch:* id minted in React is dead indirection: the backend
    // retired the rewrite in 01KPZMXXEXKVE3RNPA4XJP0105. Dispatch the
    // canonical `view.set` with `args.view_id` instead.
    expect(offenders).toEqual([]);
  });
});
