import {
  AlertTriangle,
  CheckCircle,
  Clock,
  Play,
  PlusCircle,
  type LucideIcon,
} from "lucide-react";
import { DateTime } from "luxon";
import { formatRelativeMagnitude } from "@/lib/format-date";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import { DisplayText, type DisplayProps } from "./text-display";

/**
 * Tagged shape produced by the backend `derive-status-date` derivation.
 *
 * The `kind` identifies which stage of the task lifecycle drove selection
 * (completed / overdue / started / scheduled / created) and `timestamp`
 * is the underlying date/datetime string that was chosen.
 */
export type StatusKind =
  | "completed"
  | "overdue"
  | "started"
  | "scheduled"
  | "created";

/**
 * Tagged payload produced by the backend `derive-status-date` derivation.
 *
 * `kind` identifies which lifecycle stage was selected and `timestamp` is the
 * underlying ISO 8601 date or datetime string chosen for that stage.
 */
export interface StatusDateValue {
  kind: StatusKind;
  timestamp: string;
}

/** All kinds recognised by {@link parseStatusDateValue}, for external reuse. */
export const STATUS_KINDS: readonly StatusKind[] = [
  "completed",
  "overdue",
  "started",
  "scheduled",
  "created",
];

/** Descriptor for a single `kind` — icon + verb used in both display modes. */
interface KindDescriptor {
  Icon: LucideIcon;
  /** Short verb used in compact mode (e.g. "Completed"). */
  label: string;
  /**
   * Phrase prefix used when composing a relative-time sentence
   * (e.g. "Completed 3 days ago", "Overdue by 2 weeks").
   */
  phrasePrefix: string;
}

const KIND_DESCRIPTORS: Record<StatusKind, KindDescriptor> = {
  completed: {
    Icon: CheckCircle,
    label: "Completed",
    phrasePrefix: "Completed",
  },
  overdue: {
    Icon: AlertTriangle,
    label: "Overdue",
    phrasePrefix: "Overdue by",
  },
  started: { Icon: Play, label: "Started", phrasePrefix: "Started" },
  scheduled: { Icon: Clock, label: "Scheduled", phrasePrefix: "Scheduled" },
  created: { Icon: PlusCircle, label: "Created", phrasePrefix: "Created" },
};

/**
 * Narrow an unknown value to a well-formed StatusDateValue, or return null.
 *
 * Rejects:
 * - null / undefined
 * - non-objects and arrays
 * - missing or unknown `kind`
 * - non-string `timestamp`
 *
 * Exported so the emptiness predicate used by the Field registry (see
 * {@link file://./status-date-empty.ts}) can share the exact same narrowing
 * rules — keeping "what counts as a valid status_date" in one place.
 */
