/**
 * Type-only tests for the kanban TS types in `kanban.ts`.
 *
 * These tests are mostly compile-time assertions: the runtime body is just
 * `expect(true).toBe(true)` so that vitest registers a test, but the real
 * verification is that the literals satisfy the type without a TS error.
 * If a future change ever breaks the type shape, `tsc --noEmit` (run as
 * part of `npm test`) fails the build.
 */

import { describe, it, expect } from "vitest";
import type { PerspectiveDef } from "./kanban";

describe("PerspectiveDef", () => {
  it("accepts a legacy literal without view_id", () => {
    // Legacy shape: only `id`, `name`, `view`. The view_id field is optional
    // (`view_id?: string`), so this literal must satisfy PerspectiveDef.
    const legacy: PerspectiveDef = {
      id: "01JPERSP00000000000LEGACY0",
      name: "Legacy",
      view: "grid",
    };

    expect(legacy.view_id).toBeUndefined();
    expect(legacy.view).toBe("grid");
  });

  it("accepts a scoped literal with view_id", () => {
    // New shape: view_id pins the perspective to a specific view instance.
    const scoped: PerspectiveDef = {
      id: "01JPERSP00000000000SCOPED0",
      name: "Scoped",
      view: "board",
      view_id: "01JMVIEW0000000000BOARD0",
    };

    expect(scoped.view_id).toBe("01JMVIEW0000000000BOARD0");
  });
});
