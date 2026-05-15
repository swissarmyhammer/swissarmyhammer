import type { Entity, FieldDef } from "@/types/kanban";

/** A single group bucket containing tasks that share the same field value. */
export interface GroupBucket {
  /** The raw group value (string). Empty string for ungrouped. */
  value: string;
  /** Human-readable label for the group header. */
  label: string;
  /** Task entities belonging to this group, preserving their input order. */
  tasks: Entity[];
}

/**
 * Compute group buckets from an array of tasks based on a grouping field.
 *
 * - Single-value fields: each task appears in exactly one group.
 * - Multi-value fields (arrays): a task appears in every group matching its values.
 * - Tasks with null, undefined, or empty values go into an "(ungrouped)" bucket.
 * - Groups are sorted alphabetically by value, with "(ungrouped)" always last.
 * - Task order within each group is preserved from the input array.
 *
 * @param tasks - The entities to group.
 * @param groupField - The field name to group by (key into task.fields).
 * @param _fieldDefs - Field definitions (reserved for future kind-based detection).
 * @returns An ordered array of GroupBucket objects.
 */
export function computeGroups(
  tasks: Entity[],
  groupField: string,
  _fieldDefs: FieldDef[],
): GroupBucket[] {
  if (tasks.length === 0) return [];

  const bucketMap = new Map<string, Entity[]>();

  for (const task of tasks) {
    const raw = task.fields[groupField];
    const values = resolveValues(raw);

    for (const v of values) {
      let bucket = bucketMap.get(v);
      if (!bucket) {
        bucket = [];
        bucketMap.set(v, bucket);
      }
      bucket.push(task);
    }
  }

  // Sort keys alphabetically, but push "" (ungrouped) to the end.
  const keys = [...bucketMap.keys()].sort((a, b) => {
    if (a === "") return 1;
    if (b === "") return -1;
    return a.localeCompare(b);
  });

  return keys.map((key) => ({
    value: key,
    label: key === "" ? "(ungrouped)" : key,
    tasks: bucketMap.get(key)!,
  }));
}

/**
 * Resolve a raw field value into an array of group keys.
 *
 * Arrays are expanded so the task joins each value's group.
 * Null, undefined, empty strings, and empty arrays map to [""] (ungrouped).
 * Scalar values map to a single-element array.
 */
function resolveValues(raw: unknown): string[] {
  if (raw == null) return [""];

  if (Array.isArray(raw)) {
    const strings = raw.filter((v) => typeof v === "string" && v !== "");
    return strings.length > 0 ? (strings as string[]) : [""];
  }

  if (typeof raw === "string") {
    return raw === "" ? [""] : [raw];
  }

  // Coerce other types to string.
  return [String(raw)];
}
