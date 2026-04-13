import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as chrono from "chrono-node";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { Compartment } from "@codemirror/state";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import { Calendar } from "@/components/ui/calendar";
import { useUIState } from "@/lib/ui-state-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import type { EditorProps } from ".";

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

/**
 * Parse-as-you-type state. Owns `draft` (raw text), `resolved` (ISO yyyy-MM-dd
 * or null), and reports intermediate resolved values to `onChange` for
 * autosave. Skips the first render so the initial value isn't re-written.
 */
function useDateParsing(value: unknown, onChange?: (v: unknown) => void) {
  const initial = typeof value === "string" ? value : "";
  const [draft, setDraft] = useState(initial);
  const [resolved, setResolved] = useState<string | null>(initial || null);
  const hasMounted = useRef(false);

  useEffect(() => {
    const parsed = parseNatural(draft);
    setResolved(parsed);
    if (!hasMounted.current) {
      hasMounted.current = true;
      return;
    }
    if (parsed) onChange?.(parsed);
  }, [draft, onChange]);

  return { initial, draft, setDraft, resolved, setResolved };
}

/**
 * Commit/cancel guards + the ref shims CM6's keymap extension consumes.
 * Exposes one-shot `commit` + mode-sensitive `submitRef`/`escapeRef`/
 * `commitResolved`/`handleEscape`. A single `committedRef` ensures the
 * commit/cancel paths are idempotent across CM, Radix Popover, and the
 * manual Escape handler.
 */
function useDateCommitHandlers(
  mode: string,
  resolved: string | null,
  onCommit: (v: unknown) => void,
  onCancel: () => void,
) {
  const committedRef = useRef(false);
  const commit = useCallback(
    (iso: string) => {
      if (committedRef.current) return;
      committedRef.current = true;
      onCommit(iso);
    },
    [onCommit],
  );
  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  const resolvedRef = useRef(resolved);
  resolvedRef.current = resolved;
  const commitRef = useRef(commit);
  commitRef.current = commit;
  const cancelRef = useRef(cancel);
  cancelRef.current = cancel;

  const commitOrCancel = useCallback(() => {
    const r = resolvedRef.current;
    if (r) commitRef.current(r);
    else cancelRef.current();
  }, []);

  // Enter → commit-if-resolved. Escape → same in vim, cancel otherwise.
  const submitRef = useRef<(() => void) | null>(null);
  submitRef.current = commitOrCancel;
  const escapeRef = useRef<(() => void) | null>(null);
  escapeRef.current =
    mode === "vim" ? commitOrCancel : () => cancelRef.current();

  const handleEscape = useCallback(() => escapeRef.current?.(), []);

  return {
    commit,
    submitRef,
    escapeRef,
    commitResolved: commitOrCancel,
    handleEscape,
  };
}

/** Map an ISO yyyy-MM-dd string to a local Date for calendar highlighting. */
function parseISOToDate(iso: string | null): Date | undefined {
  if (!iso) return undefined;
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d);
}

/**
 * Composes {@link useDateParsing} + {@link useDateCommitHandlers} and adds
 * the derived CM6 extensions, calendar handler, and highlight date.
 */
function useDateEditorState(params: {
  value: unknown;
  onCommit: EditorProps["onCommit"];
  onCancel: EditorProps["onCancel"];
  onChange: EditorProps["onChange"];
  mode: string;
}) {
  const { value, onCommit, onCancel, onChange, mode } = params;
  const parsing = useDateParsing(value, onChange);
  const handlers = useDateCommitHandlers(mode, parsing.resolved, onCommit, onCancel);

  const handleCalendarSelect = useCallback(
    (day: Date | undefined) => {
      if (!day) return;
      const iso = toISO(day);
      parsing.setDraft(iso);
      parsing.setResolved(iso);
      handlers.commit(iso);
    },
    [handlers, parsing],
  );

  const selectedDate = useMemo(
    () => parseISOToDate(parsing.resolved),
    [parsing.resolved],
  );

  const keymapCompartment = useRef(new Compartment());
  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: handlers.submitRef,
        onCancelRef: handlers.escapeRef,
        singleLine: true,
      }),
    ],
    [mode, handlers.submitRef, handlers.escapeRef],
  );

  return {
    initial: parsing.initial,
    draft: parsing.draft,
    setDraft: parsing.setDraft,
    resolved: parsing.resolved,
    selectedDate,
    extensions,
    handleCalendarSelect,
    handleEscape: handlers.handleEscape,
    commitResolved: handlers.commitResolved,
  };
}

/**
 * PopoverContent body: CM6 natural-language input, parse feedback, calendar.
 * Escape must run through the keymap-aware cancel path (CUA/emacs cancel,
 * vim commit-if-resolved) BEFORE Radix's DismissableLayer closes the
 * popover — otherwise `onOpenChange(false)` fires `commitResolved()` first
 * and saves unconditionally.
 */
function DateEditorContent(props: {
  editorRef: React.Ref<ReactCodeMirrorRef>;
  draft: string;
  setDraft: (v: string) => void;
  extensions: ReturnType<typeof useDateEditorState>["extensions"];
  resolved: string | null;
  selectedDate: Date | undefined;
  onCalendarSelect: (d: Date | undefined) => void;
  onEscape: () => void;
  onAfterEscape: () => void;
}) {
  return (
    <PopoverContent
      align="start"
      className="w-auto p-0"
      onEscapeKeyDown={(e) => {
        e.preventDefault();
        props.onEscape();
        props.onAfterEscape();
      }}
    >
      <div className="p-3 pb-0 space-y-1">
        <CodeMirror
          ref={props.editorRef}
          autoFocus
          value={props.draft}
          onChange={props.setDraft}
          extensions={props.extensions}
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
        {props.draft && (
          <div className="text-xs px-1">
            {props.resolved ? (
              <span className="text-muted-foreground">&rarr; {props.resolved}</span>
            ) : (
              <span className="text-destructive">Could not parse date</span>
            )}
          </div>
        )}
      </div>
      <Calendar
        mode="single"
        selected={props.selectedDate}
        onSelect={props.onCalendarSelect}
        defaultMonth={props.selectedDate}
      />
    </PopoverContent>
  );
}

/** Date editor — CM6 natural language input + calendar picker in a popover. */
export function DateEditor({ value, onCommit, onCancel, onChange }: EditorProps) {
  const [open, setOpen] = useState(true);
  const { keymap_mode: mode } = useUIState();
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const state = useDateEditorState({ value, onCommit, onCancel, onChange, mode });
  const display = state.resolved ?? state.initial;

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        // Click-outside close: commit the currently-resolved date. The
        // Escape path runs cancel/commit first and sets committedRef, so
        // this short-circuits there.
        if (!next) state.commitResolved();
      }}
    >
      <PopoverTrigger asChild>
        <div className="cursor-pointer">
          {display ? (
            <span className="text-sm tabular-nums">{display}</span>
          ) : (
            <span className="text-muted-foreground/50">-</span>
          )}
        </div>
      </PopoverTrigger>
      <DateEditorContent
        editorRef={editorRef}
        draft={state.draft}
        setDraft={state.setDraft}
        extensions={state.extensions}
        resolved={state.resolved}
        selectedDate={state.selectedDate}
        onCalendarSelect={state.handleCalendarSelect}
        onEscape={state.handleEscape}
        onAfterEscape={() => setOpen(false)}
      />
    </Popover>
  );
}
