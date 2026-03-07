import { useCallback, useEffect, useRef, useState } from "react";
import type { FieldDef } from "@/types/kanban";
import type { EditorProps } from "./markdown-editor";

interface SelectEditorProps extends EditorProps {
  field: FieldDef;
}

/** Select dropdown editor for select fields. Commits on change, cancels on Escape. */
export function SelectEditor({ field, value, onCommit, onCancel }: SelectEditorProps) {
  const options = ((field.type as Record<string, unknown>).options as Array<{ value: string; label?: string }>) ?? [];
  const [draft, setDraft] = useState(typeof value === "string" ? value : "");
  const ref = useRef<HTMLSelectElement>(null);
  const committedRef = useRef(false);

  useEffect(() => {
    ref.current?.focus();
  }, []);

  const commit = useCallback((val: string) => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCommit(val);
  }, [onCommit]);

  return (
    <select
      ref={ref}
      value={draft}
      onChange={(e) => {
        setDraft(e.target.value);
        commit(e.target.value);
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") { e.preventDefault(); onCancel(); }
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
