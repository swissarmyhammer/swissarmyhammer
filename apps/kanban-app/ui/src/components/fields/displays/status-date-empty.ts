/**
 * Emptiness predicate for the `status_date` tagged value.
 *
 * Mirrors the sibling {@link file://./progress-empty.ts} convention: a tiny,
 * display-adjacent module that encodes "what makes this computed field's row
 * not worth showing". The Field registry calls this via the `isEmpty` option
 * on {@link registerDisplay} so the inspector can suppress the surrounding
 * `FieldRow` wrapper (icon, tooltip, flex gap) when the display itself would
 * render nothing.
 *
 * The backend `derive-status-date` derivation returns `Value::Null` when the
 * task has no completed / overdue / started / scheduled / created anchor; in
 * that case the UI must collapse the row entirely. Without this predicate the
 * inspector would still show the `target` icon + empty content slot — the
 * exact regression tracked by card 01KP23V1 for the `progress` display.
 *
 * A value is "empty" when any of:
 * - it is `null` / `undefined`
 * - it is not a plain object (numbers, strings, booleans, arrays)
 * - the object is missing `kind` or `timestamp`
 * - `kind` is not one of completed / overdue / started / scheduled / created
 * - `timestamp` is not a parseable RFC 3339 datetime or bare `YYYY-MM-DD`
 *
 * Implementation reuses the exact narrowing functions the display uses so
 * "renders something" and "is not empty" stay perfectly in sync.
 */
import {
  parseStatusDateValue,
  parseDateOrDatetime,
} from "./status-date-display";

/**
 * Return `true` when the given status_date payload should suppress its row.
 *
 * @param value - Raw field value as delivered by the backend (JSON-ish).
 * @returns `true` when the display would render nothing useful.
 */
export function isStatusDateEmpty(value: unknown): boolean {
  const parsed = parseStatusDateValue(value);
  if (parsed === null) return true;
  return parseDateOrDatetime(parsed.timestamp) === null;
}
