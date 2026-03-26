import { useCallback, useRef, useState } from "react";
import { useUIState } from "@/lib/ui-state-context";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { FieldDef } from "@/types/kanban";
import type { EditorProps } from ".";

interface SelectEditorProps extends EditorProps {
  field: FieldDef;
}

/** Select editor using shadcn/Radix Select. Commits on selection, Enter, or blur. */
export function SelectEditor({
  field,
  value,
  onCommit,
  onCancel,
}: SelectEditorProps) {
  const options =
    ((field.type as Record<string, unknown>).options as Array<{
      value: string;
      label?: string;
      color?: string;
    }>) ?? [];
  const initial = typeof value === "string" ? value : "";
  const [draft, setDraft] = useState(initial);
  const [open, setOpen] = useState(true);
  const committedRef = useRef(false);
  const cancelledRef = useRef(false);
  const { keymap_mode: mode } = useUIState();

  const commit = useCallback(
    (val: string) => {
      if (committedRef.current || cancelledRef.current) return;
      committedRef.current = true;
      onCommit(val);
    },
    [onCommit],
  );

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    cancelledRef.current = true;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return (
    <Select
      value={draft}
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
      }}
      onValueChange={(val) => {
        setDraft(val);
      }}
    >
      <SelectTrigger
        size="sm"
        className="w-full text-sm h-auto py-1 px-2"
        onBlur={() => {
          if (!committedRef.current) commit(draft);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.stopPropagation();
            setOpen(false);
            commit(draft);
          } else if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            if (mode === "vim") commit(draft);
            else cancel();
            setOpen(false);
          }
        }}
      >
        <SelectValue placeholder="-" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="__empty__">-</SelectItem>
        {options.map((opt) => (
          <SelectItem key={opt.value} value={opt.value}>
            {opt.color && (
              <span
                className="inline-block w-2 h-2 rounded-full mr-1.5"
                style={{ backgroundColor: `#${opt.color}` }}
              />
            )}
            {opt.label ?? opt.value}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
