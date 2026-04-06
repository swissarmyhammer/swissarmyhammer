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

type RefreshEntitiesFn = (boardPath: string) => Promise<RefreshResult>;

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
    async (boardPath: string): Promise<RefreshResult> => {
      activeBoardPathRef.current = boardPath;
      const result = await refreshBoards(boardPath);
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

  // -------------------------------------------------------------------------
  // Entity event listeners
  //
  // Architecture rule (event-architecture): events are thin signals with
  // exactly two granularities. Each handler has ONE code path:
  //
  //   entity-created  → add to store from payload fields (fast path),
  //                      or get_entity once if fields are empty (fallback).
  //   entity-field-changed → patch individual fields from changes array.
  //                          No full-state replacement, no get_entity.
  //   entity-removed  → remove from store by id.
  //
  // DO NOT add a full-fields replacement path or a get_entity re-fetch
  // fallback to entity-field-changed. The watcher produces per-field diffs.
  // -------------------------------------------------------------------------

  useEffect(() => {
    const unlisteners = [
      // Architecture rule (event-architecture): entity-created events carry
      // fields from the watcher's cache population. Use them directly when
      // available. Fall back to get_entity only when fields are empty (store
      // event arrived before the watcher cached the file).
      listen<EntityCreatedEvent>("entity-created", (event) => {
        const { entity_type, id, fields, board_path } = event.payload;
        console.warn(
          `[entity-created] received: ${entity_type}/${id} board_path=${board_path ?? "none"} fields=${Object.keys(fields ?? {}).length}`,
        );
        if (
          board_path &&
          activeBoardPathRef.current &&
          board_path !== activeBoardPathRef.current
        ) {
          console.warn(
            `[entity-created] SKIPPED: board_path mismatch (active=${activeBoardPathRef.current})`,
          );
          return;
        }
        if (entity_type === "column") {
          console.warn(`[entity-created] structural type -> full refresh`);
          if (activeBoardPathRef.current) {
            refreshEntities(activeBoardPathRef.current);
          }
          return;
        }

        // Fast path: watcher provided fields from disk — use directly.
        if (fields && Object.keys(fields).length > 0) {
          console.warn(
            `[entity-created] using payload fields for ${entity_type}/${id}`,
          );
          const entity: Entity = {
            id,
            entity_type,
            fields: fields as Record<string, unknown>,
          };
          setEntitiesFor(entity_type, (prev) => {
            if (prev.some((e) => e.id === id)) {
              return prev.map((e) => (e.id === id ? entity : e));
            }
            return [...prev, entity];
          });
          return;
        }

        // Fallback: fields empty — fetch once via get_entity.
        console.warn(
          `[entity-created] fetching ${entity_type}/${id} via get_entity`,
        );
        invoke<EntityBag>("get_entity", {
          entityType: entity_type,
          id,
          ...(activeBoardPathRef.current
            ? { boardPath: activeBoardPathRef.current }
            : {}),
        })
          .then((bag) => {
            const entity = entityFromBag(bag);
            console.warn(
              `[entity-created] fetched ${entity_type}/${id}, fields: ${Object.keys(entity.fields).join(",")}`,
            );
            setEntitiesFor(entity_type, (prev) => {
              if (prev.some((e) => e.id === id)) {
                return prev.map((e) => (e.id === id ? entity : e));
              }
              return [...prev, entity];
            });
          })
          .catch((err) => {
            console.error(
              `[entity-created] Failed to fetch ${entity_type}/${id}:`,
              err,
            );
          });
      }),
      listen<EntityRemovedEvent>("entity-removed", (event) => {
        const { entity_type, id, board_path } = event.payload;
        console.warn(
          `[entity-removed] received: ${entity_type}/${id} board_path=${board_path ?? "none"}`,
        );
        if (
          board_path &&
          activeBoardPathRef.current &&
          board_path !== activeBoardPathRef.current
        ) {
          console.warn(`[entity-removed] SKIPPED: board_path mismatch`);
          return;
        }
        if (entity_type === "column") {
          if (activeBoardPathRef.current) {
            refreshEntities(activeBoardPathRef.current);
          }
        } else {
          setEntitiesFor(entity_type, (prev) =>
            prev.filter((e) => e.id !== id),
          );
        }
      }),
      // Architecture rule (event-architecture): ONE path. Patch individual
      // fields from the changes array. No full-state replacement, no re-fetch.
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        const { entity_type, id, changes, board_path } = event.payload;
        console.warn(
          `[entity-field-changed] received: ${entity_type}/${id} board_path=${board_path ?? "none"} changes=${changes?.length ?? 0}`,
        );
        if (
          board_path &&
          activeBoardPathRef.current &&
          board_path !== activeBoardPathRef.current
        ) {
          console.warn(
            `[entity-field-changed] SKIPPED: board_path mismatch (active=${activeBoardPathRef.current})`,
          );
          return;
        }

        if (!changes || changes.length === 0) {
          console.warn(
            `[entity-field-changed] SKIPPED: empty changes for ${entity_type}/${id}`,
          );
          return;
        }

        // Patch individual changed fields in place (upsert if not yet in store).
        setEntitiesFor(entity_type, (prev) => {
          let found = false;
          const next = prev.map((e) => {
            if (e.id !== id) return e;
            found = true;
            const patched = { ...e.fields };
            for (const { field, value } of changes) {
              patched[field] = value;
            }
            return { ...e, fields: patched };
          });
          if (!found) {
            // Race recovery: field-changed arrived before entity-created.
            // Construct entity from the changes so the patch isn't lost.
            const fields: Record<string, unknown> = {};
            for (const { field, value } of changes) {
              fields[field] = value;
            }
            console.warn(
              `[entity-field-changed] entity ${entity_type}/${id} not in store, upserting`,
            );
            return [...next, { entity_type, id, fields }];
          }
          return next;
        });
      }),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn: () => void) => fn());
      }
    };
  }, [refreshEntities, setEntitiesFor]);

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
