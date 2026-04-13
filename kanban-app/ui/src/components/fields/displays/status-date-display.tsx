import {
  AlertTriangle,
  CheckCircle,
  Clock,
  Play,
  PlusCircle,
  type LucideIcon,
} from "lucide-react";
import type { DisplayProps } from "./text-display";

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
  completed: { Icon: CheckCircle, label: "Completed", phrasePrefix: "Completed" },
  overdue: { Icon: AlertTriangle, label: "Overdue", phrasePrefix: "Overdue by" },
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

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;
const MONTH = 30 * DAY;
const YEAR = 365 * DAY;

/**
 * Format the absolute millisecond delta between two instants as an
 * English phrase like `"3 days"` or `"2 weeks"`.
 *
 * Caller decides whether to prepend "in", append "ago", etc. — this helper
 * only produces the magnitude phrase so both "Scheduled in 3 days" and
 * "Created 3 days ago" compose cleanly.
 */
function formatDurationMagnitude(deltaMs: number): string {
  const abs = Math.abs(deltaMs);
  if (abs < MINUTE) return "just now";
  if (abs < HOUR) {
    const n = Math.round(abs / MINUTE);
    return `${n} minute${n === 1 ? "" : "s"}`;
  }
  if (abs < DAY) {
    const n = Math.round(abs / HOUR);
    return `${n} hour${n === 1 ? "" : "s"}`;
  }
  if (abs < WEEK) {
    const n = Math.round(abs / DAY);
    return `${n} day${n === 1 ? "" : "s"}`;
  }
  if (abs < MONTH) {
    const n = Math.round(abs / WEEK);
    return `${n} week${n === 1 ? "" : "s"}`;
  }
  if (abs < YEAR) {
    const n = Math.round(abs / MONTH);
    return `${n} month${n === 1 ? "" : "s"}`;
  }
  const n = Math.round(abs / YEAR);
  return `${n} year${n === 1 ? "" : "s"}`;
}

/**
 * Compose the full sentence shown to the user, e.g. `"Completed 2 days ago"`
 * or `"Scheduled in 3 weeks"`. For past magnitudes the `ago` suffix is
 * appended; for future ones the `in ` prefix is added to the magnitude.
 *
 * The `overdue` kind uses `phrasePrefix: "Overdue by"` so the resulting
 * sentence reads `"Overdue by 5 days"` without any directional word.
 */
function composeStatusPhrase(
  kind: StatusKind,
  timestampDate: Date,
  now: Date,
): string {
  const { phrasePrefix } = KIND_DESCRIPTORS[kind];
  const deltaMs = timestampDate.getTime() - now.getTime();
  const magnitude = formatDurationMagnitude(deltaMs);

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
 * Smart status-date display — renders the single most salient date from the
 * task's lifecycle, tagged by `kind` so an appropriate icon + phrasing is
 * chosen. Returns null for any input that is not a well-formed tagged value
 * so the inspector row (and compact card cell) collapse away.
 *
 * - `compact`: icon + short phrase, truncates.
 * - `full`: icon + phrase with the absolute ISO timestamp exposed via a
 *   native `title` tooltip.
 */
export function StatusDateDisplay({ value, mode }: DisplayProps) {
  const parsed = parseStatusDateValue(value);
  if (!parsed) return null;

  const descriptor = KIND_DESCRIPTORS[parsed.kind];
  const timestampDate = parseDateOrDatetime(parsed.timestamp);
  const phrase =
    timestampDate == null
      ? descriptor.label
      : composeStatusPhrase(parsed.kind, timestampDate, new Date());

  const { Icon } = descriptor;

  if (mode === "compact") {
    return (
      <span
        className="inline-flex items-center gap-1 text-xs text-muted-foreground truncate"
        title={parsed.timestamp}
      >
        <Icon className="w-3 h-3 shrink-0" aria-hidden="true" />
        <span className="truncate">{phrase}</span>
      </span>
    );
  }

  return (
    <span
      className="inline-flex items-center gap-1.5 text-sm"
      title={parsed.timestamp}
    >
      <Icon className="w-4 h-4 shrink-0" aria-hidden="true" />
      <span>{phrase}</span>
    </span>
  );
}
