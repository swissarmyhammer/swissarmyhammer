/**
 * The failing fixture of the `missing-docs-typescript` tool rule.
 *
 * It holds one undocumented exported item of every kind the passing fixture
 * documents. The tool must report each one. A tool upgrade that stops
 * reporting a kind makes the doctor mark the rule unusable.
 */

/** A documented exported function, so only the items below are reported. */
export function documentedNeighbor(): void {}

export interface UndocumentedInterface {
  undocumentedProperty: string;
}

export type UndocumentedAlias = string;

export enum UndocumentedEnum {
  UndocumentedMember = "undocumented",
}

export class UndocumentedClass {
  undocumentedMethod(): void {}
}

export function undocumentedFunction(): void {}
