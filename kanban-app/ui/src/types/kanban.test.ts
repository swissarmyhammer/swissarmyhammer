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
import type { CommandDef, ParamDef, PerspectiveDef } from "./kanban";

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

describe("CommandDef (YAML schema mirror)", () => {
  it("accepts a minimal literal omitting tab_button and params", () => {
    // Mirrors the Rust round-trip: CommandDef with no new fields. All of
    // tab_button, params[].shape, options_from, options are optional, so a
    // literal that only carries id/name/scope must satisfy the interface.
    const minimal: CommandDef = {
      id: "app.quit",
      name: "Quit",
      scope: undefined,
    };

    expect(minimal.tab_button).toBeUndefined();
    expect(minimal.params).toBeUndefined();
  });

  it("accepts a literal carrying tab_button and an enum param", () => {
    // Mirrors the Rust round-trip: CommandDef with tab_button + an enum
    // param that names a backend resolver. This is the shape consumed by
    // <CommandButton>; if a future change breaks the type it must fail
    // here before the dependent tasks (resolver / button) can compile.
    const filter: CommandDef = {
      id: "perspective.filter.set",
      name: "Filter",
      scope: "entity:perspective",
      tab_button: { icon: "filter" },
      params: [
        {
          name: "field",
          shape: "enum",
          options_from: "perspective.fields",
        },
      ],
    };

    expect(filter.tab_button?.icon).toBe("filter");
    expect(filter.params?.[0].shape).toBe("enum");
    expect(filter.params?.[0].options_from).toBe("perspective.fields");
  });

  it("accepts inline param options", () => {
    // Static option lists known at YAML write time live inline on the
    // ParamDef rather than going through a resolver.
    const sort: ParamDef = {
      name: "direction",
      shape: "enum",
      options: [
        { value: "asc", label: "Ascending" },
        { value: "desc", label: "Descending" },
      ],
    };

    expect(sort.options?.length).toBe(2);
    expect(sort.options?.[0].value).toBe("asc");
  });
});
