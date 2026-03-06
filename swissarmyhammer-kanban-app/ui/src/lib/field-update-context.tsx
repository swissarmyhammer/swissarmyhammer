import { createContext, useContext, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { error as logError } from "@/lib/log";

/**
 * Signature for the centralized field update function.
 *
 * All entity field mutations go through this single path:
 * invoke("dispatch_command", { cmd: "entity.update_field", args }).
 * The Rust side emits a "board-changed" event automatically for undoable
 * commands, so no manual refresh callback is needed.
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
  children: ReactNode;
}

/**
 * Provides a single `updateField` function to the entire component tree.
 *
 * Every component that edits an entity field — TaskCard title, column
 * rename, tag inspector, EntityInspector fields — calls the same function.
 * This centralizes error handling and logging in one place. Refresh is
 * handled automatically via the "board-changed" Tauri event emitted by
 * `dispatch_command`.
 */
export function FieldUpdateProvider({ children }: FieldUpdateProviderProps) {
  const updateField: UpdateFieldFn = useCallback(
    async (entityType, entityId, fieldName, value) => {
      try {
        await invoke("dispatch_command", {
          cmd: "entity.update_field",
          args: { entity_type: entityType, id: entityId, field_name: fieldName, value },
        });
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        logError(`updateField failed: ${entityType}/${entityId}.${fieldName}: ${msg}`);
        throw e;
      }
    },
    [],
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
