/**
 * Canonical actor color palette and deterministic hash.
 *
 * These values are copied verbatim from the Rust implementation in
 * `kanban-app/src/state.rs` (the source of truth).
 * Any changes must be made in Rust first, then mirrored here.
 */

/** 15-color palette matching `ACTOR_COLORS` in state.rs. */
export const ACTOR_COLORS: readonly string[] = [
  "e53e3e", "dd6b20", "d69e2e", "38a169", "319795",
  "3182ce", "5a67d8", "805ad5", "d53f8c", "2b6cb0",
  "c05621", "2f855a", "2c7a7b", "6b46c1", "b83280",
];

/**
 * Derive a deterministic hex color from a string using djb2 hash.
 *
 * Matches `deterministic_color()` in state.rs exactly:
 *   hash = bytes.fold(5381, |h, b| h.wrapping_mul(33).wrapping_add(b))
 *   palette[hash % len]
 *
 * We use BigInt to faithfully reproduce Rust's u64 wrapping arithmetic.
 */
export function deriveActorColor(id: string): string {
  const MASK = (1n << 64n) - 1n; // u64 max
  let hash = 5381n;
  for (let i = 0; i < id.length; i++) {
    hash = ((hash * 33n) + BigInt(id.charCodeAt(i))) & MASK;
  }
  return ACTOR_COLORS[Number(hash % BigInt(ACTOR_COLORS.length))];
}
