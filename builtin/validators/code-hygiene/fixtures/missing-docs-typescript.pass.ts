/**
 * The passing fixture of the `missing-docs-typescript` tool rule.
 *
 * Every exported item here has documentation. The tool must report nothing. A
 * tool upgrade that reports one anyway makes the doctor mark the rule unusable.
 */

/** A documented exported interface. */
export interface DocumentedInterface {
  /** A documented property of the interface. */
  documentedProperty: string;
}

/** A documented exported type alias. */
export type DocumentedAlias = string;

/** A documented exported enumeration. */
export enum DocumentedEnum {
  /** The only member of the enumeration. */
  DocumentedMember = "documented",
}

/** A documented exported class. */
export class DocumentedClass {
  /** A documented public method. */
  documentedMethod(): void {}
}

/** A documented exported function. */
export function documentedFunction(): void {}
