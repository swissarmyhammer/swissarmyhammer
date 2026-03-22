import { useCallback, useRef, useState } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { FieldDef } from "@/types/kanban";
import type { EditorProps } from "./markdown-editor";

interface SelectEditorProps extends EditorProps {
  field: FieldDef;
}

/** Select editor using shadcn/Radix Select. Commits on selection, Enter, or blur. */
export function SelectEditor({ field, value, onCommit, onCancel }: SelectEditorProps) {
  const options = ((field.type as Record<string, unknown>).options as Array<{ value: string; label?: string; color?: string }>) ?? [];
  const initial = typeof value === "string" ? value : "";
  const [draft, setDraft] = useState(initial);
  const [open, setOpen] = useState(true);
  const committedRef = useRef(false);

  const commit = useCallback(
    (val: string) => {
      if (committedRef.current) return;
      committedRef.current = true;
      onCommit(val);
    },
    [onCommit],
  );

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return (
    <Select
      value={draft}
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        // Closing the dropdown without a new selection = commit current value
        if (!next && !committedRef.current) {
          commit(draft);
        }
      }}
      onValueChange={(val) => {
        setDraft(val);
        setOpen(false);
        commit(val);
      }}
    >
      <SelectTrigger
        size="sm"
        className="w-full text-sm h-auto py-1 px-2"
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            e.stopPropagation();
            setOpen(false);
            commit(draft);
          } else if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            setOpen(false);
            cancel();
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
