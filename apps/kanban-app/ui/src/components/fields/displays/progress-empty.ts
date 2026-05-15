/**
 * Emptiness predicate shared by `ProgressDisplay` and `ProgressRingDisplay`.
 *
 * A progress value is "empty" when the backend has nothing to visualise —
 * either the value is missing / malformed, or `total` is 0. Both the bar and
 * the ring already `return null` in that state; the inspector calls this
 * helper to decide whether to suppress the surrounding `FieldRow` wrapper
 * (icon, tooltip, flex gap) so computed rows don't take up empty space.
 *
 * Accepts both backend shapes:
 * - task progress: `{ total, completed, percent }`
 * - board percent_complete: `{ done, total, percent }`
 *
 * Any non-object, null, or object without a numeric positive `total` counts
 * as empty.
 */
export function isProgressEmpty(value: unknown): boolean {
  if (value == null || typeof value !== "object") return true;
  const obj = value as Record<string, unknown>;
  const total = typeof obj.total === "number" ? obj.total : 0;
  return total === 0;
}
