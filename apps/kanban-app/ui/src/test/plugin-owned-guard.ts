/**
 * Shared scan/detector helpers for the static architectural guards that walk
 * the UI `src/` tree (`board-commands.plugin-owned.node.test.ts`,
 * `grid-commands.plugin-owned.node.test.ts`,
 * `editor-drill-in-commands.plugin-owned.node.test.ts`,
 * `webview-command-bus.guard.node.test.ts`).
 *
 * The plugin-owned guards all assert the same invariant — a command id family
 * is DEFINED only by its builtin plugin, never by a client-built `CommandDef`
 * in React — and vary only in the id pattern they hunt for, so the detector is
 * parameterized on that pattern (`definesPluginCommand`) and the directory
 * scan on top of it is shared (`findCommandDefinitionOffenders`). The bus
 * guard has its own detectors but reuses the same source walk
 * (`collectSourceFiles`). Each guard file still unit-proves its detector
 * against known-good and known-bad source so the scan stays trustworthy.
 */
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

// Vitest runs with cwd = the ui project root (where vite.config.ts lives).
export const SRC_ROOT = join(process.cwd(), "src");

/**
 * Recursively collect non-test `.ts`/`.tsx` source files under `dir`.
 *
 * Skips `node_modules` and the `test/` harness directory (its registry
 * mirrors legitimately carry the plugin metadata, including the guarded
 * command ids).
 *
 * @param dir - The directory to walk (typically `SRC_ROOT`).
 * @returns Absolute paths of every matching source file.
 */
export function collectSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === "test") continue;
      out.push(...collectSourceFiles(full));
      continue;
    }
    if (!/\.tsx?$/.test(entry.name)) continue;
    if (/\.test\.tsx?$/.test(entry.name)) continue;
    out.push(full);
  }
  return out;
}

/**
 * Whether `source` defines a command object whose id matches `idPattern`.
 *
 * The structural smell is an object-literal `id:` property holding a matching
 * string — the shape every `CommandDef` / command-object construction uses, in
 * any quote style including a template literal. Bus registrations
 * (`registerWebviewCommandHandler("…", …)`) pass the id as a bare call
 * argument / record key, never as an `id:` property, so handler-registration
 * sites do not trip the detector.
 *
 * @param source - The source text to inspect.
 * @param idPattern - Regex source matched against the start of the id string
 *   (e.g. `String.raw`board\.`` or an alternation of full ids).
 */
export function definesPluginCommand(
  source: string,
  idPattern: string,
): boolean {
  return new RegExp(`\\bid:\\s*["'\`](?:${idPattern})`).test(source);
}

/**
 * Scan every non-test source file under `SRC_ROOT` and return the files that
 * define a command object matching `idPattern` — empty when the plugin owns
 * every definition.
 *
 * @param idPattern - Regex source for the guarded id family (see
 *   {@link definesPluginCommand}).
 */
export function findCommandDefinitionOffenders(idPattern: string): string[] {
  return collectSourceFiles(SRC_ROOT).filter((f) =>
    definesPluginCommand(readFileSync(f, "utf8"), idPattern),
  );
}