export function parseStatusDateValue(value: unknown): StatusDateValue | null {
  if (value == null || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const obj = value as Record<string, unknown>;
  const kind = obj.kind;
  const timestamp = obj.timestamp;
  if (typeof kind !== "string" || typeof timestamp !== "string") {
    return null;
  }
  if (!(kind in KIND_DESCRIPTORS)) {
    return null;
  }
  return { kind: kind as StatusKind, timestamp };
}

/**
 * Parse an RFC 3339 datetime or bare `YYYY-MM-DD` calendar date.
 *
 * Bare dates are interpreted as midnight UTC so they sort sensibly next to
 * RFC 3339 timestamps. Returns a finite `Date` or null if parsing fails.
 *
 * Exported so the emptiness predicate (see {@link file://./status-date-empty.ts})
 * can use the same "parseable timestamp" rule as the display.
 */
export function parseDateOrDatetime(value: string): Date | null {
  // Bare calendar date ("2026-04-10") → treat as midnight UTC.
  if (/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    const d = new Date(`${value}T00:00:00Z`);
    return Number.isNaN(d.getTime()) ? null : d;
  }
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? null : d;
}

/**
 * Compose the full sentence shown to the user, e.g. `"Completed 2 days ago"`
 * or `"Scheduled in 3 weeks"`. For past magnitudes the `ago` suffix is
 * appended; for future ones the `in ` prefix is added to the magnitude.
 *
 * The `overdue` kind uses `phrasePrefix: "Overdue by"` so the resulting
 * sentence reads `"Overdue by 5 days"` without any directional word.
 *
 * Magnitude phrasing (`"2 days"`, `"3 weeks"`, `"just now"`, ...) is produced
 * by the shared {@link formatRelativeMagnitude} helper so the status-date
 * display and {@link ./date-display.tsx DateDisplay} agree on how durations
 * are worded. Direction (past vs future) is determined here from the raw
 * millisecond delta because the magnitude helper is intentionally
 * direction-agnostic.
 */
function composeStatusPhrase(
  kind: StatusKind,
  timestamp: string,
  timestampDate: Date,
  now: Date,
): string {
  const { phrasePrefix } = KIND_DESCRIPTORS[kind];
  const deltaMs = timestampDate.getTime() - now.getTime();
  const magnitude = formatRelativeMagnitude(
    timestamp,
    DateTime.fromJSDate(now),
  );

  if (magnitude === "just now") {
    return `${phrasePrefix} just now`;
  }
  if (kind === "overdue") {
    // "Overdue by 5 days" — prefix already carries the directional word.
    return `${phrasePrefix} ${magnitude}`;
  }
  if (deltaMs > 0) {
    return `${phrasePrefix} in ${magnitude}`;
  }
  return `${phrasePrefix} ${magnitude} ago`;
}

/**
 * Return the kind-specific icon for a status_date value, or null if the
 * value is not a well-formed `{ kind, timestamp }` payload.
 *
 * Registered as the `iconOverride` on the status-date display so the parent
 * layout (inspector row, card field) renders the kind's icon in the tooltip
 * position instead of the static YAML icon. This eliminates the duplicate
 * icon that previously appeared (one from the layout, one inside the display).
 *
 * @param value - Raw field value as delivered by the backend (JSON-ish).
 * @returns The kind-specific LucideIcon, or null for invalid input.
 */
export function statusDateIconOverride(value: unknown): LucideIcon | null {
  const parsed = parseStatusDateValue(value);
  if (!parsed) return null;
  return KIND_DESCRIPTORS[parsed.kind].Icon;
}

/**
 * Return a value-dependent tooltip phrase for a status_date value, or null
 * if the value is not a well-formed `{ kind, timestamp }` payload.
 *
 * Registered as the `tooltipOverride` on the status-date display so the
 * parent layout (inspector row, card field) shows a dynamic phrase like
 * "Completed 3 days ago" instead of the static YAML field description.
 * Falls back to the kind's label when the timestamp is unparseable.
 *
 * @param value - Raw field value as delivered by the backend (JSON-ish).
 * @returns A human-readable status phrase, or null for invalid input.
 */
export function statusDateTooltipOverride(value: unknown): string | null {
  const parsed = parseStatusDateValue(value);
  if (!parsed) return null;
  const descriptor = KIND_DESCRIPTORS[parsed.kind];
  const timestampDate = parseDateOrDatetime(parsed.timestamp);
  if (timestampDate == null) return descriptor.label;
  return composeStatusPhrase(
    parsed.kind,
    parsed.timestamp,
    timestampDate,
    new Date(),
  );
}

/**
 * Smart status-date display — renders the single most salient date from the
 * task's lifecycle, tagged by `kind` so an appropriate phrasing is chosen.
 * Returns null for any input that is not a well-formed tagged value so the
 * inspector row (and compact card cell) collapse away.
 *
 * The kind-specific icon is provided to the parent layout via
 * {@link statusDateIconOverride} registered on the display — the display
 * itself renders only the text phrase, avoiding the duplicate-icon problem.
 *
 * - `compact`: short phrase, truncates.
 * - `full`: phrase with the absolute ISO timestamp exposed via a native
 *   `title` tooltip.
 */
export function StatusDateDisplay({ value, mode }: DisplayProps) {
  const parsed = parseStatusDateValue(value);
  if (!parsed) {
    // Unparseable values render an empty wrapper in compact mode so the
    // row honors the fixed `ROW_HEIGHT` virtualizer contract; full mode
    // collapses away as before.
    return mode === "compact" ? (
      <CompactCellWrapper>{null}</CompactCellWrapper>
    ) : null;
  }

  const descriptor = KIND_DESCRIPTORS[parsed.kind];
  const timestampDate = parseDateOrDatetime(parsed.timestamp);
  const phrase =
    timestampDate == null
      ? descriptor.label
      : composeStatusPhrase(
          parsed.kind,
          parsed.timestamp,
          timestampDate,
          new Date(),
        );

  return <DisplayText text={phrase} mode={mode} title={parsed.timestamp} />;
}
