import { createContext, useContext, useEffect, useState, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EntitySchema, FieldDef } from "@/types/kanban";

interface SchemaContextValue {
  getSchema: (entityType: string) => EntitySchema | undefined;
  getFieldDef: (entityType: string, fieldName: string) => FieldDef | undefined;
  loading: boolean;
}

const SchemaContext = createContext<SchemaContextValue | null>(null);

/** Entity types to pre-load schemas for on mount. */
const PRELOAD_TYPES = ["task", "column", "tag", "board", "swimlane"];

/**
 * Provides cached EntitySchema lookups to the component tree.
 *
 * On mount, pre-fetches schemas for all core entity types via the
 * `get_entity_schema` Tauri command. Components access schemas through
 * the `useSchema` hook which exposes `getSchema`, `getFieldDef`, and
 * a `loading` flag.
 */
export function SchemaProvider({ children }: { children: ReactNode }) {
  const [schemas, setSchemas] = useState<Map<string, EntitySchema>>(new Map());
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;

    async function loadSchemas() {
      const results = await Promise.allSettled(
        PRELOAD_TYPES.map(async (type) => {
          const schema = await invoke<EntitySchema>("get_entity_schema", { entityType: type });
          return [type, schema] as const;
        })
      );

      if (cancelled) return;

      const map = new Map<string, EntitySchema>();
      for (const result of results) {
        if (result.status === "fulfilled") {
          const [type, schema] = result.value;
          map.set(type, schema);
        }
      }
      setSchemas(map);
      setLoading(false);
    }

    loadSchemas();
    return () => { cancelled = true; };
  }, []);

  /** Return the cached schema for the given entity type, or undefined. */
  const getSchema = useCallback(
    (entityType: string) => schemas.get(entityType),
    [schemas]
  );

  /** Return a single field definition by entity type and field name. */
  const getFieldDef = useCallback(
    (entityType: string, fieldName: string) => {
      const schema = schemas.get(entityType);
      if (!schema) return undefined;
      return schema.fields.find((f) => f.name === fieldName);
    },
    [schemas]
  );

  return (
    <SchemaContext.Provider value={{ getSchema, getFieldDef, loading }}>
      {children}
    </SchemaContext.Provider>
  );
}

/**
 * Hook to access the schema context.
 *
 * Must be called within a `<SchemaProvider>`. Returns `getSchema`,
 * `getFieldDef`, and `loading`.
 */
export function useSchema() {
  const ctx = useContext(SchemaContext);
  if (!ctx) throw new Error("useSchema must be used within a SchemaProvider");
  return ctx;
}
