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
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { UndoProvider } from "@/lib/undo-context";
import { CommandScopeProvider } from "@/lib/command-scope";
import type { Entity, EntityBag } from "@/types/kanban";
import { entityFromBag } from "@/types/kanban";
import { refreshBoards, type RefreshResult } from "@/lib/refresh";

// ---------------------------------------------------------------------------
// Event payload types
// ---------------------------------------------------------------------------

/** Payload for entity-created Tauri event. */
interface EntityCreatedEvent {
  kind: "entity-created";
  entity_type: string;
  id: string;
  fields: Record<string, unknown>;
  board_path?: string;
}

/** Payload for entity-removed Tauri event. */
interface EntityRemovedEvent {
  kind: "entity-removed";
  entity_type: string;
  id: string;
  board_path?: string;
}

/**
 * Payload for entity-field-changed Tauri event.
 *
 * Architecture rule (event-architecture): events are thin signals.
 * Each change carries ONE field name and its new value. The frontend
 * patches individual fields in place — no full-state replacement,
 * no get_entity re-fetch. DO NOT add a `fields` map here.
 */
interface EntityFieldChangedEvent {
  kind: "entity-field-changed";
  entity_type: string;
  id: string;
  changes: Array<{ field: string; value: unknown }>;
  board_path?: string;
}

// ---------------------------------------------------------------------------
// RefreshEntities context — lets the parent trigger entity refresh
// ---------------------------------------------------------------------------

type RefreshEntitiesFn = (boardPath: string, taskFilter?: string) => Promise<RefreshResult>;

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

  /** Helper to update entities for a single type. */
  const setEntitiesFor = useCallback(
    (type: string, updater: (prev: Entity[]) => Entity[]) =>
      setEntitiesByType((prev) => ({
        ...prev,
        [type]: updater(prev[type] ?? []),
      })),
    [],
  );

  /**
   * Refresh all entities from the backend for the given board path.
   * Updates entitiesByType internally and returns the full RefreshResult
   * so the caller can also use openBoards and boardData.
   */
  const refreshEntities = useCallback(
    async (boardPath: string, taskFilter?: string): Promise<RefreshResult> => {
      activeBoardPathRef.current = boardPath;
      const result = await refreshBoards(boardPath, taskFilter);
      if (result.entitiesByType) {
        setEntitiesByType(result.entitiesByType);
      }
      return result;
    },
    [],
  );

  /** Set the active board path for event filtering without fetching. */
  const setActiveBoardPath = useCallback((path: string | undefined) => {
    activeBoardPathRef.current = path;
  }, []);

  useEntityEventListeners(activeBoardPathRef, refreshEntities, setEntitiesFor);

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
// Entity event listeners — extracted from RustEngineContainer for readability
//
// Architecture rule (event-architecture): events are thin signals with
// exactly two granularities. Each handler has ONE code path:
//
//   entity-created       → add from payload fields (fast) or get_entity (fallback)
//   entity-field-changed → patch individual fields from changes array
//   entity-removed       → remove from store by id
//
// DO NOT add full-state replacement or get_entity re-fetch to field-changed.
// ---------------------------------------------------------------------------

/** Deps shared by all entity event handlers. */
interface EventHandlerDeps {
  activeBoardPathRef: React.RefObject<string | undefined>;
  refreshEntities: RefreshEntitiesFn;
  setEntitiesFor: (type: string, updater: (prev: Entity[]) => Entity[]) => void;
}

/** Check if an event's board_path matches the active board. */
function isBoardMismatch(boardPath: string | undefined, activeRef: React.RefObject<string | undefined>): boolean {
  return !!(boardPath && activeRef.current && boardPath !== activeRef.current);
}

/**
 * Handle entity-created events.
 *
 * Fast path: use payload fields directly when available.
 * Fallback: fetch via get_entity when fields are empty.
 */
