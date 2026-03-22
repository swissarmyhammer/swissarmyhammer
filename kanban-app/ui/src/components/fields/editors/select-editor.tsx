import { useCallback, useEffect, useRef, useState } from "react";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useUIState } from "@/lib/ui-state-context";
import type { FieldDef } from "@/types/kanban";
import type { EditorProps } from "./markdown-editor";

interface SelectEditorProps extends EditorProps {
  field: FieldDef;
}

/** Select dropdown editor. Saves directly via updateField on change/blur. */
export function SelectEditor({ field, value, entityType, entityId, fieldName, onCommit, onCancel }: SelectEditorProps) {
  const options = ((field.type as Record<string, unknown>).options as Array<{ value: string; label?: string }>) ?? [];
  const [draft, setDraft] = useState(typeof value === "string" ? value : "");
  const ref = useRef<HTMLSelectElement>(null);
  const committedRef = useRef(false);
  const { updateField } = useFieldUpdate();
  const { keymap_mode: mode } = useUIState();

  useEffect(() => {
    ref.current?.focus();
  }, []);

  /** Save to entity and call legacy onCommit. */
  const commit = useCallback((val: string) => {
    if (committedRef.current) return;
    committedRef.current = true;
    if (entityType && entityId && fieldName) {
      updateField(entityType, entityId, fieldName, val).catch(() => {});
    }
    onCommit(val);
  }, [onCommit, entityType, entityId, fieldName, updateField]);

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return (
    <select
      ref={ref}
      value={draft}
      onChange={(e) => {
        setDraft(e.target.value);
        commit(e.target.value);
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          if (mode === "vim") commit(draft);
          else cancel();
        }
        e.stopPropagation();
      }}
      onBlur={() => {
        if (!committedRef.current) commit(draft);
      }}
      className="w-full px-3 py-1.5 text-sm bg-transparent border-none outline-none ring-0"
    >
      <option value="">-</option>
      {options.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label ?? opt.value}
        </option>
      ))}
    </select>
  );
}
