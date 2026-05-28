/**
 * RustEngineContainer consolidates all Rust backend bridge providers and
 * entity state management into a single container component.
 *
 * Owns:
 * - CommandScopeProvider with moniker "engine"
 * - SchemaProvider, EntityStoreProvider, EntityFocusProvider, FieldUpdateProvider,
 *   UIStateProvider, UndoProvider
 * - entitiesByType state (managed internally)
 * - All entity Tauri event listeners (entity-created, entity-removed, entity-field-changed)
 * - refreshEntities(boardPath) exposed via context for parent to call on board switch
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  subscribeStoreChanged,
  type StoreChanged,
  type StoreChangeBatch,
} from "@/lib/mcp-notifications";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  CommandScopeProvider,
  useSetCommandInflight,
} from "@/lib/command-scope";
import type { Entity } from "@/types/kanban";
import { refreshBoards, type RefreshResult } from "@/lib/refresh";

// ---------------------------------------------------------------------------
// RefreshEntities context — lets the parent trigger entity refresh
// ---------------------------------------------------------------------------

type RefreshEntitiesFn = (
  boardPath: string,
  taskFilter?: string,
) => Promise<RefreshResult>;

const RefreshEntitiesContext = createContext<RefreshEntitiesFn>(async () => ({
  openBoards: [],
  boardData: null,
  entitiesByType: null,
}));

/**
 * Returns a function that refreshes all entities for a given board path.
 * The container updates entitiesByType internally; the returned RefreshResult
 * also includes openBoards and boardData so the caller can update those.
 */
export function useRefreshEntities(): RefreshEntitiesFn {
  return useContext(RefreshEntitiesContext);
}

// ---------------------------------------------------------------------------
// EntitiesByType reactive context — consumers re-render when entities change
// ---------------------------------------------------------------------------

const EntitiesByTypeContext = createContext<Record<string, Entity[]>>({});

/**
 * Returns the current entitiesByType map. Components that call this hook
 * re-render whenever the map changes (new entity added, removed, or updated).
 */
export function useEntitiesByType(): Record<string, Entity[]> {
  return useContext(EntitiesByTypeContext);
}

// ---------------------------------------------------------------------------
// SetEntitiesByType context — lets the parent directly set entities
// ---------------------------------------------------------------------------

type SetEntitiesByTypeFn = React.Dispatch<
  React.SetStateAction<Record<string, Entity[]>>
>;

const SetEntitiesByTypeContext = createContext<SetEntitiesByTypeFn>(() => {});

/**
 * Returns the raw setEntitiesByType setter. Used by the parent when it needs
 * to clear or replace entities (e.g. on board close or board-opened event).
 */
export function useSetEntitiesByType(): SetEntitiesByTypeFn {
  return useContext(SetEntitiesByTypeContext);
}

// ---------------------------------------------------------------------------
// ActiveBoardPath context for entity events — internal to the container
// ---------------------------------------------------------------------------

type SetActiveBoardPathFn = (path: string | undefined) => void;

const EngineActiveBoardPathContext = createContext<SetActiveBoardPathFn>(
  () => {},
);

/**
 * Returns a setter to update the engine's active board path reference.
 * Used by the parent when switching boards so entity events filter correctly.
 */
export function useEngineSetActiveBoardPath(): SetActiveBoardPathFn {
  return useContext(EngineActiveBoardPathContext);
}

// ---------------------------------------------------------------------------
// RustEngineContainer
// ---------------------------------------------------------------------------

interface RustEngineContainerProps {
  children: ReactNode;
}

/**
 * Refresh callback with two cross-cutting concerns layered on top of
 * `refreshBoards`:
 *
 * 1. **Busy tracking** — increments the shared `CommandBusy` counter for
 *    the duration of the call, so the nav-bar progress bar lights up
 *    while a refresh is in flight (matching `useDispatchCommand`).
 * 2. **Latest-wins guard** — a monotonic id captured per call; if the id
 *    has advanced by the time `refreshBoards` settles, the result is
 *    returned to the caller (so `openBoards` / `boardData` consumers still
 *    get a value) but the entity store is NOT overwritten with stale tasks.
 *    Collapses bursts of refetches into a single store write without
 *    needing AbortController plumbing across the Tauri IPC boundary.
 */
function useGuardedRefreshEntities(
  activeBoardPathRef: React.MutableRefObject<string | undefined>,
  setEntitiesByType: SetEntitiesByTypeFn,
): RefreshEntitiesFn {
  const refetchIdRef = useRef(0);
  const setInflightCount = useSetCommandInflight();
  return useCallback(
    async (boardPath: string, taskFilter?: string): Promise<RefreshResult> => {
      activeBoardPathRef.current = boardPath;
      const myId = ++refetchIdRef.current;
      setInflightCount((c) => c + 1);
      try {
        const result = await refreshBoards(boardPath, taskFilter);
        // Stale-response branch — see "Latest-wins guard" in the block comment.
        if (myId !== refetchIdRef.current) return result;
        if (result.entitiesByType) setEntitiesByType(result.entitiesByType);
        return result;
      } finally {
        setInflightCount((c) => c - 1);
      }
    },
    [activeBoardPathRef, setEntitiesByType, setInflightCount],
  );
}

