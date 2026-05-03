/**
 * Structural guard for the spatial-nav migration close-out
 * (kanban task `01KQJDKBQ2VNT3SE7AN3VM2KGZ`).
 *
 * The legacy `FocusActions.broadcastNavCommand` callback was the entry
 * point of the predicate-based pull navigation registry. The Rust
 * spatial-nav kernel replaced that pathway end-to-end (see
 * `01KQJDDPHB55Z4MF77YTYSAP0C` for the grid template), and the broadcast
 * function was reduced to a no-op stub that always returned `false` so
 * callers compiled while the migration progressed.
 *
 * This test asserts the field is gone from the actions bag — every
 * production caller has been migrated to either the global
 * `NAV_COMMAND_SPEC` (which dispatches `spatial_navigate`) or to a
 * direct `spatialActions.navigate(focusedFq, direction)` call. Once
 * deletion lands, the bag must not even carry the key, otherwise a new
 * caller could re-introduce the dead pathway by tab-completion.
 *
 * The earlier `describe("broadcastNavCommand", ...)` in
 * `entity-focus-context.test.tsx` is replaced by this guard — those
 * tests asserted the no-op semantics, which only made sense while the
 * stub still existed.
 */

import { describe, it, expect, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { type ReactNode } from "react";
import {
  EntityFocusProvider,
  useFocusActions,
} from "./entity-focus-context";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ label: "main" })),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

const wrapper = ({ children }: { children: ReactNode }) => (
  <EntityFocusProvider>{children}</EntityFocusProvider>
);

describe("FocusActions — broadcastNavCommand has been removed", () => {
  it("does not expose `broadcastNavCommand` on the actions bag", () => {
    const { result } = renderHook(() => useFocusActions(), { wrapper });
    // The actions bag must NOT carry the broadcast field. Re-introducing
    // it (for any reason) reopens the no-op pathway that silently broke
    // keyboard nav inside scopes that shadowed the global `nav.*` set.
    expect(Object.keys(result.current)).not.toContain("broadcastNavCommand");
    // Belt-and-suspenders: the runtime value must also be undefined,
    // not a stub. Tests that mock the module can keep
    // backwards-compat shape, but the production provider must not
    // ship the key at all.
    expect(
      (result.current as unknown as Record<string, unknown>)
        .broadcastNavCommand,
    ).toBeUndefined();
  });
});
