/**
 * Architectural guard: the three editor drill-in commands are DEFINED only by
 * the `app-shell-commands` builtin plugin — no client-built `CommandDef` in
 * the webview may define `filter_editor.drillIn`,
 * `app.ai-panel.composer.drillIn`, or
 * `app.ai-panel.elicitation.field.drillIn` (in either the live `app.*` or the
 * retired `ui.*` spelling, including the retired per-field minted form
 * `...elicitation.field.drillIn:{key}`).
 *
 * Card E of the ui-command-cleanup project moved the drill-in definitions
 * (id / name / keys / scope) out of `perspective-tab-bar.tsx`,
 * `ai-prompt-composer.tsx`, and `ai-elements/elicitation.tsx` into the
 * builtin plugin layer; the ui.*→app.* rename then folded the bundle into
 * `builtin/plugins/app-shell-commands/commands/ui.ts`
 * (`UI_SURFACE_COMMANDS`). The React
 * tree's only remaining role is BEHAVIOR: each editor surface registers a
 * webview-bus handler for its id while spatial focus is within its subtree
 * (`useFocusedWebviewCommandHandlers`). A drill-in `CommandDef` reappearing
 * anywhere in `src/` would re-split the ownership this card unified.
 *
 * The structural smell is an object-literal `id:` property holding one of the
 * drill-in ids — the shape every `CommandDef` construction uses, in any quote
 * style including a template literal (the retired elicitation def minted its
 * id as `` `app.ai-panel.elicitation.field.drillIn:${key}` ``; see
 * `@/test/plugin-owned-guard`). Bus registrations pass the id as a bare call
 * argument / record key, never as an `id:` property, so
 * handler-registration sites do not trip the scan.
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

/** Regex source for the guarded id family: the three editor drill-in ids.
 * Matches BOTH the live `app.ai-panel.*` ids and the retired `ui.ai-panel.*`
 * spellings, so neither form may ever reappear as a client `CommandDef`. */
const DRILL_IN_ID_PATTERN = String.raw`filter_editor\.drillIn|(?:ui|app)\.ai-panel\.composer\.drillIn|(?:ui|app)\.ai-panel\.elicitation\.field\.drillIn`;

/** Whether `source` defines a command object with an editor drill-in id. */
function definesEditorDrillInCommand(source: string): boolean {
  return definesPluginCommand(source, DRILL_IN_ID_PATTERN);
}

describe("editor drill-in command definitions are plugin-owned", () => {
  it("detects a client-side drill-in CommandDef (detector is sound)", () => {
    expect(
      definesEditorDrillInCommand(
        'const cmds = [{ id: "filter_editor.drillIn", name: "Edit Filter" }];',
      ),
    ).toBe(true);
    expect(
      definesEditorDrillInCommand("{ id: 'app.ai-panel.composer.drillIn' }"),
    ).toBe(true);
    // The retired per-field minted template-literal form must also trip it.
    expect(
      definesEditorDrillInCommand(
        "{ id: `app.ai-panel.elicitation.field.drillIn:${key}` }",
      ),
    ).toBe(true);
  });

  it("does not flag bus handler registrations or unrelated ids (no false positives)", () => {
    expect(
      definesEditorDrillInCommand(
        'registerWebviewCommandHandler("filter_editor.drillIn", () => {});',
      ),
    ).toBe(false);
    expect(
      definesEditorDrillInCommand(
        '{ "app.ai-panel.composer.drillIn": handler }',
      ),
    ).toBe(false);
    expect(definesEditorDrillInCommand('{ id: "nav.drillIn" }')).toBe(false);
    // A comment mentioning a drill-in id must not trip the detector.
    expect(
      definesEditorDrillInCommand("// the filter_editor.drillIn command"),
    ).toBe(false);
  });

  it("no client source defines an editor drill-in CommandDef — the plugin owns the definitions", () => {
    const offenders = findCommandDefinitionOffenders(DRILL_IN_ID_PATTERN);

    // A drill-in id defined in React re-splits command ownership. Define it in
    // `builtin/plugins/app-shell-commands/commands/ui.ts` and register the
    // behavior via `useFocusedWebviewCommandHandlers` instead.
    expect(offenders).toEqual([]);
  });
});