/**
 * Container that owns entity state and all Rust backend bridge providers.
 *
 * Wraps children with CommandScopeProvider (moniker="engine"), SchemaProvider,
 * EntityStoreProvider, EntityFocusProvider, FieldUpdateProvider, UIStateProvider,
 * and UndoProvider.
 *
 * Manages entitiesByType state internally and listens for entity-created,
 * entity-removed, and entity-field-changed Tauri events to keep the store
 * up to date.
 */
export function RustEngineContainer({ children }: RustEngineContainerProps) {
  const [entitiesByType, setEntitiesByType] = useState<
    Record<string, Entity[]>
  >({});

  /** Ref tracking the active board path for event filtering. */
  const activeBoardPathRef = useRef<string | undefined>(undefined);

  const refreshEntities = useGuardedRefreshEntities(
    activeBoardPathRef,
    setEntitiesByType,
  );

  const setActiveBoardPath = useCallback((path: string | undefined) => {
    activeBoardPathRef.current = path;
  }, []);

  useMcpStoreSubscription(
    activeBoardPathRef,
    refreshEntities,
    setEntitiesByType,
  );

  return (
    <EngineProviderStack
      refreshEntities={refreshEntities}
      setEntitiesByType={setEntitiesByType}
      setActiveBoardPath={setActiveBoardPath}
      entitiesByType={entitiesByType}
    >
      {children}
    </EngineProviderStack>
  );
}

// ---------------------------------------------------------------------------
// MCP store-change reducer — entity store kept live by the MCP notification
// stream (`notifications/store/changed`), not Tauri `entity-*` events.
//
// Input source migration (this task): the webview is a pure MCP client. The
// change stream is the same one external agents receive. The reducer below is
// the SAME field-patch / removal / column-refresh logic that the Tauri
// `entity-created` / `entity-removed` / `entity-field-changed` handlers
// applied — only the input source changed from those three Tauri events to
// the one generic `store/changed` notification:
//
//   op:"updated"|"created" with changes → patch individual fields in place,
//                                          upserting if the item is absent
//   op:"removed"                        → remove from store by id
//   structural type (column)            → trigger a full refresh
//
// DO NOT add full-state replacement or get_entity re-fetch to field patches.
//
// Transaction batching: a command's N `store/changed` notifications share one
// `txn` and arrive as ONE batch (see `subscribeStoreChanged`). The whole batch
// is applied in a single `setEntitiesByType` so a multi-write command (or an
// undo of one) re-renders exactly once, not N times.
//
// Architecture contract — event-driven grid:
// Grid navigation (arrow keys, cell clicks, focus changes that don't touch
// entity data) must NEVER trigger a backend data-fetch. The grid body
// stays in sync exclusively through this reducer: field cells subscribe via
// `useFieldValue` (see `entity-store-context.tsx`) and redraw from the store
// when this code patches an entity in place. On navigation only `ui.setFocus`
// is dispatched — no `list_entities`, `get_entity`, `get_board_data`, or
// `perspective.list`. The regression test `grid-view.nav-is-eventdriven.test.tsx`
// enforces this invariant.
// ---------------------------------------------------------------------------

/** Stores whose changes are reload-item signals, not field patches. */
const RELOAD_ITEM_STORES: ReadonlySet<string> = new Set(["view", "perspective"]);

/** Structural entity types whose changes require a full board refresh. */
const STRUCTURAL_TYPES: ReadonlySet<string> = new Set(["column"]);

/**
 * Apply one `store/changed` notification to a single entity-type list.
 *
 * This is the unchanged per-entity reducer logic, lifted out of the former
 * `handleEntityFieldChanged` / `handleEntityRemoved` Tauri handlers so a whole
 * batch can be folded in one `setEntitiesByType`.
 *
 * - `op:"removed"` → drop the item by id.
 * - `op:"updated"|"created"` with `changes` → patch fields in place, upserting
 *   from the changes array when the item is absent (race with create).
 */
