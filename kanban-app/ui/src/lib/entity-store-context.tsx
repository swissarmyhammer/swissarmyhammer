import {
  createContext,
  useContext,
  useCallback,
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import type { Entity } from "@/types/kanban";

interface EntityStoreContextValue {
  /** Get all entities of a given type. */
  getEntities: (entityType: string) => Entity[];
  /** Look up a single entity by type and id. */
  getEntity: (entityType: string, id: string) => Entity | undefined;
}

const EntityStoreContext = createContext<EntityStoreContextValue>({
  getEntities: () => [],
  getEntity: () => undefined,
});

interface EntityStoreProviderProps {
  /** All loaded entities, keyed by type. */
  entities: Record<string, Entity[]>;
  children: ReactNode;
}

/**
 * Generic entity store — provides loaded entities to the component tree.
 *
 * Components call `useEntityStore().getEntities("tag")` to get all tags,
 * or `getEntity("task", id)` to look up one task. No hardcoded knowledge
 * of which entity types exist.
 *
 * Uses a ref-based pattern so that `getEntities` and `getEntity` have
 * stable function identities. This prevents every context consumer from
 * re-rendering whenever any entity changes — components that receive
 * entities as props still re-render through normal prop diffing.
 */
export function EntityStoreProvider({
  entities,
  children,
}: EntityStoreProviderProps) {
  const entitiesRef = useRef(entities);
  entitiesRef.current = entities;

  const getEntities = useCallback(
    (entityType: string) => entitiesRef.current[entityType] ?? [],
    [],
  );

  const getEntity = useCallback(
    (entityType: string, id: string) =>
      entitiesRef.current[entityType]?.find((e) => e.id === id),
    [],
  );

  const value = useMemo(
    () => ({ getEntities, getEntity }),
    [getEntities, getEntity],
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
