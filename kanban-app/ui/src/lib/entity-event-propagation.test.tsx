/**
 * Integration tests for entity event propagation.
 *
 * Verifies that Tauri `entity-created`, `entity-removed`, and
 * `entity-field-changed` events correctly update the entity store state.
 *
 * Strategy: render a minimal React component that:
 *   1. Registers entity event listeners via `listen` (same as RustEngineContainer)
 *   2. On `entity-created`, calls `invoke("get_entity")` to fetch fresh state
 *   3. On `entity-field-changed`, patches fields from the event payload (no round-trip)
 *   4. Updates a React state map keyed by entity type
 *   5. Exposes that state via `EntityStoreProvider` for inspection
 *
 * The `listen` mock captures event handler callbacks so tests can fire them
 * directly without a running Tauri backend.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useCallback, useEffect, useState, type ReactNode } from "react";

// ── Mocks must be declared before any imports that trigger module evaluation ──

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve({}));

vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

// Listen mock: stores each registered handler so tests can fire events.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const listenHandlers: Record<string, Array<(event: any) => void>> = {};
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockListen = vi.fn((eventName: string, handler: (event: any) => void) => {
  if (!listenHandlers[eventName]) {
    listenHandlers[eventName] = [];
  }
  listenHandlers[eventName].push(handler);
  // Return an unlisten function (simulates the real Tauri API)
  return Promise.resolve(() => {
    listenHandlers[eventName] = listenHandlers[eventName].filter(
      (h) => h !== handler,
    );
  });
});
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: Parameters<typeof mockListen>) => mockListen(...args),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { EntityStoreProvider, useEntityStore } from "./entity-store-context";
import type { Entity, EntityBag } from "@/types/kanban";
import { entityFromBag } from "@/types/kanban";

// ── Test helpers ──────────────────────────────────────────────────────────────

/**
 * Fire a simulated Tauri event to all registered handlers for `eventName`.
 * Wraps the call in `act()` so React state updates are flushed.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function fireEvent(eventName: string, payload: Record<string, any>) {
  const handlers = listenHandlers[eventName] ?? [];
  if (handlers.length === 0) {
    throw new Error(
      `No handlers registered for event "${eventName}". Did the component mount?`,
    );
  }
  await act(async () => {
    for (const handler of handlers) {
      handler({ payload });
    }
  });
}

/**
 * Minimal hook that replicates RustEngineContainer's entity event listener logic.
 *
 * - Registers listeners for `entity-created`, `entity-removed`, and
 *   `entity-field-changed` using `listen`.
 * - On `entity-created`, calls `invoke("get_entity")` to fetch fresh entity state.
 * - On `entity-field-changed`, patches fields from the event payload (no round-trip).
 * - On `entity-removed`, directly removes the entity from state.
 * - Returns the current entity map and a `setEntitiesFor` setter.
 *
 * This is a test-only hook; production code uses the same pattern in
 * RustEngineContainer. Keeping it here avoids rendering the full component tree.
 */
