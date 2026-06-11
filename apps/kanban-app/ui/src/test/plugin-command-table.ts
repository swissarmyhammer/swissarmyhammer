/**
 * Shared parsing/comparison helpers for the plugin-command mirror drift
 * guards (`nav-plugin-commands-mirror.spatial.node.test.ts`,
 * `grid-plugin-commands-mirror.spatial.node.test.ts`).
 *
 * The builtin plugin modules (`builtin/plugins/<bundle>/index.ts`) are NOT
 * importable from vitest — they import `@swissarmyhammer/plugin`, which exists
 * only inside the embedded plugin runtime — so each test keeps a manual mirror
 * in `mock-command-list.ts` and guards it against the plugin SOURCE read from
 * disk. These helpers parse a `const <TABLE> = [ ... ];` data table of
 * `{ id, name, keys? }` entries out of that source and diff a mirror against
 * it in both directions.
 */

/** One `{ id, name, keys }` triple — the shape both sides are compared as. */
export interface PluginCommandEntry {
  id: string;
  name: string;
  keys: Record<string, string>;
}

/**
 * Parse a named command data table out of a plugin source text.
 *
 * Tolerant of formatting (whitespace, line breaks, entry order) but anchored
 * on the declaration name and the `id:` / `name:` / `keys:` properties, so a
 * structural rewrite of the table fails the guard (by parsing zero entries)
 * rather than passing vacuously. Entries without a `keys:` block parse with
 * empty keys.
 *
 * @param source - The plugin module source text.
 * @param tableName - The `const` identifier of the data table (e.g.
 *   `"NAV_DIRECTIONS"`, `"GRID_COMMANDS"`).
 */
export function parseCommandTable(
  source: string,
  tableName: string,
): PluginCommandEntry[] {
  const table = new RegExp(`${tableName}[^=]*=\\s*\\[([\\s\\S]*?)\\n\\];`).exec(
    source,
  )?.[1];
  if (!table) return [];
  // Split the table body into per-entry slices anchored on each `id:`.
  const idRe = /id:\s*"([^"]+)"/g;
  const anchors: Array<{ id: string; start: number }> = [];
  for (let m = idRe.exec(table); m; m = idRe.exec(table)) {
    anchors.push({ id: m[1], start: m.index });
  }
  return anchors.map(({ id, start }, i) => {
    const slice = table.slice(start, anchors[i + 1]?.start ?? table.length);
    const name = /name:\s*"([^"]+)"/.exec(slice)?.[1] ?? "";
    const keysBody = /keys:\s*\{([^}]*)\}/.exec(slice)?.[1] ?? "";
    const keys: Record<string, string> = {};
    const kvRe = /(\w+):\s*"([^"]*)"/g;
    for (let kv = kvRe.exec(keysBody); kv; kv = kvRe.exec(keysBody)) {
      keys[kv[1]] = kv[2];
    }
    return { id, name, keys };
  });
}

/**
 * Compare a mirror against the parsed plugin table and return a list of
 * human-readable mismatches — empty when the mirror is faithful. Checks id
 * set equality in both directions plus per-id `name` and `keys` equality.
 *
 * @param mirror - The test mirror entries (from `mock-command-list.ts`).
 * @param pluginEntries - The entries parsed from the plugin source.
 * @param tableName - The plugin table's name, for the not-found message.
 */
export function mirrorMismatches(
  mirror: ReadonlyArray<{
    id: string;
    name: string;
    keys?: Record<string, string>;
  }>,
  pluginEntries: ReadonlyArray<PluginCommandEntry>,
  tableName: string,
): string[] {
  const mismatches: string[] = [];
  if (pluginEntries.length === 0) {
    return [`${tableName} table not found / not parseable in plugin source`];
  }
  const byId = new Map(pluginEntries.map((e) => [e.id, e]));
  for (const m of mirror) {
    const plugin = byId.get(m.id);
    if (!plugin) {
      mismatches.push(`mirror id ${m.id} not present in plugin ${tableName}`);
      continue;
    }
    if (plugin.name !== m.name) {
      mismatches.push(
        `name mismatch for ${m.id}: mirror "${m.name}" vs plugin "${plugin.name}"`,
      );
    }
    const mirrorKeys = m.keys ?? {};
    const modes = new Set([
      ...Object.keys(mirrorKeys),
      ...Object.keys(plugin.keys),
    ]);
    for (const mode of modes) {
      if (mirrorKeys[mode] !== plugin.keys[mode]) {
        mismatches.push(
          `keys.${mode} mismatch for ${m.id}: mirror ${JSON.stringify(
            mirrorKeys[mode],
          )} vs plugin ${JSON.stringify(plugin.keys[mode])}`,
        );
      }
    }
  }
  const mirrorIds = new Set(mirror.map((m) => m.id));
  for (const e of pluginEntries) {
    if (!mirrorIds.has(e.id)) {
      mismatches.push(`plugin id ${e.id} missing from mirror`);
    }
  }
  return mismatches;
}
