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
  const caretOffsetRef = useRef<number | null>(null);

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

      const offset = caretOffsetRef.current;
      caretOffsetRef.current = null;

      if (offset !== null) {
        el.setSelectionRange(offset, offset);
      } else {
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
        // Resolve click position to a character offset while the span's
        // text node is still in the DOM. caretRangeFromPoint gives us
        // the offset into the text node under the cursor.
        let offset: number | null = null;
        if (document.caretRangeFromPoint) {
          const range = document.caretRangeFromPoint(e.clientX, e.clientY);
          if (range) {
            offset = range.startOffset;
          }
        }
        caretOffsetRef.current = offset;
        setDraft(value);
        setEditing(true);
      }}
    >
      {display}
    </span>
  );
}
