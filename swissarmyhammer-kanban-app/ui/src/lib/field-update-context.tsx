import { createContext, useContext, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { error as logError } from "@/lib/log";

/**
 * Signature for the centralized field update function.
 *
 * All entity field mutations go through this single path:
 * invoke("update_entity_field") → refresh board state.
 */
type UpdateFieldFn = (
  entityType: string,
  entityId: string,
  fieldName: string,
  value: unknown,
) => Promise<void>;

interface FieldUpdateContextValue {
  updateField: UpdateFieldFn;
}

const FieldUpdateContext = createContext<FieldUpdateContextValue | null>(null);

interface FieldUpdateProviderProps {
  /** Called after every successful field update to reload board state. */
  onRefresh: () => void | Promise<void>;
  children: ReactNode;
}

/**
 * Provides a single `updateField` function to the entire component tree.
 *
 * Every component that edits an entity field — TaskCard title, column
 * rename, tag inspector, EntityInspector fields — calls the same function.
 * This centralizes error handling, logging, and the refresh-after-save
 * pattern in one place.
 */
export function FieldUpdateProvider({ onRefresh, children }: FieldUpdateProviderProps) {
  const updateField: UpdateFieldFn = useCallback(
    async (entityType, entityId, fieldName, value) => {
      try {
        await invoke("update_entity_field", {
          entityType,
          id: entityId,
          fieldName,
          value,
        });
        await onRefresh();
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        logError(`updateField failed: ${entityType}/${entityId}.${fieldName}: ${msg}`);
        throw e;
      }
    },
    [onRefresh],
  );

  return (
    <FieldUpdateContext.Provider value={{ updateField }}>
      {children}
    </FieldUpdateContext.Provider>
  );
}

/**
 * Hook to access the centralized field update function.
 *
 * Usage:
 * ```tsx
 * const { updateField } = useFieldUpdate();
 * await updateField("task", taskId, "title", newTitle);
 * ```
 */
export function useFieldUpdate(): FieldUpdateContextValue {
  const ctx = useContext(FieldUpdateContext);
  if (!ctx) throw new Error("useFieldUpdate must be used within a FieldUpdateProvider");
  return ctx;
}
