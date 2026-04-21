/**
 * JS shim parity tests.
 *
 * Runs every scenario in `spatial-parity-cases.json` through the JS
 * `SpatialStateShim` and asserts that every step emits the expected
 * event (or no event) and leaves `focused_key` matching expectations.
 *
 * The matching Rust test (`swissarmyhammer-spatial-nav/tests/parity.rs`)
 * deserializes the same JSON file and asserts the same expectations
 * against the production `SpatialState`. If the two suites agree, the
 * shim is behavior-equivalent to Rust for every covered scenario; the
 * vitest-browser tier can rely on the shim to model Rust faithfully.
 *
 * Keep this test in the **browser** suite (filename ends `.test.ts`,
 * not `.node.test.ts`) so TypeScript sees the same module resolution
 * as the rest of the UI tests.
 */

import { describe, it, expect } from "vitest";
import { SpatialStateShim } from "./spatial-shim";
import { PARITY_CASES, type ParityOp } from "./spatial-parity-cases";
import type { FocusChangedPayload, ShimDirection } from "./spatial-shim";

/**
 * Apply one parity op to the shim and return the emitted event (or null).
 *
 * Mirrors the event-emission contract in `setup-spatial-shim.ts` and in
 * `kanban-app/src/spatial.rs`: every mutator that can change focus
 * returns `FocusChanged | null`.
 */
function applyOp(
  shim: SpatialStateShim,
  op: ParityOp,
): FocusChangedPayload | null {
  switch (op.op) {
    case "push_layer":
      shim.pushLayer(op.key, op.name);
      return null;
    case "remove_layer":
      return shim.removeLayer(op.key);
    case "register":
      shim.register({
        key: op.entry.key,
        moniker: op.entry.moniker,
        rect: {
          x: op.entry.rect.x,
          y: op.entry.rect.y,
          width: op.entry.rect.w,
          height: op.entry.rect.h,
        },
        layerKey: op.entry.layer_key,
        parentScope: op.entry.parent_scope,
        overrides: op.entry.overrides,
      });
      return null;
    case "unregister":
      return shim.unregister(op.key);
    case "focus":
      return shim.focus(op.key);
    case "clear_focus":
      return shim.clearFocus();
    case "navigate":
      return shim.navigate(op.from_key, op.direction as ShimDirection);
  }
}

describe("SpatialStateShim parity with Rust SpatialState", () => {
  for (const parityCase of PARITY_CASES) {
    it(parityCase.name, () => {
      const shim = new SpatialStateShim();
      parityCase.steps.forEach((step, i) => {
        const event = applyOp(shim, step.op);
        expect(event, `step ${i} (${step.op.op}) event`).toEqual(
          step.expect.event,
        );
        expect(
          shim.focusedKeySnapshot(),
          `step ${i} (${step.op.op}) focused key`,
        ).toEqual(step.expect.focused);
      });
    });
  }
});
