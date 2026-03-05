import { createContext, useContext, useCallback, useMemo, type ReactNode } from "react";
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
 */
export function EntityStoreProvider({ entities, children }: EntityStoreProviderProps) {
  const getEntities = useCallback(
    (entityType: string) => entities[entityType] ?? [],
    [entities],
  );

  const getEntity = useCallback(
    (entityType: string, id: string) =>
      entities[entityType]?.find((e) => e.id === id),
    [entities],
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
