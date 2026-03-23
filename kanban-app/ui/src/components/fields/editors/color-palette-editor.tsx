import { useCallback, useRef, useState } from "react";
import { HexColorPicker } from "react-colorful";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { ColorSwatchDisplay } from "@/components/fields/displays/color-swatch-display";
import type { EditorProps } from "./markdown-editor";

/**
 * Color editor — popover with HexColorPicker.
 * Picking a color updates the draft. Closing the popover (blur, click-outside)
 * or pressing Enter commits. Escape cancels.
 */
export function ColorPaletteEditor({ value, onCommit, onCancel }: EditorProps) {
  const initial = typeof value === "string" ? value : "888888";
  const [draft, setDraft] = useState(initial);
  const [open, setOpen] = useState(true);
  const committedRef = useRef(false);
  const cancelledRef = useRef(false);

  const commit = useCallback(
    (hex: string) => {
      if (committedRef.current || cancelledRef.current) return;
      committedRef.current = true;
      onCommit(hex);
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
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        // Commit on close (click-outside, blur) — but not if Escape cancelled
        if (!next && !committedRef.current && !cancelledRef.current) {
          commit(draft);
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
          if (e.key === "Enter") {
            e.preventDefault();
            e.stopPropagation();
            setOpen(false);
            commit(draft);
          } else if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            cancel();
            setOpen(false);
          }
        }}
      >
        <HexColorPicker
          color={`#${draft}`}
          onChange={(hex) => {
            setDraft(hex.replace("#", ""));
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
            }}
            className="flex-1 text-xs font-mono bg-transparent border border-input rounded px-1.5 py-0.5"
            maxLength={6}
          />
        </div>
      </PopoverContent>
    </Popover>
  );
}
