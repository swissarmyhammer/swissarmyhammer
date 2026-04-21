/**
 * Type definitions + JSON loader for the JS-shim ↔ Rust parity fixtures.
 *
 * The fixture data itself lives in `spatial-parity-cases.json`. Both the
 * JS parity test (this file) and the Rust parity test (in
 * `swissarmyhammer-spatial-nav/tests/parity.rs`) consume the same JSON
 * file — Rust via `include_str!` + serde.
 *
 * ### Why one fixture list, not two parallel test suites
 *
 * Two-impl behavioral equivalence is the entire contract of the shim.
 * If the two implementations drift — Rust adds an edge case, JS doesn't;
 * or JS scores something differently from Rust — the vitest-browser
 * tests silently become meaningless because the shim's `focus-changed`
 * event no longer matches what production emits.
 *
 * Sharing a single fixture list means:
 * - Every behavioral assertion is specified once.
 * - Adding a test case means writing one JSON entry, not two test files.
 * - The JS parity test imports the list and runs it against the shim.
 * - The Rust parity test deserializes the same list and runs it through
 *   production `SpatialState`. Divergence surfaces as a failure on one
 *   side or the other.
 */

import type { ShimDirection } from "./spatial-shim";
import rawCases from "./spatial-parity-cases.json";

/** Rect in fixture format (fields match Rust snake_case). */
export interface ParityRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Entry to register as part of a setup step. */
export interface ParityEntry {
  key: string;
  moniker: string;
  rect: ParityRect;
  layer_key: string;
  parent_scope: string | null;
  /** Direction → moniker | null (null blocks, missing falls through). */
  overrides: Record<string, string | null>;
}

/** One operation in the case's operation sequence. */
export type ParityOp =
  | { op: "push_layer"; key: string; name: string }
  | { op: "remove_layer"; key: string }
  | { op: "register"; entry: ParityEntry }
  | { op: "unregister"; key: string }
  | { op: "focus"; key: string }
  | { op: "clear_focus" }
  | { op: "navigate"; from_key: string; direction: ShimDirection }
  | { op: "focus_first_in_layer"; layer_key: string };

/** Event (or no-op) we expect the implementation to emit after an op. */
export interface ParityExpect {
  event: { prev_key: string | null; next_key: string | null } | null;
  focused: string | null;
}

/** One parity case: a name, then a list of (op, expected) pairs. */
export interface ParityCase {
  name: string;
  steps: Array<{ op: ParityOp; expect: ParityExpect }>;
}

/**
 * The shared list of parity cases, loaded from the JSON fixture.
 *
 * The JSON file is the source of truth — Rust reads the same file
 * byte-for-byte through `include_str!`. Keep assertions tight: every
 * case is expected to agree across implementations.
 */
export const PARITY_CASES: ParityCase[] = rawCases as ParityCase[];
