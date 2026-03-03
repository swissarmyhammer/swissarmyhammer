import type { FieldDef } from "@/types/kanban";

interface FieldPlaceholderProps {
  field: FieldDef;
  value: unknown;
  editing: boolean;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

/** Placeholder presenter/editor for field types not yet implemented. */
export function FieldPlaceholder({
  field,
  value,
  editing,
  onEdit,
  onCommit,
  onCancel,
}: FieldPlaceholderProps) {
  if (editing) {
    return (
      <div className="text-sm">
        <input
          autoFocus
          type="text"
          defaultValue={typeof value === "string" ? value : JSON.stringify(value ?? "")}
          className="w-full bg-transparent border-b border-ring text-sm px-0 py-0.5 outline-none"
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              onCommit((e.target as HTMLInputElement).value);
            } else if (e.key === "Escape") {
              onCancel();
            }
          }}
          onBlur={(e) => onCommit(e.target.value)}
        />
      </div>
    );
  }

  const display = formatValue(field, value);

  return (
    <div
      className="text-sm cursor-text min-h-[1.25rem]"
      onClick={onEdit}
    >
      {display || <span className="text-muted-foreground italic">Empty</span>}
    </div>
  );
}

function formatValue(_field: FieldDef, value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  if (typeof value === "boolean") return value ? "Yes" : "No";
  if (Array.isArray(value)) return value.join(", ");
  return JSON.stringify(value);
}