function useEntityEventListeners() {
  const [entitiesByType, setEntitiesByType] = useState<
    Record<string, Entity[]>
  >({});

  const setEntitiesFor = useCallback(
    (type: string, updater: (prev: Entity[]) => Entity[]) =>
      setEntitiesByType((prev) => ({
        ...prev,
        [type]: updater(prev[type] ?? []),
      })),
    [],
  );

  useEffect(() => {
    // The active board path is not relevant for unit tests — skip the
    // board_path filter by passing undefined (same as secondary windows).
    const activeBoardPath: string | undefined = undefined;

    const unlisteners = [
      listen<{
        entity_type: string;
        id: string;
        fields: Record<string, unknown>;
        board_path?: string;
      }>("entity-created", (event) => {
        const { entity_type, id, board_path } = event.payload;
        if (board_path && activeBoardPath && board_path !== activeBoardPath) {
          return;
        }
        invoke<EntityBag>("get_entity", {
          entityType: entity_type,
          id,
        })
          .then((bag) => {
            const entity = entityFromBag(bag);
            setEntitiesFor(entity_type, (prev) => {
              if (prev.some((e) => e.id === id)) {
                return prev.map((e) => (e.id === id ? entity : e));
              }
              return [...prev, entity];
            });
          })
          .catch(() => {
            // Ignore fetch errors in tests
          });
      }),

      listen<{
        entity_type: string;
        id: string;
        board_path?: string;
      }>("entity-removed", (event) => {
        const { entity_type, id, board_path } = event.payload;
        if (board_path && activeBoardPath && board_path !== activeBoardPath) {
          return;
        }
        setEntitiesFor(entity_type, (prev) => prev.filter((e) => e.id !== id));
      }),

      listen<{
        entity_type: string;
        id: string;
        changes: Array<{ field: string; value: unknown }>;
        fields?: Record<string, unknown>;
        board_path?: string;
      }>("entity-field-changed", (event) => {
        const { entity_type, id, changes, fields, board_path } = event.payload;
        if (board_path && activeBoardPath && board_path !== activeBoardPath) {
          return;
        }
        // Patch the entity in place from the event payload instead of
        // re-fetching via get_entity.
        setEntitiesFor(entity_type, (prev) =>
          prev.map((e) => {
            if (e.id !== id) return e;
            if (fields) {
              return { ...e, fields: { ...fields } };
            }
            if (changes && changes.length > 0) {
              const patched = { ...e.fields };
              for (const { field, value } of changes) {
                patched[field] = value;
              }
              return { ...e, fields: patched };
            }
            return e;
          }),
        );
      }),
    ];

    return () => {
      Promise.all(unlisteners).then((fns) => fns.forEach((fn) => fn()));
    };
  }, [setEntitiesFor]);

  return { entitiesByType, setEntitiesFor };
}

/**
 * Wrapper that provides `EntityStoreProvider` using state from
 * `useEntityEventListeners`. Exposes the entity store so tests can verify
 * state changes after firing events.
 */
