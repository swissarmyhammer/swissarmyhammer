import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as chrono from "chrono-node";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap, EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { getCM } from "@replit/codemirror-vim";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { Calendar } from "@/components/ui/calendar";
import { useUIState } from "@/lib/ui-state-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import type { EditorProps } from "./markdown-editor";

/** Format a Date as YYYY-MM-DD */
function toISO(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** Try to parse text as a date via chrono-node, return YYYY-MM-DD or null */
function parseNatural(text: string): string | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  // Try exact YYYY-MM-DD first
  if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) return trimmed;
  const results = chrono.parse(trimmed);
  if (results.length > 0) return toISO(results[0].start.date());
  return null;
}

/** Date editor — CM6 natural language input + calendar picker in a popover. */
export function DateEditor({ value, entityType, entityId, fieldName, onCommit, onCancel }: EditorProps) {
  const initial = typeof value === "string" ? value : "";
  const [draft, setDraft] = useState(initial);
  const [open, setOpen] = useState(true);
  const [resolved, setResolved] = useState<string | null>(initial || null);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const keymapCompartment = useRef(new Compartment());
  const committedRef = useRef(false);
  const { keymap_mode: mode } = useUIState();
  const { updateField } = useFieldUpdate();

  // Parse as user types
  useEffect(() => {
    setResolved(parseNatural(draft));
  }, [draft]);

  const commit = useCallback(
    (iso: string) => {
      if (committedRef.current) return;
      committedRef.current = true;
      if (entityType && entityId && fieldName) {
        updateField(entityType, entityId, fieldName, iso).catch(() => {});
      }
      onCommit(iso);
    },
    [onCommit, entityType, entityId, fieldName, updateField],
  );

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  const commitRef = useRef(commit);
  commitRef.current = commit;
  const cancelRef = useRef(cancel);
  cancelRef.current = cancel;
  const resolvedRef = useRef(resolved);
  resolvedRef.current = resolved;

  const commitResolved = useCallback(() => {
    const r = resolvedRef.current;
    if (r) commitRef.current(r);
    else cancelRef.current();
  }, []);

  const handleCalendarSelect = useCallback(
    (day: Date | undefined) => {
      if (!day) return;
      const iso = toISO(day);
      setDraft(iso);
      setResolved(iso);
      commit(iso);
    },
    [commit],
  );

  // Selected date for the calendar highlight
  const selectedDate = useMemo(() => {
    if (!resolved) return undefined;
    const [y, m, d] = resolved.split("-").map(Number);
    return new Date(y, m - 1, d);
  }, [resolved]);

  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      // Vim: Escape in normal mode commits, insert→normal saves
      ...(mode === "vim"
        ? [
            EditorView.domEventHandlers({
              keydown(event, view) {
                if (event.key === "Escape") {
                  const cm = getCM(view);
                  if (cm?.state?.vim?.insertMode) return false;
                  const r = resolvedRef.current;
                  if (r) commitRef.current(r);
                  else cancelRef.current();
                  return true;
                }
                if (event.key === "Enter") {
                  event.preventDefault();
                  const r = resolvedRef.current;
                  if (r) commitRef.current(r);
                  return true;
                }
                return false;
              },
            }),
          ]
        : [
            keymap.of([
              {
                key: "Escape",
                run: () => { cancelRef.current(); return true; },
              },
              {
                key: "Enter",
                run: () => {
                  const r = resolvedRef.current;
                  if (r) commitRef.current(r);
                  else cancelRef.current();
                  return true;
                },
              },
            ]),
          ]),
    ],
    [mode],
  );

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) commitResolved();
      }}
    >
      <PopoverTrigger asChild>
        <div className="cursor-pointer">
          {(resolved ?? initial) ? (
            <span className="text-sm tabular-nums">{resolved ?? initial}</span>
          ) : (
            <span className="text-muted-foreground/50">-</span>
          )}
        </div>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-auto p-0"
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            setOpen(false);
            cancel();
          }
        }}
      >
        {/* CM6 natural language input */}
        <div className="p-3 pb-0 space-y-1">
          <CodeMirror
            ref={editorRef}
            autoFocus
            value={draft}
            onChange={(val) => setDraft(val)}
            extensions={extensions}
            theme={shadcnTheme}
            basicSetup={{
              lineNumbers: false,
              foldGutter: false,
              highlightActiveLine: false,
              highlightActiveLineGutter: false,
              indentOnInput: false,
              bracketMatching: false,
              autocompletion: false,
            }}
            className="text-sm border border-input rounded-md px-2 py-1"
            placeholder="Type a date... (e.g. tomorrow, next friday)"
          />
          {draft && (
            <div className="text-xs px-1">
              {resolved ? (
                <span className="text-muted-foreground">
                  &rarr; {resolved}
                </span>
              ) : (
                <span className="text-destructive">
                  Could not parse date
                </span>
              )}
            </div>
          )}
        </div>
        {/* Calendar picker */}
        <Calendar
          mode="single"
          selected={selectedDate}
          onSelect={handleCalendarSelect}
          defaultMonth={selectedDate}
        />
      </PopoverContent>
    </Popover>
  );
}
