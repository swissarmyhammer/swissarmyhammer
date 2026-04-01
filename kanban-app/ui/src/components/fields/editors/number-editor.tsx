import { useCallback, useEffect, useRef, useState } from "react";
import { useUIState } from "@/lib/ui-state-context";
import type { EditorProps } from ".";

/** Numeric input editor. Commits on Enter or blur. */
export function NumberEditor({
  value,
  onCommit,
  onCancel,
  onChange,
}: EditorProps) {
  const initial = value != null ? String(value) : "";
  const [draft, setDraft] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const committedRef = useRef(false);
  const { keymap_mode: mode } = useUIState();

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  /** Commit the current draft value. */
  const commit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    const val = draft === "" ? null : Number(draft);
    onCommit(val);
  }, [draft, onCommit]);

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return (
    <input
      ref={ref}
      type="number"
      value={draft}
      onChange={(e) => {
        const v = e.target.value;
        setDraft(v);
        onChange?.(v === "" ? null : Number(v));
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          commit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          // Vim: Escape saves. CUA/emacs: Escape discards.
          if (mode === "vim") commit();
          else cancel();
        }
        e.stopPropagation();
      }}
      onBlur={commit}
      className="w-full px-3 py-1.5 text-sm bg-transparent border-none outline-none ring-0"
    />
  );
}
