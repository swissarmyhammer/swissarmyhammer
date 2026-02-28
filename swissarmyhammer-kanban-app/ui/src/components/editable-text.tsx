import { useCallback, useEffect, useRef, useState } from "react";

interface EditableTextProps {
  value: string;
  onCommit: (value: string) => void;
  className?: string;
  inputClassName?: string;
  /** Render a textarea instead of a single-line input */
  multiline?: boolean;
  /** Placeholder shown when value is empty */
  placeholder?: string;
}

export function EditableText({
  value,
  onCommit,
  className,
  inputClassName,
  multiline,
  placeholder,
}: EditableTextProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const ref = useRef<HTMLInputElement & HTMLTextAreaElement>(null);
  const clickPosRef = useRef<{ x: number; y: number } | null>(null);

  const autoSize = useCallback(() => {
    const el = ref.current;
    if (el && multiline) {
      el.style.height = "auto";
      el.style.height = `${el.scrollHeight}px`;
    }
  }, [multiline]);

  useEffect(() => {
    if (editing && ref.current) {
      const el = ref.current;
      el.focus();
      autoSize();

      const pos = clickPosRef.current;
      clickPosRef.current = null;

      if (pos) {
        // Place caret where the user clicked by dispatching a synthetic
        // mousedown at the same coordinates. The input/textarea occupies
        // the same space as the span it replaced, so coordinates map
        // to the correct character position.
        requestAnimationFrame(() => {
          el.dispatchEvent(
            new MouseEvent("mousedown", {
              clientX: pos.x,
              clientY: pos.y,
              bubbles: true,
            })
          );
          el.dispatchEvent(
            new MouseEvent("mouseup", {
              clientX: pos.x,
              clientY: pos.y,
              bubbles: true,
            })
          );
        });
      } else {
        // Fallback: place cursor at end
        const len = el.value.length;
        el.setSelectionRange(len, len);
      }
    }
  }, [editing, autoSize]);

  const commit = useCallback(() => {
    setEditing(false);
    const trimmed = draft.trim();
    if (trimmed !== value) {
      onCommit(trimmed);
    }
  }, [draft, value, onCommit]);

  const cancel = useCallback(() => {
    setEditing(false);
    setDraft(value);
  }, [value]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !multiline) {
        e.preventDefault();
        commit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        cancel();
      }
    },
    [commit, cancel, multiline]
  );

  if (editing) {
    const shared = {
      ref: ref as React.RefObject<never>,
      value: draft,
      onChange: (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
        setDraft(e.target.value);
        autoSize();
      },
      onBlur: commit,
      onKeyDown: handleKeyDown,
      className: inputClassName ?? className,
    };

    return multiline ? (
      <textarea {...shared} rows={1} style={{ overflow: "hidden" }} />
    ) : (
      <input {...shared} type="text" />
    );
  }

  const display = value || placeholder;
  const isEmpty = !value && placeholder;

  return (
    <span
      className={`${className ?? ""}${isEmpty ? " text-muted-foreground italic" : ""}`}
      onClick={(e) => {
        clickPosRef.current = { x: e.clientX, y: e.clientY };
        setDraft(value);
        setEditing(true);
      }}
    >
      {display}
    </span>
  );
}
