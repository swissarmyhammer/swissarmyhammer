/**
 * Architectural guard: `entity.inspect` and `nav.focus` are DEFINED only by
 * their builtin plugins — no client-built `CommandDef` in the webview may
 * define either id.
 *
 * Card G of the ui-command-cleanup project consolidated the two last
 * multiply-defined command ids:
 *
 *   - `entity.inspect` — formerly defined THREE times client-side (the
 *     `app-shell.tsx` root-scope Space fallback `buildRootInspectCommand`,
 *     the per-`<Inspectable>` scope `CommandDef` in `inspectable.tsx`, and
 *     the keymap's static Space routing). It is now defined ONCE in
 *     `builtin/plugins/ui-commands/index.ts`: a global Space command whose
 *     execute resolves the target SERVER-SIDE (explicit `ctx.target`, else
 *     the innermost inspectable moniker in the scope chain, else an inert
 *     no-op).
 *
 *   - `nav.focus` — formerly defined in BOTH focus contexts
 *     (`entity-focus-context.tsx` / `spatial-focus-context.tsx`) as scope
 *     `CommandDef`s taking the execute fast-path. It is now defined ONCE in
 *     `builtin/plugins/nav-commands/index.ts`; the webview's only remaining
 *     role is BEHAVIOR — `SpatialFocusProvider` registers a webview-bus
 *     handler (`registerWebviewCommandHandler("nav.focus", …)`) that runs
 *     the snapshot-bearing `actions.focus(fq)` commit, exactly the Card B–F
 *     plugin-definition / webview-behavior split.
 *
 * A `CommandDef` with either id reappearing anywhere in `src/` would
 * re-split the ownership this card unified. The structural smell is an
 * object-literal `id:` property holding the exact id string (see
 * `@/test/plugin-owned-guard`); bus registrations pass the id as a bare
 * call argument and never trip the scan.
 *
 * As with the other `*.plugin-owned.node.test.ts` guards, the detector is
 * unit-proven against known-good and known-bad source below so the
 * directory scan is trustworthy.
 */
import { describe, it, expect } from "vitest";
import {
  definesPluginCommand,
  findCommandDefinitionOffenders,
} from "@/test/plugin-owned-guard";

/** Regex source for the guarded ids: exactly `entity.inspect` / `nav.focus`. */
const GUARDED_ID_PATTERN = String.raw`entity\.inspect|nav\.focus`;

/** Whether `source` defines a command object with a guarded id. */
function definesGuardedCommand(source: string): boolean {
  return definesPluginCommand(source, GUARDED_ID_PATTERN);
}

describe("entity.inspect / nav.focus definitions are plugin-owned", () => {
  it("detects a client-side CommandDef for either id (detector is sound)", () => {
    expect(
      definesGuardedCommand(
        'const cmds = [{ id: "entity.inspect", name: "Inspect" }];',
      ),
    ).toBe(true);
    expect(definesGuardedCommand("{ id: 'nav.focus' }")).toBe(true);
  });

  it("does not flag bus registrations or unrelated ids (no false positives)", () => {
    expect(
      definesGuardedCommand(
        'registerWebviewCommandHandler("nav.focus", () => {});',
      ),
    ).toBe(false);
    expect(definesGuardedCommand('dispatch("entity.inspect")')).toBe(false);
    expect(
      definesGuardedCommand('useDispatchCommand("nav.focus")({ args: {} })'),
    ).toBe(false);
    expect(definesGuardedCommand('{ id: "entity.add" }')).toBe(false);
    // A comment mentioning a guarded id must not trip the detector.
    expect(definesGuardedCommand("// the entity.inspect command")).toBe(false);
  });

  it("no client source defines an entity.inspect or nav.focus CommandDef — the plugins own the definitions", () => {
    const offenders = findCommandDefinitionOffenders(GUARDED_ID_PATTERN);

    // A guarded id defined in React re-splits command ownership. Define
    // `entity.inspect` in `builtin/plugins/ui-commands/index.ts` and
    // `nav.focus` in `builtin/plugins/nav-commands/index.ts`; register
    // webview behavior via `registerWebviewCommandHandler` instead.
    expect(offenders).toEqual([]);
  });
});
