/**
 * Inspectable-entity moniker prefixes — the webview-side copy of the
 * `INSPECTABLE_ENTITY_PREFIXES` list declared in
 * `builtin/plugins/ui-commands/index.ts` (the server-side filter
 * `entity.inspect` uses to resolve its target from a dispatch's scope chain).
 *
 * The plugin module is NOT importable from vitest — it imports
 * `@swissarmyhammer/plugin`, which exists only inside the embedded plugin
 * runtime — so this copy is the single webview-side source of the list:
 *
 *   - `focus-architecture.guards.node.test.ts` (Guards B + C) pins it
 *     against the `<Inspectable>` JSX call sites.
 *   - `ui-plugin-inspectable-prefixes-mirror.spatial.node.test.ts` pins it
 *     against the plugin source parsed from disk.
 *
 * Adding a new inspectable entity kind? Add it to the plugin's
 * `INSPECTABLE_ENTITY_PREFIXES` AND here — the mirror guard fails loudly on
 * any drift between the two.
 */
export const INSPECTABLE_ENTITY_PREFIXES: readonly string[] = [
  "task:",
  "tag:",
  "column:",
  "board:",
  "field:",
  "attachment:",
];
