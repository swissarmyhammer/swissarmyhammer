/**
 * Tests for the shared spatial-kernel mock harness
 * (`mock-spatial-kernel.ts`).
 *
 * The harness was copy-pasted near-verbatim across ~13 spatial browser
 * tests (card `01KV6250AH0DPRMG9SJ6A45SPW`). These tests pin the single
 * source of truth's contract so the kernel-echo behavior stays identical
 * to every inline copy it replaces:
 *
 *   - `spatial_register_scope` / `spatial_unregister_scope` maintain the
 *     `monikerToKey` (segment → fq) projection.
 *   - `spatial_focus` advances `currentFocusKey` and emits a queued
 *     `focus-changed` event resolving the moniker from the projection.
 *   - `spatial_clear_focus` is the idempotent `Some(prev) → None` inverse.
 *   - `spatial_drill_in` / `spatial_drill_out` echo the focused moniker
 *     under the no-silent-dropout contract, with an optional per-test
 *     `drillInResponses` override.
 *   - Non-spatial commands return the {@link UNHANDLED} sentinel so the
 *     caller can fall through to its own entity / UI-state answers.
 */
import { describe, it, expect, vi } from "vitest";
import {
  UNHANDLED,
  makeSpatialKernelMock,
} from "@/test/mock-spatial-kernel";

/** Drain the microtask queue so queued `focus-changed` emits flush. */
async function flushMicrotasks() {
  await Promise.resolve();
  await Promise.resolve();
}

describe("makeSpatialKernelMock", () => {
  it("returns UNHANDLED for non-spatial commands", () => {
    const emit = vi.fn();
    const { handleSpatialCommand } = makeSpatialKernelMock({ emit });

    expect(handleSpatialCommand("get_ui_state")).toBe(UNHANDLED);
    expect(handleSpatialCommand("list_entity_types")).toBe(UNHANDLED);
    expect(handleSpatialCommand("dispatch_command", {})).toBe(UNHANDLED);
  });

  it("records the segment → fq projection on register and removes it on unregister", () => {
    const emit = vi.fn();
    const { handleSpatialCommand, monikerToKey } = makeSpatialKernelMock({
      emit,
    });

    handleSpatialCommand("spatial_register_scope", {
      fq: "window/card:T1",
      segment: "card:T1",
    });
    expect(monikerToKey.get("card:T1")).toBe("window/card:T1");

    handleSpatialCommand("spatial_unregister_scope", {
      fq: "window/card:T1",
    });
    expect(monikerToKey.has("card:T1")).toBe(false);
  });

  it("advances currentFocusKey and emits a queued focus-changed on spatial_focus", async () => {
    const emit = vi.fn();
    const { handleSpatialCommand, currentFocusKey } = makeSpatialKernelMock({
      emit,
    });

    handleSpatialCommand("spatial_register_scope", {
      fq: "window/card:T1",
      segment: "card:T1",
    });
    expect(handleSpatialCommand("spatial_focus", { fq: "window/card:T1" })).toBe(
      undefined,
    );

    // Synchronously the focus key advances...
    expect(currentFocusKey.key).toBe("window/card:T1");
    // ...but the emit is queued, not synchronous.
    expect(emit).not.toHaveBeenCalled();

    await flushMicrotasks();

    expect(emit).toHaveBeenCalledTimes(1);
    expect(emit).toHaveBeenCalledWith({
      payload: {
        window_label: "main",
        prev_fq: null,
        next_fq: "window/card:T1",
        next_segment: "card:T1",
      },
    });
  });

  it("emits the Some(prev) → None inverse on spatial_clear_focus, idempotent when unfocused", async () => {
    const emit = vi.fn();
    const { handleSpatialCommand, currentFocusKey } = makeSpatialKernelMock({
      emit,
    });

    // No prior focus → clear is a no-op (no emit).
    expect(handleSpatialCommand("spatial_clear_focus", {})).toBe(undefined);
    await flushMicrotasks();
    expect(emit).not.toHaveBeenCalled();

    handleSpatialCommand("spatial_register_scope", {
      fq: "window/card:T1",
      segment: "card:T1",
    });
    handleSpatialCommand("spatial_focus", { fq: "window/card:T1" });
    await flushMicrotasks();
    emit.mockClear();

    handleSpatialCommand("spatial_clear_focus", {});
    expect(currentFocusKey.key).toBe(null);
    await flushMicrotasks();

    expect(emit).toHaveBeenCalledTimes(1);
    expect(emit).toHaveBeenCalledWith({
      payload: {
        window_label: "main",
        prev_fq: "window/card:T1",
        next_fq: null,
        next_segment: null,
      },
    });
  });

  it("echoes the focused moniker on drill under the no-silent-dropout contract", () => {
    const emit = vi.fn();
    const { handleSpatialCommand } = makeSpatialKernelMock({ emit });

    expect(
      handleSpatialCommand("spatial_drill_in", { focusedFq: "window/card:T1" }),
    ).toBe("window/card:T1");
    expect(
      handleSpatialCommand("spatial_drill_out", { focusedFq: "window/card:T1" }),
    ).toBe("window/card:T1");
    // Missing focusedFq falls back to null (a leaf with nothing to echo).
    expect(handleSpatialCommand("spatial_drill_in", {})).toBe(null);
  });

  it("honors a per-test drillInResponses override for spatial_drill_in", () => {
    const emit = vi.fn();
    const { handleSpatialCommand, drillInResponses } = makeSpatialKernelMock({
      emit,
    });

    // Non-null entry → drill walked to a child; return it verbatim.
    drillInResponses.set("window/field:tags", "tag:bug");
    expect(
      handleSpatialCommand("spatial_drill_in", {
        fq: "window/field:tags",
        focusedFq: "window/field:tags",
      }),
    ).toBe("tag:bug");

    // Null entry → stay put; echo the focused moniker.
    drillInResponses.set("window/field:name", null);
    expect(
      handleSpatialCommand("spatial_drill_in", {
        fq: "window/field:name",
        focusedFq: "window/field:name",
      }),
    ).toBe("window/field:name");
  });

  it("resets the projection and focus state on reset()", async () => {
    const emit = vi.fn();
    const { handleSpatialCommand, monikerToKey, currentFocusKey, reset } =
      makeSpatialKernelMock({ emit });

    handleSpatialCommand("spatial_register_scope", {
      fq: "window/card:T1",
      segment: "card:T1",
    });
    handleSpatialCommand("spatial_focus", { fq: "window/card:T1" });
    await flushMicrotasks();

    reset();
    expect(monikerToKey.size).toBe(0);
    expect(currentFocusKey.key).toBe(null);
  });
});
