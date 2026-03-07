import { useCallback, useEffect, useRef, useState } from "react";
import type { EditorProps } from "./markdown-editor";

/** Date input editor. Commits on Enter/blur, cancels on Escape. */
export function DateEditor({ value, onCommit, onCancel }: EditorProps) {
  const initial = typeof value === "string" ? value : "";
  const [draft, setDraft] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const committedRef = useRef(false);

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  const commit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCommit(draft);
  }, [draft, onCommit]);

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  return (
    <input
      ref={ref}
      type="date"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") { e.preventDefault(); commit(); }
        else if (e.key === "Escape") { e.preventDefault(); cancel(); }
        e.stopPropagation();
      }}
      onBlur={commit}
      className="w-full px-3 py-1.5 text-sm bg-transparent border-none outline-none ring-0"
    />
  );
}
