/**
 * Headline contract: a command's N `store/changed` notifications that share
 * one `txn` apply as ONE atomic re-render; a notification with a new `txn`
 * flushes separately.
 *
 * This pins the transaction-batching seam (`subscribeStoreChanged` +
 * `applyStoreChangeBatch` in `rust-engine-container.tsx`): a multi-write
 * command (or an undo of one) must re-render the entity store once, not N
 * times. The probe counts how many times a `useEntitiesByType()` consumer
 * commits, so it observes the production state-update granularity directly.
 *
 * The same test also proves the existing field-patch reducer is reused: the
 * three changes land on the entity exactly as the per-field path produced
 * them.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";
import { useRef } from "react";

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const mockInvoke = vi.fn((..._args: any[]): Promise<any> => {
    const cmd = _args[0] as string;
    if (cmd === "get_ui_state")
      return Promise.resolve({
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      });
    if (cmd === "list_schemas") return Promise.resolve([]);
    return Promise.resolve(null);
  });
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: mockInvoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mockListen }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

import { RustEngineContainer, useEntitiesByType } from "./rust-engine-container";
import type { Entity } from "@/types/kanban";

const STORE_CHANGED = "notifications/store/changed";

interface StoreChangeNote {
  store: string;
  item: string;
  op?: "created" | "removed" | "updated";
  changes?: Array<{ field: string; value: unknown }>;
  txn?: string | null;
  origin?: string;
}

function emit(note: StoreChangeNote) {
  const cbs = listeners.get(STORE_CHANGED) ?? [];
  for (const cb of cbs) {
    cb({
      payload: { op: "updated", origin: "user", ...note },
    });
  }
}

async function waitForSubscription() {
  await waitFor(() => {
    expect((listeners.get(STORE_CHANGED)?.length ?? 0) > 0).toBe(true);
  });
}

/**
 * Probe that counts how many times the entity store committed a new value,
 * and surfaces the task's current `title` so we can assert the patches landed.
 */
function RenderCountProbe() {
  const entitiesByType = useEntitiesByType();
  const renders = useRef(0);
  const lastSeen = useRef<Record<string, Entity[]> | null>(null);
  // Count only commits where the entity map identity actually changed — i.e.
  // a real store update, not a re-render from an unrelated parent.
  if (lastSeen.current !== entitiesByType) {
    lastSeen.current = entitiesByType;
    renders.current += 1;
  }
  const task = (entitiesByType.task ?? [])[0];
  const title = task ? String(task.fields.title ?? "") : "none";
  const status = task ? String(task.fields.status ?? "") : "none";
  return (
    <>
      <span data-testid="store-commits">{renders.current}</span>
      <span data-testid="task-title">{title}</span>
      <span data-testid="task-status">{status}</span>
    </>
  );
}

describe("transaction batching", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
  });

  it("three store/changed sharing one txn produce exactly one store commit", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <RenderCountProbe />
        </RustEngineContainer>,
      );
    });
    await waitForSubscription();

    // Seed one task in its own (separate) transaction so the store has an
    // initial commit, then read the baseline commit count.
    await act(async () => {
      emit({
        store: "task",
        item: "t1",
        op: "created",
        changes: [{ field: "title", value: "Initial" }],
        txn: "seed",
      });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe("Initial");
    });
    const commitsAfterSeed = Number(
      screen.getByTestId("store-commits").textContent,
    );

    // Fire THREE notifications for one command, all sharing `txn: "cmd-1"`.
    // They arrive synchronously (one tick) and must coalesce into a single
    // store commit when the txn batcher's microtask flushes.
    await act(async () => {
      emit({
        store: "task",
        item: "t1",
        op: "updated",
        changes: [{ field: "title", value: "Renamed" }],
        txn: "cmd-1",
      });
      emit({
        store: "task",
        item: "t1",
        op: "updated",
        changes: [{ field: "status", value: "doing" }],
        txn: "cmd-1",
      });
      emit({
        store: "task",
        item: "t1",
        op: "updated",
        changes: [{ field: "title", value: "Renamed Again" }],
        txn: "cmd-1",
      });
      await Promise.resolve();
    });

    // All three patches landed (reducer reused unchanged)...
    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe(
        "Renamed Again",
      );
    });
    expect(screen.getByTestId("task-status").textContent).toBe("doing");

    // ...but as EXACTLY ONE additional store commit.
    const commitsAfterTxn = Number(
      screen.getByTestId("store-commits").textContent,
    );
    expect(commitsAfterTxn - commitsAfterSeed).toBe(1);
  });

  it("a notification with a new txn flushes as a separate commit", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <RenderCountProbe />
        </RustEngineContainer>,
      );
    });
    await waitForSubscription();

    // Seed.
    await act(async () => {
      emit({
        store: "task",
        item: "t1",
        op: "created",
        changes: [{ field: "title", value: "Initial" }],
        txn: "seed",
      });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe("Initial");
    });
    const baseline = Number(screen.getByTestId("store-commits").textContent);

    // First transaction: three changes → one commit.
    await act(async () => {
      emit({ store: "task", item: "t1", changes: [{ field: "title", value: "A" }], txn: "cmd-1" });
      emit({ store: "task", item: "t1", changes: [{ field: "title", value: "B" }], txn: "cmd-1" });
      emit({ store: "task", item: "t1", changes: [{ field: "title", value: "C" }], txn: "cmd-1" });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe("C");
    });

    // Second transaction (a 4th notification with a NEW txn) → one more commit.
    await act(async () => {
      emit({ store: "task", item: "t1", changes: [{ field: "title", value: "D" }], txn: "cmd-2" });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getByTestId("task-title").textContent).toBe("D");
    });

    const total = Number(screen.getByTestId("store-commits").textContent);
    // Exactly two commits beyond the baseline: one per transaction.
    expect(total - baseline).toBe(2);
  });
});