function applyStoreChangeToList(prev: Entity[], note: StoreChanged): Entity[] {
  const { store: entity_type, item: id, op, changes } = note;

  if (op === "removed") {
    return prev.filter((e) => e.id !== id);
  }

  if (!changes || changes.length === 0) return prev;

  let found = false;
  const next = prev.map((e) => {
    if (e.id !== id) return e;
    found = true;
    const patched = { ...e.fields };
    for (const { field, value } of changes) patched[field] = value;
    return { ...e, fields: patched };
  });
  if (!found) {
    const fields: Record<string, unknown> = {};
    for (const { field, value } of changes) fields[field] = value;
    next.push({
      entity_type,
      id,
      moniker: `${entity_type}:${id}`,
      fields,
    });
  }
  return next;
}

/**
 * Fold a batch of same-`txn` `store/changed` notifications into the entity map.
 *
 * Returns the next `entitiesByType` map (referentially new only for the types
 * a notification touched) plus the set of structural types that need a full
 * refresh (columns), which the caller dispatches outside the state update.
 * Views/perspectives are reload-item stores owned by their own contexts and
 * are skipped here.
 */
export function applyStoreChangeBatch(
  prev: Record<string, Entity[]>,
  batch: StoreChangeBatch,
): { next: Record<string, Entity[]>; refreshNeeded: boolean } {
  let next = prev;
  let mutated = false;
  let refreshNeeded = false;

  for (const note of batch) {
    if (RELOAD_ITEM_STORES.has(note.store)) continue; // owned by views/perspective ctx
    if (STRUCTURAL_TYPES.has(note.store)) {
      refreshNeeded = true;
      continue;
    }
    const type = note.store;
    const before = next[type] ?? [];
    const after = applyStoreChangeToList(before, note);
    if (after !== before) {
      if (!mutated) {
        next = { ...next };
        mutated = true;
      }
      next[type] = after;
    }
  }

  return { next, refreshNeeded };
}

/**
 * Subscribe the entity store to the MCP `store/changed` plane.
 *
 * Replaces the former Tauri `entity-created` / `entity-removed` /
 * `entity-field-changed` listeners. Each transaction's notifications arrive as
 * one batch and are applied in a single state update (see
 * `applyStoreChangeBatch`). Structural (column) changes trigger a guarded
 * board refresh, matching the prior `handleEntityRemoved`/`handleEntityCreated`
 * column branch.
 */
function useMcpStoreSubscription(
  activeBoardPathRef: React.RefObject<string | undefined>,
  refreshEntities: RefreshEntitiesFn,
  setEntitiesByType: SetEntitiesByTypeFn,
): void {
  useEffect(() => {
    let disposed = false;

    const onBatch = (batch: StoreChangeBatch) => {
      setEntitiesByType((prev) => {
        const { next, refreshNeeded } = applyStoreChangeBatch(prev, batch);
        if (refreshNeeded && activeBoardPathRef.current) {
          // Defer the refresh so it does not run inside the state updater.
          const path = activeBoardPathRef.current;
          queueMicrotask(() => {
            if (!disposed) refreshEntities(path);
          });
        }
        return next;
      });
    };

    const unsubPromise = subscribeStoreChanged(onBatch);

    return () => {
      disposed = true;
      unsubPromise.then((unsub) => unsub());
    };
  }, [activeBoardPathRef, refreshEntities, setEntitiesByType]);
}

// ---------------------------------------------------------------------------
// Provider stack — extracted from RustEngineContainer JSX
// ---------------------------------------------------------------------------

/** Props for the extracted provider stack. */
interface EngineProviderStackProps {
  refreshEntities: RefreshEntitiesFn;
  setEntitiesByType: SetEntitiesByTypeFn;
  setActiveBoardPath: SetActiveBoardPathFn;
  entitiesByType: Record<string, Entity[]>;
  children: ReactNode;
}

/**
 * Nested provider stack for the Rust engine container.
 *
 * Wraps children with all engine-level contexts: command scope, schema,
 * entity store, entity focus, field update, UI state, and undo providers.
 */
function EngineProviderStack({
  refreshEntities,
  setEntitiesByType,
  setActiveBoardPath,
  entitiesByType,
  children,
}: EngineProviderStackProps) {
  return (
    <CommandScopeProvider moniker="engine">
      <RefreshEntitiesContext.Provider value={refreshEntities}>
        <SetEntitiesByTypeContext.Provider value={setEntitiesByType}>
          <EngineActiveBoardPathContext.Provider value={setActiveBoardPath}>
            <SchemaProvider>
              <EntityStoreProvider entities={entitiesByType}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <UndoProvider>
                        <EntitiesByTypeContext.Provider value={entitiesByType}>
                          {children}
                        </EntitiesByTypeContext.Provider>
                      </UndoProvider>
                    </UIStateProvider>
                  </FieldUpdateProvider>
                </EntityFocusProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </EngineActiveBoardPathContext.Provider>
        </SetEntitiesByTypeContext.Provider>
      </RefreshEntitiesContext.Provider>
    </CommandScopeProvider>
  );
}
