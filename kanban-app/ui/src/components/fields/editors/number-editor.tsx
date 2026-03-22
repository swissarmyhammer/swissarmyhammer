import { useCallback, useEffect, useRef, useState } from "react";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useUIState } from "@/lib/ui-state-context";
import type { EditorProps } from "./markdown-editor";

/** Numeric input editor. Saves directly via updateField on Enter/blur. */
export function NumberEditor({ value, entityType, entityId, fieldName, onCommit, onCancel }: EditorProps) {
  const initial = value != null ? String(value) : "";
  const [draft, setDraft] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const committedRef = useRef(false);
  const { updateField } = useFieldUpdate();
  const { keymap_mode: mode } = useUIState();

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  /** Save to entity and call legacy onCommit. */
  const commit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    const val = draft === "" ? null : Number(draft);
    if (entityType && entityId && fieldName) {
      updateField(entityType, entityId, fieldName, val).catch(() => {});
    }
    onCommit(val);
  }, [draft, onCommit, entityType, entityId, fieldName, updateField]);

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return (
    <input
      ref={ref}
      type="number"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") { e.preventDefault(); commit(); }
        else if (e.key === "Escape") {
          e.preventDefault();
          // Vim: Escape saves. CUA/emacs: Escape discards.
          if (mode === "vim") commit();
          else cancel();
        }
        e.stopPropagation();
      }}
      onBlur={commit}
      className="w-full px-3 py-1.5 text-sm bg-transparent border-none outline-none ring-0"
    />
  );
}
