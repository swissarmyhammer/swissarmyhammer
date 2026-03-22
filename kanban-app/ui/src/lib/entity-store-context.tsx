import {
  createContext,
  useContext,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Field-level subscription manager
// ---------------------------------------------------------------------------

type FieldSubscriber = () => void;

/** Key for a field-level subscription: "entityType:entityId:fieldName" */
function fieldKey(entityType: string, id: string, fieldName: string): string {
  return `${entityType}:${id}:${fieldName}`;
}

/** Deep equality for field values — handles primitives, arrays, and objects. */
function fieldValuesEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a == null || b == null) return a === b;
  if (typeof a !== typeof b) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((v, i) => fieldValuesEqual(v, b[i]));
  }
  if (typeof a === "object" && typeof b === "object") {
    const aObj = a as Record<string, unknown>;
    const bObj = b as Record<string, unknown>;
    const aKeys = Object.keys(aObj);
    const bKeys = Object.keys(bObj);
    if (aKeys.length !== bKeys.length) return false;
    return aKeys.every((k) => fieldValuesEqual(aObj[k], bObj[k]));
  }
  return false;
}

/**
 * Manages field-level subscriptions. Field components subscribe to a specific
 * (entityType, entityId, fieldName) tuple and get notified when that field changes.
 */
class FieldSubscriptions {
  private subs = new Map<string, Set<FieldSubscriber>>();

  subscribe(key: string, cb: FieldSubscriber): () => void {
    let set = this.subs.get(key);
    if (!set) {
      set = new Set();
      this.subs.set(key, set);
    }
    set.add(cb);
    return () => {
      set!.delete(cb);
      if (set!.size === 0) this.subs.delete(key);
    };
  }

  notify(key: string) {
    const set = this.subs.get(key);
    if (set) for (const cb of set) cb();
  }

  /** Diff old and new entities, notify subscribers for changed fields. */
  diff(
    prev: Record<string, Entity[]>,
    next: Record<string, Entity[]>,
  ) {
    // Check all entity types in the new state
    for (const entityType of Object.keys(next)) {
      const prevEntities = prev[entityType] ?? [];
      const nextEntities = next[entityType] ?? [];
      const prevMap = new Map(prevEntities.map((e) => [e.id, e]));

      for (const entity of nextEntities) {
        const old = prevMap.get(entity.id);
        if (!old) {
          // New entity — notify all fields
          for (const fieldName of Object.keys(entity.fields)) {
            this.notify(fieldKey(entityType, entity.id, fieldName));
          }
          continue;
        }
        // Existing entity — check each field by value
        for (const fieldName of Object.keys(entity.fields)) {
          if (!fieldValuesEqual(entity.fields[fieldName], old.fields[fieldName])) {
            this.notify(fieldKey(entityType, entity.id, fieldName));
          }
        }
        // Check for removed fields
        for (const fieldName of Object.keys(old.fields)) {
          if (!(fieldName in entity.fields)) {
            this.notify(fieldKey(entityType, entity.id, fieldName));
          }
        }
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

interface EntityStoreContextValue {
  /** Get all entities of a given type. */
  getEntities: (entityType: string) => Entity[];
  /** Look up a single entity by type and id. */
  getEntity: (entityType: string, id: string) => Entity | undefined;
  /** Subscribe to changes on a specific field. Returns unsubscribe function. */
  subscribeField: (entityType: string, id: string, fieldName: string, cb: () => void) => () => void;
  /** Get a specific field value (snapshot for useSyncExternalStore). */
  getFieldValue: (entityType: string, id: string, fieldName: string) => unknown;
}

const EntityStoreContext = createContext<EntityStoreContextValue>({
  getEntities: () => [],
  getEntity: () => undefined,
  subscribeField: () => () => {},
  getFieldValue: () => undefined,
});

interface EntityStoreProviderProps {
  /** All loaded entities, keyed by type. */
  entities: Record<string, Entity[]>;
  children: ReactNode;
}

/**
 * Generic entity store with field-level change subscriptions.
 *
 * Components call `getEntities("tag")` for bulk access, or
 * `useFieldValue("task", id, "title")` for reactive field binding.
 * Field-level subscribers re-render only when their specific field changes.
 */
export function EntityStoreProvider({
  entities,
  children,
}: EntityStoreProviderProps) {
  const entitiesRef = useRef(entities);
  const subsRef = useRef(new FieldSubscriptions());

  // Diff and notify on every entities change
  const prevRef = useRef(entities);
  useEffect(() => {
    if (prevRef.current !== entities) {
      subsRef.current.diff(prevRef.current, entities);
      prevRef.current = entities;
    }
    entitiesRef.current = entities;
  }, [entities]);

  const getEntities = useCallback(
    (entityType: string) => entitiesRef.current[entityType] ?? [],
    [],
  );

  const getEntity = useCallback(
    (entityType: string, id: string) =>
      entitiesRef.current[entityType]?.find((e) => e.id === id),
    [],
  );

  const subscribeField = useCallback(
    (entityType: string, id: string, fieldName: string, cb: () => void) =>
      subsRef.current.subscribe(fieldKey(entityType, id, fieldName), cb),
    [],
  );

  // Snapshot cache for useSyncExternalStore — returns stable references
  // when the value hasn't changed, preventing unnecessary re-renders.
  const snapshotCache = useRef(new Map<string, unknown>());

  const getFieldValue = useCallback(
    (entityType: string, id: string, fieldName: string) => {
      const raw = entitiesRef.current[entityType]?.find((e) => e.id === id)?.fields[fieldName];
      const key = fieldKey(entityType, id, fieldName);
      const cached = snapshotCache.current.get(key);
      if (fieldValuesEqual(raw, cached)) return cached;
      snapshotCache.current.set(key, raw);
      return raw;
    },
    [],
  );

  const value = useMemo(
    () => ({ getEntities, getEntity, subscribeField, getFieldValue }),
    [getEntities, getEntity, subscribeField, getFieldValue],
  );

  return (
    <EntityStoreContext.Provider value={value}>
      {children}
    </EntityStoreContext.Provider>
  );
}

export function useEntityStore(): EntityStoreContextValue {
  return useContext(EntityStoreContext);
}

/**
 * Subscribe to a single field value. Re-renders only when this specific
 * field changes — not when other fields on the same entity change.
 */
export function useFieldValue(entityType: string, entityId: string, fieldName: string): unknown {
  const { subscribeField, getFieldValue } = useEntityStore();

  const subscribe = useCallback(
    (cb: () => void) => subscribeField(entityType, entityId, fieldName, cb),
    [subscribeField, entityType, entityId, fieldName],
  );

  const getSnapshot = useCallback(
    () => getFieldValue(entityType, entityId, fieldName),
    [getFieldValue, entityType, entityId, fieldName],
  );

  return useSyncExternalStore(subscribe, getSnapshot);
}
