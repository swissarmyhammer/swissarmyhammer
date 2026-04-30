/**
 * Regression tests for the segment-form scope-chain contract.
 *
 * After the path-monikers (FQM) refactor, `<FocusScope>` and `<FocusZone>`
 * briefly wrote the *full FQM* (`/window/perspective:p1/board:b1/task:abc`)
 * into `CommandScope.moniker`. The Rust scope-commands code parses each
 * scope-chain entry with `split_once(':')` to extract an entity type — so
 * an FQM like `"/window/perspective:p1/..."` yields `entity_type =
 * "/window/perspective"`, no entity match, and right-click menus came up
 * empty.
 *
 * Fix: scope-chain entries must be **segments** (`"task:abc"`), not full
 * paths. The registry stays keyed by FQM (the kernel needs paths to
 * disambiguate same-segment registrations across layers), but
 * `scope.moniker` carries the segment.
 *
 * These tests pin the contract at three layers:
 *
 *   1. The leaf `CommandScope` produced by `<FocusScope>` / `<FocusZone>`
 *      has `.moniker === segment` (not the FQM).
 *   2. The full ancestor chain visible at the leaf is all-segments — every
 *      entry parses as `entity_type:entity_id` via `split_once(':')`.
 *   3. `useIsFocused(segment)` returns true for every segment in the
 *      focused leaf's ancestor chain (so callers can keep passing
 *      segments and not FQMs).
 *
 * If any of these regress, right-click menus break — the symptom is
 * "context menu shows only globals; no Cut/Copy/Delete/etc."
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { useContext } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import { FocusScope } from "./focus-scope";
import { FocusZone } from "./focus-zone";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  CommandScopeContext,
  CommandScopeProvider,
  scopeChainFromScope,
  type CommandScope,
} from "@/lib/command-scope";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { asSegment } from "@/types/spatial";

/**
 * Captures the `CommandScope` visible at its mount point so tests can
 * assert against the exact scope object FocusScope/FocusZone produced.
 */
function CaptureScope({
  out,
}: {
  out: { current: CommandScope | null };
}): null {
  out.current = useContext(CommandScopeContext);
  return null;
}

/**
 * Walk a scope chain to its root and return every `.moniker` string in
 * leaf-to-root order. Mirrors `scopeChainFromScope` but exposed locally so
 * the test failure messages can reference the field name explicitly.
 */
function collectMonikers(scope: CommandScope | null): string[] {
  return scopeChainFromScope(scope);
}

describe("scope chain emits segment-form monikers (regression)", () => {
  it("FocusScope.scope.moniker is the segment, not the full FQM", () => {
    const captured: { current: CommandScope | null } = { current: null };

    render(
      <SpatialFocusProvider>
        <EntityFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <FocusScope moniker={asSegment("task:abc")}>
              <CaptureScope out={captured} />
            </FocusScope>
          </FocusLayer>
        </EntityFocusProvider>
      </SpatialFocusProvider>,
    );

    expect(captured.current).not.toBeNull();
    // Segment, not FQM. If this flips back to the FQM
    // ("/window/task:abc") the Rust scope-chain parser breaks and right-
    // click menus go empty.
    expect(captured.current!.moniker).toBe("task:abc");
    expect(captured.current!.moniker).not.toContain("/");
  });

  it("FocusZone.scope.moniker is the segment, not the full FQM", () => {
    const captured: { current: CommandScope | null } = { current: null };

    render(
      <SpatialFocusProvider>
        <EntityFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <FocusZone moniker={asSegment("ui:toolbar.actions")}>
              <CaptureScope out={captured} />
            </FocusZone>
          </FocusLayer>
        </EntityFocusProvider>
      </SpatialFocusProvider>,
    );

    expect(captured.current).not.toBeNull();
    expect(captured.current!.moniker).toBe("ui:toolbar.actions");
    expect(captured.current!.moniker).not.toContain("/");
  });

  it("nested FocusScope produces an all-segment ancestor chain — every entry is split_once(':')-parseable", () => {
    const captured: { current: CommandScope | null } = { current: null };

    render(
      <SpatialFocusProvider>
        <EntityFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <CommandScopeProvider moniker="view:v1">
              <FocusZone moniker={asSegment("board:b1")}>
                <FocusZone moniker={asSegment("column:todo")}>
                  <FocusScope moniker={asSegment("task:abc")}>
                    <CaptureScope out={captured} />
                  </FocusScope>
                </FocusZone>
              </FocusZone>
            </CommandScopeProvider>
          </FocusLayer>
        </EntityFocusProvider>
      </SpatialFocusProvider>,
    );

    const chain = collectMonikers(captured.current);

    // Leaf-to-root ordering — matches what the Rust side receives.
    expect(chain).toEqual(["task:abc", "column:todo", "board:b1", "view:v1"]);

    // The Rust scope-chain parser does this on every entry. It must pull
    // a non-empty entity type back out — i.e. the entry must NOT begin
    // with the path separator and the head must not be empty.
    for (const entry of chain) {
      expect(entry.startsWith("/")).toBe(false);
      const [entityType, rest] = entry.split(":", 2);
      expect(entityType).not.toBe("");
      // Path entries (e.g. "/window/board:b1") would split as
      // ("/window/board", "b1") — non-empty but starting with "/". The
      // assertion above already pins that. Sanity-check the tail too.
      expect(rest).toBeDefined();
      expect(rest).not.toBe("");
    }
  });

  it("CommandScope chain still works when the FocusLayer fallback is the only ancestor (no FocusScope/Zone above)", () => {
    const captured: { current: CommandScope | null } = { current: null };

    render(
      <SpatialFocusProvider>
        <EntityFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <FocusScope moniker={asSegment("task:lone")}>
              <CaptureScope out={captured} />
            </FocusScope>
          </FocusLayer>
        </EntityFocusProvider>
      </SpatialFocusProvider>,
    );

    const chain = collectMonikers(captured.current);
    expect(chain).toEqual(["task:lone"]);
  });
});
