/**
 * Parity + removal contract for the MCP `store/changed` plane.
 *
 * - A `store/changed` `{field,value}` patch lands on the board identically to
 *   the old Tauri `entity-field-changed` path — the reducer
 *   (`applyStoreChangeBatch` in `rust-engine-container.tsx`) is the SAME
 *   field-patch logic, only the input source changed.
 * - An `op: "removed"` notification removes the card from the store.
 *
 * The test drives the production `RustEngineContainer` so it exercises the
 * real subscription + reducer, not a test-only replica.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";

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

import {
  RustEngineContainer,
  useEntitiesByType,
} from "./rust-engine-container";
import { useFieldValue } from "@/lib/entity-store-context";

const STORE_CHANGED = "notifications/store/changed";

interface StoreChangeNote {
  store: string;
  item: string;
  op?: "created" | "removed" | "updated";
  changes?: Array<{ field: string; value: unknown }>;
  txn?: string | null;
  origin?: string;
}

async function fireStore(note: StoreChangeNote) {
  await act(async () => {
    const cbs = listeners.get(STORE_CHANGED) ?? [];
    for (const cb of cbs) {
      cb({ payload: { op: "updated", txn: null, origin: "user", ...note } });
    }
    await Promise.resolve();
  });
}

async function waitForSubscription() {
  await waitFor(() => {
    expect((listeners.get(STORE_CHANGED)?.length ?? 0) > 0).toBe(true);
  });
}

/** Surfaces a card's title via the production `useFieldValue` hook + count. */
function CardProbe() {
  const entitiesByType = useEntitiesByType();
  const title = useFieldValue("task", "t1", "title");
  const count = (entitiesByType.task ?? []).length;
  return (
    <>
      <span data-testid="card-title">
        {title === undefined ? "missing" : String(title)}
      </span>
      <span data-testid="task-count">{count}</span>
    </>
  );
}

describe("MCP store/changed notifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
  });

  it("a {field,value} patch updates the board card identically to the old path", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <CardProbe />
        </RustEngineContainer>,
      );
    });
    await waitForSubscription();

    // Seed a task card.
    await fireStore({
      store: "task",
      item: "t1",
      op: "created",
      changes: [{ field: "title", value: "Original" }],
    });
    await waitFor(() => {
      expect(screen.getByTestId("card-title").textContent).toBe("Original");
    });

    const getEntityBefore = mockInvoke.mock.calls.filter(
      (c) => c[0] === "get_entity",
    ).length;

    // Patch the title via a {field,value} change — the reducer patches in
    // place exactly as the former entity-field-changed handler did.
    await fireStore({
      store: "task",
      item: "t1",
      op: "updated",
      changes: [{ field: "title", value: "Patched" }],
    });

    await waitFor(() => {
      expect(screen.getByTestId("card-title").textContent).toBe("Patched");
    });

    // No get_entity refetch — purely a thin patch from the notification.
    const getEntityAfter = mockInvoke.mock.calls.filter(
      (c) => c[0] === "get_entity",
    ).length;
    expect(getEntityAfter).toBe(getEntityBefore);
  });

  it('op:"removed" removes the card from the store', async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <CardProbe />
        </RustEngineContainer>,
      );
    });
    await waitForSubscription();

    await fireStore({
      store: "task",
      item: "t1",
      op: "created",
      changes: [{ field: "title", value: "Doomed" }],
    });
    await waitFor(() => {
      expect(screen.getByTestId("task-count").textContent).toBe("1");
    });

    await fireStore({ store: "task", item: "t1", op: "removed" });

    await waitFor(() => {
      expect(screen.getByTestId("task-count").textContent).toBe("0");
    });
  });
});
