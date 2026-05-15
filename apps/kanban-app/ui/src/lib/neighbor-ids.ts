/**
 * Determine the neighbor task IDs for the task at `index` within `ids`.
 * `selfId` is excluded so that the task being moved is never its own neighbor.
 *
 * The backend uses these to compute the ordinal via `compute_ordinal_for_neighbors`.
 */
export function neighborIds(
  ids: string[],
  index: number,
  selfId: string,
): { beforeId: string | null; afterId: string | null } {
  const filtered = ids.filter((id) => id !== selfId);
  // After filtering out selfId, the task's position is the number of items before `index`
  // that aren't selfId.
  const insertPos = ids.slice(0, index).filter((id) => id !== selfId).length;
  const beforeId = insertPos > 0 ? filtered[insertPos - 1] : null;
  const afterId = insertPos < filtered.length ? filtered[insertPos] : null;
  return { beforeId, afterId };
}
