import {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  useMemo,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type { EntityCommand, EntitySchema, FieldDef } from "@/types/kanban";

/** Describes an entity type that supports prefix-based mentions (e.g. #tag, @actor). */
export interface MentionableType {
  entityType: string;
  prefix: string;
  displayField: string;
}

interface SchemaContextValue {
  getSchema: (entityType: string) => EntitySchema | undefined;
  getFieldDef: (entityType: string, fieldName: string) => FieldDef | undefined;
  /** Return the commands array for an entity type, or [] if schema not loaded yet. */
  getEntityCommands: (entityType: string) => readonly EntityCommand[];
  /** Entity types that have mention_prefix defined — for CM6 decorations/autocomplete. */
  mentionableTypes: MentionableType[];
  loading: boolean;
}

const SchemaContext = createContext<SchemaContextValue | null>(null);

/** Entity types to pre-load schemas for on mount. */
const PRELOAD_TYPES = ["task", "column", "tag", "board", "swimlane", "actor"];

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
          const schema = await invoke<EntitySchema>("get_entity_schema", {
            entityType: type,
          });
          return [type, schema] as const;
        }),
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
    return () => {
      cancelled = true;
    };
  }, []);

  /** Return the cached schema for the given entity type, or undefined. */
  const getSchema = useCallback(
    (entityType: string) => schemas.get(entityType),
    [schemas],
  );

  /** Return a single field definition by entity type and field name. */
  const getFieldDef = useCallback(
    (entityType: string, fieldName: string) => {
      const schema = schemas.get(entityType);
      if (!schema) return undefined;
      return schema.fields.find((f) => f.name === fieldName);
    },
    [schemas],
  );

  /** Return the commands for the given entity type, or [] if schema not loaded. */
  const getEntityCommands = useCallback(
    (entityType: string): readonly EntityCommand[] => {
      const schema = schemas.get(entityType);
      return schema?.entity.commands ?? [];
    },
    [schemas],
  );

  const mentionableTypes = useMemo(() => {
    const result: MentionableType[] = [];
    for (const schema of schemas.values()) {
      const { mention_prefix, mention_display_field } = schema.entity;
      if (mention_prefix && mention_display_field) {
        result.push({
          entityType: schema.entity.name,
          prefix: mention_prefix,
          displayField: mention_display_field,
        });
      }
    }
    return result;
  }, [schemas]);

  return (
    <SchemaContext.Provider
      value={{
        getSchema,
        getFieldDef,
        getEntityCommands,
        mentionableTypes,
        loading,
      }}
    >
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

/**
 * Returns schema context if available, or a stub that always returns undefined/[].
 * Use in components optionally rendered outside a SchemaProvider.
 */
export function useSchemaOptional(): Pick<
  SchemaContextValue,
  "getSchema" | "getFieldDef" | "getEntityCommands"
> {
  const ctx = useContext(SchemaContext);
  if (ctx) return ctx;
  return {
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  };
}
