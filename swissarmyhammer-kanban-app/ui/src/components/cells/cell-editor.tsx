import { useCallback, useEffect, useRef, useState } from "react";
import { FieldPlaceholderEditor } from "@/components/fields/field-placeholder";
import type { FieldDef } from "@/types/kanban";

interface CellEditorProps {
  field: FieldDef;
  value: unknown;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

/**
 * Inline cell editor for grid view.
 * Uses CM6 (via FieldPlaceholderEditor) for text/markdown fields,
 * native select for select fields, and native inputs for number/date/color.
 * Commits on Enter/blur, cancels on Escape.
 */
export function CellEditor({ field, value, onCommit, onCancel }: CellEditorProps) {
  const kind = field.type.kind;

  if (kind === "select") {
    return <SelectCellEditor field={field} value={value} onCommit={onCommit} onCancel={onCancel} />;
  }

  if (kind === "number" || kind === "integer") {
    return <InputCellEditor type="number" value={value} onCommit={onCommit} onCancel={onCancel} />;
  }

  if (kind === "date") {
    return <InputCellEditor type="date" value={value} onCommit={onCommit} onCancel={onCancel} />;
  }

  if (kind === "color") {
    return <InputCellEditor type="color" value={value} isColor onCommit={onCommit} onCancel={onCancel} />;
  }

  // Default: CM6 markdown editor for text/markdown/string fields
  return (
    <FieldPlaceholderEditor
      value={toStr(value)}
      onCommit={(text) => onCommit(text)}
      onCancel={onCancel}
    />
  );
}

function InputCellEditor({
  type,
  value,
  isColor,
  onCommit,
  onCancel,
}: {
  type: string;
  value: unknown;
  isColor?: boolean;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}) {
  const initial = isColor && typeof value === "string" ? `#${value}` : toStr(value);
  const [draft, setDraft] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const committedRef = useRef(false);

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  const commit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    let val: unknown = draft;
    if (type === "number") val = draft === "" ? null : Number(draft);
    else if (isColor) val = draft.replace("#", "");
    onCommit(val);
  }, [draft, type, isColor, onCommit]);

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return (
    <input
      ref={ref}
      type={type}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") { e.preventDefault(); commit(); }
        else if (e.key === "Escape") { e.preventDefault(); cancel(); }
        e.stopPropagation();
      }}
      onBlur={commit}
      className="w-full px-3 py-1.5 text-sm bg-transparent border-none outline-none ring-0"
    />
  );
}

function SelectCellEditor({
  field,
  value,
  onCommit,
  onCancel,
}: {
  field: FieldDef;
  value: unknown;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}) {
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

function toStr(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  if (Array.isArray(value)) return value.join(", ");
  return String(value);
}