function EntityEventWrapper({ children }: { children: ReactNode }) {
  const { entitiesByType } = useEntityEventListeners();
  return (
    <EntityStoreProvider entities={entitiesByType}>
      {children}
    </EntityStoreProvider>
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("entity event propagation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockClear();
    // Reset all captured handlers between tests
    for (const key of Object.keys(listenHandlers)) {
      delete listenHandlers[key];
    }
  });

  it("entity-created event: fetches entity and adds it to the store", async () => {
    // Set up invoke to return the new entity
    mockInvoke.mockImplementation(
      (cmd: string, args: Record<string, string>) => {
        if (cmd === "get_entity" && args.id === "task-001") {
          return Promise.resolve({
            entity_type: "task",
            id: "task-001",
            title: "My New Task",
          });
        }
        return Promise.resolve({});
      },
    );

    const { result } = renderHook(() => useEntityStore(), {
      wrapper: EntityEventWrapper,
    });

    // Wait for effects (listen registrations) to run
    await act(async () => {});

    // Verify the store starts empty
    expect(result.current.getEntities("task")).toHaveLength(0);

    // Fire the entity-created event
    await fireEvent("entity-created", {
      kind: "entity-created",
      entity_type: "task",
      id: "task-001",
      fields: { title: "My New Task" },
    });

    // Allow the invoke promise to resolve and state to update
    await act(async () => {});

    // Verify get_entity was called with the right args
    expect(mockInvoke).toHaveBeenCalledWith("get_entity", {
      entityType: "task",
      id: "task-001",
    });

    // Verify the entity appears in the store
    const tasks = result.current.getEntities("task");
    expect(tasks).toHaveLength(1);
    expect(tasks[0].id).toBe("task-001");
    expect(tasks[0].fields.title).toBe("My New Task");
  });

  it("entity-removed event: removes the entity from the store", async () => {
    // Pre-populate via entity-created so we have something to remove
    mockInvoke.mockImplementation(
      (cmd: string, args: Record<string, string>) => {
        if (cmd === "get_entity" && args.id === "tag-doomed") {
          return Promise.resolve({
            entity_type: "tag",
            id: "tag-doomed",
            tag_name: "doomed",
          });
        }
        return Promise.resolve({});
      },
    );

    const { result } = renderHook(() => useEntityStore(), {
      wrapper: EntityEventWrapper,
    });

    await act(async () => {});

    // Add the entity via entity-created first
    await fireEvent("entity-created", {
      kind: "entity-created",
      entity_type: "tag",
      id: "tag-doomed",
      fields: { tag_name: "doomed" },
    });
    await act(async () => {});

    expect(result.current.getEntities("tag")).toHaveLength(1);

    // Now fire the entity-removed event
    await fireEvent("entity-removed", {
      kind: "entity-removed",
      entity_type: "tag",
      id: "tag-doomed",
    });

    // entity-removed does not call invoke — just removes from store
    expect(result.current.getEntities("tag")).toHaveLength(0);
  });

  it("entity-field-changed event: patches entity fields from event payload", async () => {
    // Pre-populate via entity-created
    mockInvoke.mockImplementation(
      (cmd: string, args: Record<string, string>) => {
        if (cmd === "get_entity" && args.id === "tag-bug") {
          return Promise.resolve({
            entity_type: "tag",
            id: "tag-bug",
            tag_name: "bug",
            color: "ff0000",
          });
        }
        return Promise.resolve({});
      },
    );

    const { result } = renderHook(() => useEntityStore(), {
      wrapper: EntityEventWrapper,
    });

    await act(async () => {});

    // Seed the store with the initial entity
    await fireEvent("entity-created", {
      kind: "entity-created",
      entity_type: "tag",
      id: "tag-bug",
      fields: { tag_name: "bug", color: "ff0000" },
    });
    await act(async () => {});

    expect(result.current.getEntity("tag", "tag-bug")?.fields.color).toBe(
      "ff0000",
    );

    // Clear mock call history so we can verify no get_entity round-trip
    mockInvoke.mockClear();

    // Fire the entity-field-changed event with changes in the payload
    await fireEvent("entity-field-changed", {
      kind: "entity-field-changed",
      entity_type: "tag",
      id: "tag-bug",
      changes: [{ field: "color", value: "00ff00" }],
    });
    await act(async () => {});

    // Verify get_entity was NOT called — patching from event payload only
    const getEntityCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "get_entity",
    );
    expect(getEntityCalls).toHaveLength(0);

    // Verify the entity store has the updated field value
    const updated = result.current.getEntity("tag", "tag-bug");
    expect(updated?.fields.color).toBe("00ff00");
    // Verify other fields are preserved
    expect(updated?.fields.tag_name).toBe("bug");
  });

  it("entity-created event for existing id: replaces rather than duplicates", async () => {
    // First creation
    mockInvoke.mockImplementation(
      (cmd: string, args: Record<string, string>) => {
        if (cmd === "get_entity" && args.id === "actor-alice") {
          return Promise.resolve({
            entity_type: "actor",
            id: "actor-alice",
            name: "Alice",
          });
        }
        return Promise.resolve({});
      },
    );

    const { result } = renderHook(() => useEntityStore(), {
      wrapper: EntityEventWrapper,
    });

    await act(async () => {});

    await fireEvent("entity-created", {
      kind: "entity-created",
      entity_type: "actor",
      id: "actor-alice",
      fields: { name: "Alice" },
    });
    await act(async () => {});

    expect(result.current.getEntities("actor")).toHaveLength(1);

    // Fire entity-created again for the same id (e.g. file re-created after a sync)
    mockInvoke.mockImplementation(
      (cmd: string, args: Record<string, string>) => {
        if (cmd === "get_entity" && args.id === "actor-alice") {
          return Promise.resolve({
            entity_type: "actor",
            id: "actor-alice",
            name: "Alice Updated",
          });
        }
        return Promise.resolve({});
      },
    );

    await fireEvent("entity-created", {
      kind: "entity-created",
      entity_type: "actor",
      id: "actor-alice",
      fields: { name: "Alice Updated" },
    });
    await act(async () => {});

    // Should still be only one entry, not two
    expect(result.current.getEntities("actor")).toHaveLength(1);
    expect(result.current.getEntity("actor", "actor-alice")?.fields.name).toBe(
      "Alice Updated",
    );
  });

  it("verify listen is called for all three event names", async () => {
    // This ensures the useEffect runs and registers all listeners
    renderHook(() => useEntityStore(), { wrapper: EntityEventWrapper });
    await act(async () => {});

    expect(mockListen).toHaveBeenCalledWith(
      "entity-created",
      expect.any(Function),
    );
    expect(mockListen).toHaveBeenCalledWith(
      "entity-removed",
      expect.any(Function),
    );
    expect(mockListen).toHaveBeenCalledWith(
      "entity-field-changed",
      expect.any(Function),
    );
  });
});