function handleEntityCreated(payload: EntityCreatedEvent, deps: EventHandlerDeps): void {
  const { entity_type, id, fields, board_path } = payload;
  if (isBoardMismatch(board_path, deps.activeBoardPathRef)) return;

  if (entity_type === "column") {
    if (deps.activeBoardPathRef.current) deps.refreshEntities(deps.activeBoardPathRef.current);
    return;
  }

  if (fields && Object.keys(fields).length > 0) {
    const entity: Entity = { id, entity_type, moniker: `${entity_type}:${id}`, fields: fields as Record<string, unknown> };
    deps.setEntitiesFor(entity_type, (prev) =>
      prev.some((e) => e.id === id) ? prev.map((e) => (e.id === id ? entity : e)) : [...prev, entity],
    );
    return;
  }

  invoke<EntityBag>("get_entity", {
    entityType: entity_type, id,
    ...(deps.activeBoardPathRef.current ? { boardPath: deps.activeBoardPathRef.current } : {}),
  })
    .then((bag) => {
      const entity = entityFromBag(bag);
      deps.setEntitiesFor(entity_type, (prev) =>
        prev.some((e) => e.id === id) ? prev.map((e) => (e.id === id ? entity : e)) : [...prev, entity],
      );
    })
    .catch((err) => console.error(`[entity-created] Failed to fetch ${entity_type}/${id}:`, err));
}

/**
 * Handle entity-removed events.
 *
 * Structural types (column) trigger a full refresh; others are removed by ID.
 */
function handleEntityRemoved(payload: EntityRemovedEvent, deps: EventHandlerDeps): void {
  const { entity_type, id, board_path } = payload;
  if (isBoardMismatch(board_path, deps.activeBoardPathRef)) return;

  if (entity_type === "column") {
    if (deps.activeBoardPathRef.current) deps.refreshEntities(deps.activeBoardPathRef.current);
  } else {
    deps.setEntitiesFor(entity_type, (prev) => prev.filter((e) => e.id !== id));
  }
}

/**
 * Handle entity-field-changed events.
 *
 * Patches individual fields in place. If the entity isn't in the store yet
 * (race with entity-created), upserts from the changes array.
 */
function handleEntityFieldChanged(payload: EntityFieldChangedEvent, deps: EventHandlerDeps): void {
  const { entity_type, id, changes, board_path } = payload;
  if (isBoardMismatch(board_path, deps.activeBoardPathRef)) return;
  if (!changes || changes.length === 0) return;

  deps.setEntitiesFor(entity_type, (prev) => {
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
      return [...next, { entity_type, id, moniker: `${entity_type}:${id}`, fields }];
    }
    return next;
  });
}

/**
 * Hook that subscribes to Tauri entity events and dispatches to handlers.
 *
 * Listens for entity-created, entity-removed, and entity-field-changed events
 * and cleans up subscriptions on unmount.
 */
function useEntityEventListeners(
  activeBoardPathRef: React.RefObject<string | undefined>,
  refreshEntities: RefreshEntitiesFn,
  setEntitiesFor: (type: string, updater: (prev: Entity[]) => Entity[]) => void,
): void {
  useEffect(() => {
    const deps: EventHandlerDeps = { activeBoardPathRef, refreshEntities, setEntitiesFor };
    const unlisteners = [
      listen<EntityCreatedEvent>("entity-created", (e) => handleEntityCreated(e.payload, deps)),
      listen<EntityRemovedEvent>("entity-removed", (e) => handleEntityRemoved(e.payload, deps)),
      listen<EntityFieldChangedEvent>("entity-field-changed", (e) => handleEntityFieldChanged(e.payload, deps)),
    ];
    return () => { for (const p of unlisteners) p.then((fn: () => void) => fn()); };
  }, [activeBoardPathRef, refreshEntities, setEntitiesFor]);
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
  refreshEntities, setEntitiesByType, setActiveBoardPath, entitiesByType, children,
}: EngineProviderStackProps) {
  return (
    <CommandScopeProvider commands={[]} moniker="engine">
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
