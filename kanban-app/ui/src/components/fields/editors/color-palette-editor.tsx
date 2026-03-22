import { useCallback, useRef, useState } from "react";
import { HexColorPicker } from "react-colorful";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { ColorSwatchDisplay } from "@/components/fields/displays/color-swatch-display";
import { useFieldUpdate } from "@/lib/field-update-context";
import type { EditorProps } from "./markdown-editor";

/** Color editor — renders ColorSwatchDisplay as trigger, popover with HexColorPicker. Saves directly via updateField. */
export function ColorPaletteEditor({ value, entityType, entityId, fieldName, onCommit, onCancel }: EditorProps) {
  const initial = typeof value === "string" ? value : "888888";
  const [draft, setDraft] = useState(initial);
  const [open, setOpen] = useState(true);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const { updateField } = useFieldUpdate();

  /** Save to entity and call legacy onCommit. */
  const save = useCallback(
    (hex: string) => {
      if (entityType && entityId && fieldName) {
        updateField(entityType, entityId, fieldName, hex).catch(() => {});
      }
      onCommit(hex);
    },
    [onCommit, entityType, entityId, fieldName, updateField],
  );

  const commitDebounced = useCallback(
    (hex: string) => {
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => save(hex), 150);
    },
    [save],
  );

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) {
          clearTimeout(timerRef.current);
          save(draft);
        }
      }}
    >
      <PopoverTrigger asChild>
        <div className="cursor-pointer">
          <ColorSwatchDisplay value={draft} />
        </div>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-auto p-3"
        onKeyDown={(e) => {
          if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); setOpen(false); onCancel(); }
        }}
      >
        <HexColorPicker
          color={`#${draft}`}
          onChange={(hex) => {
            const c = hex.replace("#", "");
            setDraft(c);
            commitDebounced(c);
          }}
        />
        <div className="mt-2 flex items-center gap-2">
          <span className="text-xs text-muted-foreground">#</span>
          <input
            type="text"
            value={draft}
            onChange={(e) => {
              const v = e.target.value.replace(/[^0-9a-fA-F]/g, "").slice(0, 6);
              setDraft(v);
              if (v.length === 6) commitDebounced(v);
            }}
            className="flex-1 text-xs font-mono bg-transparent border border-input rounded px-1.5 py-0.5"
            maxLength={6}
          />
        </div>
      </PopoverContent>
    </Popover>
  );
}
