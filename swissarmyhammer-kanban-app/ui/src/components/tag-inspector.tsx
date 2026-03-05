import { useCallback, useRef, useState } from "react";
import { HexColorPicker } from "react-colorful";
import { X } from "lucide-react";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import { EditableMarkdown } from "@/components/editable-markdown";
import { useFieldUpdate } from "@/lib/field-update-context";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

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
  entity: Entity;
  onClose: () => void;
  style?: React.CSSProperties;
}

/**
 * Inspector panel for editing a tag entity.
 *
 * Accepts a raw Entity (entity_type: "tag") and updates individual fields
 * via the update_entity_field IPC command.
 */
export function TagInspector({
  entity,
  onClose,
  style,
}: TagInspectorProps) {
  const { updateField: contextUpdateField } = useFieldUpdate();
  const tagName = getStr(entity, "tag_name");
  const tagColor = getStr(entity, "color", "888888");
  const tagDescription = getStr(entity, "description");

  const [selectedColor, setSelectedColor] = useState(tagColor);
  const [pickerOpen, setPickerOpen] = useState(false);

  const colorTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const updateField = useCallback(
    async (fieldName: string, value: string) => {
      try {
        await contextUpdateField("tag", entity.id, fieldName, value);
      } catch {
        // contextUpdateField already logs errors
      }
    },
    [entity.id, contextUpdateField],
  );

  /** Debounced save for the color picker (fires rapidly while dragging). */
  const saveColorDebounced = useCallback(
    (color: string) => {
      clearTimeout(colorTimerRef.current);
      colorTimerRef.current = setTimeout(() => updateField("color", color), 150);
    },
    [updateField],
  );

  const saveName = useCallback(
    async (name: string) => {
      await updateField("tag_name", name);
    },
    [updateField],
  );

  const saveDescription = useCallback(
    async (desc: string) => {
      await updateField("description", desc);
    },
    [updateField],
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
            value={tagName}
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
                    updateField("color", color);
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
            value={tagDescription}
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
