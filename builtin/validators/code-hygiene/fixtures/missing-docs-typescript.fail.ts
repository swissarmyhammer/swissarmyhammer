/**
 * The failing fixture of the `missing-docs-typescript` tool rule.
 *
 * It holds one undocumented exported item. The tool must report it. A tool
 * upgrade that stops reporting it makes the doctor mark the rule unusable.
 */

/** A documented exported function, so only the item below is reported. */
export function documentedNeighbor(): void {}

export function undocumentedFunction(): void {}
