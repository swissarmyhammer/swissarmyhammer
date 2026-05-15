/**
 * Date editor — natural-language date input + calendar picker in a popover.
 *
 * Layout (FilterEditor-inspired): an icon-left + input-right row, borderless,
 * with the shadcn `Calendar` below.
 *
 * Composition:
 *
 *   - `parseNatural` turns the buffer into a `YYYY-MM-DD` (or `null`).
 *   - `useDebouncedTimer` schedules an autosave whenever the parse succeeds;
 *     Enter flushes synchronously, Escape (CUA) cancels.
 *   - `TextEditor singleLine autoFocus` owns the CM6 buffer; this file owns
 *     the commit/cancel policy via {@link buildSubmitCancelExtensions}.
 *   - The shadcn `Calendar` below the input commits a click-picked date
 *     immediately, bypassing the debounce.
 *
 * The pure helpers `parseNatural`, `toISO`, and `parseISOToDate` are exported
 * implicitly via the parse pipeline and are intentionally untouched by this
 * refactor.
 */

import { useCallback, useMemo, useRef, useState } from "react";
import * as chrono from "chrono-node";
import { Calendar as CalendarIcon, type LucideIcon } from "lucide-react";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import { Calendar } from "@/components/ui/calendar";
import { useUIState } from "@/lib/ui-state-context";
import { TextEditor } from "@/components/fields/text-editor";
import { fieldIcon } from "@/components/fields/field-icon";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useDebouncedTimer } from "@/lib/use-debounced-timer";
import type { EditorProps } from ".";

/** Format a Date as YYYY-MM-DD. */
function toISO(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** Try to parse text as a date via chrono-node, return YYYY-MM-DD or null. */
function parseNatural(text: string): string | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  // Try exact YYYY-MM-DD first.
  if (/^\d{4}-\d{2}-\d{2}$/.test(trimmed)) return trimmed;
  const results = chrono.parse(trimmed);
  if (results.length > 0) return toISO(results[0].start.date());
  return null;
}

/** Map an ISO yyyy-MM-dd string to a local Date for calendar highlighting. */
function parseISOToDate(iso: string | null): Date | undefined {
  if (!iso) return undefined;
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d);
}

/** Fallback CM6 placeholder when a field doesn't declare a description. */
const DEFAULT_CM_PLACEHOLDER = "Type a date... (e.g. tomorrow, next friday)";

/** Debounce delay for date autosave, in milliseconds. */
const AUTOSAVE_DELAY_MS = 400;

/**
 * Hook bundling the editor's draft + resolved state and the commit policy.
 *
 * Mirrors the FilterEditor pattern — typing schedules a debounced commit,
 * Enter flushes, Escape cancels (CUA) or commits-if-resolved (vim). A single
 * `committedRef` makes the commit/cancel paths idempotent across the
 * keymap, the manual escape path, and the popover's click-outside close.
 */
function useDateEditorState(params: {
  initialValue: string;
  onCommit: (v: unknown) => void;
  onCancel: () => void;
  mode: string;
}) {
  const { initialValue, onCommit, onCancel, mode } = params;

  const [draft, setDraft] = useState(initialValue);
  const [resolved, setResolved] = useState<string | null>(initialValue || null);

  const { schedule, cancel: cancelDebounce, flush } = useDebouncedTimer();

  const committedRef = useRef(false);
  const onCommitRef = useRef(onCommit);
  onCommitRef.current = onCommit;
  const onCancelRef = useRef(onCancel);
  onCancelRef.current = onCancel;

  const commit = useCallback((iso: string) => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCommitRef.current(iso);
  }, []);

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancelRef.current();
  }, []);

  // Latest-resolved snapshot for ref-driven keymap callbacks.
  const resolvedRef = useRef(resolved);
  resolvedRef.current = resolved;

  /**
   * Typing path: parse, mirror into `resolved`, schedule a debounced commit.
   * Re-typing replaces the pending callback (the timer hook only holds one).
   */
  const handleDraftChange = useCallback(
    (text: string) => {
      setDraft(text);
      const parsed = parseNatural(text);
      setResolved(parsed);
      if (parsed) {
        schedule(() => commit(parsed), AUTOSAVE_DELAY_MS);
      } else {
        cancelDebounce();
      }
    },
    [schedule, cancelDebounce, commit],
  );

  /** Enter / submit path — flush the pending debounce immediately. */
  const handleSubmit = useCallback(() => {
    const r = resolvedRef.current;
    if (r) {
      // If a debounced commit is in flight, flush replays the same callback;
      // committedRef ensures we only commit once. If no timer is pending
      // (e.g. user pressed Enter the instant after a paste before the
      // change handler scheduled), commit directly.
      flush();
      commit(r);
    } else {
      cancelDebounce();
      cancel();
    }
  }, [flush, cancelDebounce, commit, cancel]);

  /**
   * Escape path — CUA cancels (drops the in-flight change), vim commits-if-
   * resolved (preserves the established date-editor semantics).
   */
  const handleEscape = useCallback(() => {
    if (mode === "vim") {
      handleSubmit();
    } else {
      cancelDebounce();
      cancel();
    }
  }, [mode, handleSubmit, cancelDebounce, cancel]);

  /** Calendar click — bypass debounce, commit immediately. */
  const handleCalendarSelect = useCallback(
    (day: Date | undefined) => {
      if (!day) return;
      const iso = toISO(day);
      setDraft(iso);
      setResolved(iso);
      cancelDebounce();
      commit(iso);
    },
    [cancelDebounce, commit],
  );

  /**
   * Popover close path. The keymap-driven cancel/commit runs first and sets
   * `committedRef`, so this short-circuits there. Click-outside without a
   * prior key event commits the resolved date (or cancels if none).
   */
  const handlePopoverClose = useCallback(() => {
    const r = resolvedRef.current;
    if (r) {
      flush();
      commit(r);
    } else {
      cancelDebounce();
      cancel();
    }
  }, [flush, cancelDebounce, commit, cancel]);

  return {
    draft,
    resolved,
    selectedDate: parseISOToDate(resolved),
    handleDraftChange,
    handleSubmit,
    handleEscape,
    handleCalendarSelect,
    handlePopoverClose,
  };
}

