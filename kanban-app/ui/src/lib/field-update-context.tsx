import {
  createContext,
  useContext,
  useCallback,
  type ReactNode,
} from "react";
import { error as logError } from "@/lib/log";
import { useDispatchCommand } from "@/lib/command-scope";

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
  const dispatch = useDispatchCommand("entity.update_field");

  const updateField: UpdateFieldFn = useCallback(
    async (entityType, entityId, fieldName, value) => {
      try {
        await dispatch({
          args: {
            entity_type: entityType,
            id: entityId,
            field_name: fieldName,
            value,
          },
        });
      } catch (e: unknown) {
        const msg = e instanceof Error ? e.message : String(e);
        logError(
          `updateField failed: ${entityType}/${entityId}.${fieldName}: ${msg}`,
        );
        throw e;
      }
    },
    [dispatch],
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
const NO_OP_CONTEXT: FieldUpdateContextValue = {
  updateField: async () => {},
};

export function useFieldUpdate(): FieldUpdateContextValue {
  const ctx = useContext(FieldUpdateContext);
  return ctx ?? NO_OP_CONTEXT;
}
