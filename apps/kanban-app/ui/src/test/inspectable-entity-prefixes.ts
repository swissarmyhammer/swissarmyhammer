/**
 * Inspectable-entity moniker prefixes — the webview-side copy of the
 * `INSPECTABLE_ENTITY_PREFIXES` list declared in
 * `builtin/plugins/app-shell-commands/commands/ui.ts` (the server-side filter
 * `entity.inspect` / `app.inspect` use to resolve their target from a
 * dispatch's scope chain).
 *
 * `field:` is deliberately NOT in this list (kanban card
 * 01KTY6XTJQFCG9ENKTAMC6N3JV): a `field:{type}:{id}.{name}` moniker is a
 * projection of its CONTAINING entity, so chain resolution skips it and the
 * containing task wins. Fields remain inspectable via an explicit target
 * (the double-click `<Inspectable>` route) — see
 * `focus-architecture.guards.node.test.ts`, which extends this list with
 * `field:` for its Inspectable call-site guards.
 *
 * The plugin module is NOT importable from vitest — it imports
 * `@swissarmyhammer/plugin`, which exists only inside the embedded plugin
 * runtime — so this copy is the single webview-side source of the list:
 *
 *   - `focus-architecture.guards.node.test.ts` (Guards B + C) pins it
 *     (plus `field:`) against the `<Inspectable>` JSX call sites.
 *   - `ui-plugin-inspectable-prefixes-mirror.spatial.node.test.ts` pins it
 *     against the plugin source parsed from disk.
 *
 * Adding a new inspectable entity kind? Add it to the plugin's
 * `INSPECTABLE_ENTITY_PREFIXES` AND here — the mirror guard fails loudly on
 * any drift between the two. (The Rust caption renderer carries a third
 * copy, pinned by `swissarmyhammer-command-service`'s
 * `tests/inspectable_prefixes_mirror.rs`.)
 */
export const INSPECTABLE_ENTITY_PREFIXES: readonly string[] = [
  "task:",
  "tag:",
  "column:",
  "board:",
  "attachment:",
];