/**
 * Build the CM6 extensions wired to the editor's submit/escape callbacks.
 *
 * Stable refs are required because `buildSubmitCancelExtensions` reads
 * callbacks via mutable refs at event time — passing the React closure
 * directly would capture a stale callback.
 */
function useSubmitCancelExtensions(
  mode: string,
  handleSubmit: () => void,
  handleEscape: () => void,
) {
  const submitRef = useRef<(() => void) | null>(handleSubmit);
  submitRef.current = handleSubmit;
  const escapeRef = useRef<(() => void) | null>(handleEscape);
  escapeRef.current = handleEscape;

  return useMemo(
    () =>
      buildSubmitCancelExtensions({
        mode,
        onSubmitRef: submitRef,
        onCancelRef: escapeRef,
        singleLine: true,
      }),
    [mode],
  );
}

/**
 * Resolve the lucide icon component for the field's `icon` property,
 * falling back to a calendar glyph. Exported indirectly so the inspector and
 * other date-shaped editors can share the resolver via {@link fieldIcon}.
 */
function useDateFieldIcon(field: EditorProps["field"]): LucideIcon {
  return useMemo(() => fieldIcon(field) ?? CalendarIcon, [field]);
}

/** Icon + TextEditor row — the visual shape of an open date editor. */
function DateEditorInputRow(props: {
  Icon: LucideIcon;
  draft: string;
  placeholder: string;
  extensions: ReturnType<typeof useSubmitCancelExtensions>;
  resolved: string | null;
  onChange: (text: string) => void;
}) {
  return (
    <>
      <div className="flex items-center gap-2 px-3 pt-3">
        <props.Icon
          data-testid="date-editor-icon"
          className="h-4 w-4 text-muted-foreground shrink-0"
        />
        <div className="flex-1 min-w-0">
          <TextEditor
            singleLine
            autoFocus
            value={props.draft}
            placeholder={props.placeholder}
            onChange={props.onChange}
            extensions={props.extensions}
          />
        </div>
      </div>
      {props.draft && (
        <div className="text-xs px-3 pt-1">
          {props.resolved ? (
            <span className="text-muted-foreground">
              &rarr; {props.resolved}
            </span>
          ) : (
            <span className="text-destructive">Could not parse date</span>
          )}
        </div>
      )}
    </>
  );
}

/**
 * Date editor — natural-language input + calendar picker in a popover.
 *
 * Uses `field.description` as the trigger's empty-state label and the
 * TextEditor placeholder, falling back to `-` / {@link DEFAULT_CM_PLACEHOLDER}
 * when the schema doesn't declare a description.
 */
export function DateEditor({
  field,
  value,
  onCommit,
  onCancel,
  onChange,
}: EditorProps) {
  const [open, setOpen] = useState(true);
  const { keymap_mode: mode } = useUIState();
  const initial = typeof value === "string" ? value : "";

  // Mirror legacy onChange-on-resolve behaviour for callers that wired
  // intermediate-value autosave outside the popover. The committed value
  // is the same one onCommit receives, so this is a notification echo.
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const handleCommit = useCallback(
    (v: unknown) => {
      onChangeRef.current?.(v);
      onCommit(v);
    },
    [onCommit],
  );

  const state = useDateEditorState({
    initialValue: initial,
    onCommit: handleCommit,
    onCancel,
    mode,
  });

  const extensions = useSubmitCancelExtensions(
    mode,
    state.handleSubmit,
    state.handleEscape,
  );

  const Icon = useDateFieldIcon(field);
  const display = state.resolved ?? initial;
  const emptyLabel = field.description ?? "-";
  const placeholder = field.description ?? DEFAULT_CM_PLACEHOLDER;

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) state.handlePopoverClose();
      }}
    >
      <PopoverTrigger asChild>
        <div className="cursor-pointer">
          {display ? (
            <span className="text-sm tabular-nums">{display}</span>
          ) : (
            <span className="text-muted-foreground/50">{emptyLabel}</span>
          )}
        </div>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-auto p-0"
        onEscapeKeyDown={(e) => {
          // Run our keymap-aware escape BEFORE Radix's DismissableLayer
          // closes the popover, otherwise `onOpenChange(false)` would fire
          // `handlePopoverClose` first and commit unconditionally.
          e.preventDefault();
          state.handleEscape();
          setOpen(false);
        }}
      >
        <DateEditorInputRow
          Icon={Icon}
          draft={state.draft}
          placeholder={placeholder}
          extensions={extensions}
          resolved={state.resolved}
          onChange={state.handleDraftChange}
        />
        <Calendar
          mode="single"
          selected={state.selectedDate}
          onSelect={state.handleCalendarSelect}
          defaultMonth={state.selectedDate}
        />
      </PopoverContent>
    </Popover>
  );
}
