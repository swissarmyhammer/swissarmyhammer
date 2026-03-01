import { useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HexColorPicker } from "react-colorful";
import { X } from "lucide-react";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import { EditableMarkdown } from "@/components/editable-markdown";
import type { Tag } from "@/types/kanban";

/** 16-color palette matching Rust auto_color */
const PALETTE = [
  "d73a4a",
  "e36209",
  "f9c513",
  "0e8a16",
  "006b75",
  "1d76db",
  "0075ca",
  "5319e7",
  "b60205",
  "d93f0b",
  "fbca04",
  "0e8a16",
  "006b75",
  "1d76db",
  "6f42c1",
  "e4e669",
];

interface TagInspectorProps {
  tag: Tag;
  onClose: () => void;
  onRefresh: () => void;
  style?: React.CSSProperties;
}

export function TagInspector({
  tag,
  onClose,
  onRefresh,
  style,
}: TagInspectorProps) {
  const [selectedColor, setSelectedColor] = useState(tag.color);
  const [pickerOpen, setPickerOpen] = useState(false);

  const colorTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  /** Send a partial tag update — only the fields you pass get changed. */
  const updateTag = useCallback(
    async (fields: { name?: string; color?: string; description?: string }) => {
      try {
        await invoke("update_tag", { id: tag.id, ...fields });
        onRefresh();
      } catch (e) {
        console.error("Failed to update tag:", e);
      }
    },
    [tag.id, onRefresh],
  );

  // Debounced save for the color picker (fires rapidly while dragging)
  const saveColorDebounced = useCallback(
    (color: string) => {
      clearTimeout(colorTimerRef.current);
      colorTimerRef.current = setTimeout(() => updateTag({ color }), 150);
    },
    [updateTag],
  );

  const saveName = useCallback(
    async (name: string) => {
      await updateTag({ name });
    },
    [tag.name, updateTag],
  );

  const saveDescription = useCallback(
    async (desc: string) => {
      await updateTag({ description: desc });
    },
    [tag.description, updateTag],
  );

  return (
    <div
      className="fixed top-0 h-full w-[420px] max-w-[85vw] bg-background border-l border-border shadow-xl flex flex-col"
      style={style}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-3 px-5 pt-5 pb-3">
        <h2 className="text-lg font-semibold leading-snug">Edit Tag</h2>
        <button
          onClick={onClose}
          className="shrink-0 p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      <div className="mx-3 h-px bg-border" />

      {/* Body */}
      <div className="flex-1 min-h-0 overflow-y-auto px-5 py-4 space-y-4">
        {/* Tag name / rename */}
        <div>
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-1 block">
            Tag Name
          </label>
          <EditableMarkdown
            value={tag.name}
            onCommit={saveName}
            className="text-sm leading-snug cursor-text block"
            inputClassName="text-sm leading-snug bg-transparent border-b border-ring w-full"
          />
        </div>

        {/* Color palette + picker */}
        <div>
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-1 block">
            Color
          </label>
          <div className="flex items-start gap-2">
            <div className="grid grid-cols-8 gap-1 flex-1">
              {PALETTE.map((color) => (
                <button
                  key={color}
                  type="button"
                  className={`w-6 h-6 rounded-full border-2 transition-all ${
                    selectedColor === color
                      ? "border-foreground scale-110"
                      : "border-transparent hover:border-muted-foreground/50"
                  }`}
                  style={{ backgroundColor: `#${color}` }}
                  onClick={() => {
                    setSelectedColor(color);
                    updateTag({ color });
                  }}
                />
              ))}
            </div>
            <Popover open={pickerOpen} onOpenChange={setPickerOpen}>
              <PopoverTrigger asChild>
                <button
                  type="button"
                  className="shrink-0 w-8 h-8 rounded-md border border-input cursor-pointer"
                  style={{ backgroundColor: `#${selectedColor}` }}
                />
              </PopoverTrigger>
              <PopoverContent align="end" className="w-auto p-3">
                <HexColorPicker
                  color={`#${selectedColor}`}
                  onChange={(hex) => {
                    const c = hex.replace("#", "");
                    setSelectedColor(c);
                    saveColorDebounced(c);
                  }}
                />
                <div className="mt-2 flex items-center gap-2">
                  <span className="text-xs text-muted-foreground">#</span>
                  <input
                    type="text"
                    value={selectedColor}
                    onChange={(e) => {
                      const v = e.target.value
                        .replace(/[^0-9a-fA-F]/g, "")
                        .slice(0, 6);
                      setSelectedColor(v);
                      if (v.length === 6) saveColorDebounced(v);
                    }}
                    className="flex-1 text-xs font-mono bg-transparent border border-input rounded px-1.5 py-0.5"
                    maxLength={6}
                  />
                </div>
              </PopoverContent>
            </Popover>
          </div>
        </div>

        {/* Description */}
        <section className="flex-1 flex flex-col">
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-1 block">
            Description
          </label>
          <EditableMarkdown
            value={tag.description ?? ""}
            onCommit={saveDescription}
            className="text-sm leading-relaxed cursor-text flex-1"
            inputClassName="text-sm leading-relaxed bg-transparent w-full flex-1"
            multiline
            placeholder="Describe this tag..."
          />
        </section>
      </div>
    </div>
  );
}
