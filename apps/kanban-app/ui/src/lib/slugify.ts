/**
 * Convert a display string to a URL-safe slug for mention matching.
 *
 * - Lowercases
 * - Replaces non-alphanumeric runs with a single hyphen
 * - Strips leading/trailing hyphens
 * - Idempotent: slugify("fix-login-bug") === "fix-login-bug"
 *
 * ## Parity with Rust
 *
 * This function MUST produce byte-identical output to the canonical
 * Rust `swissarmyhammer_common::slug()` function. The two sides are
 * pinned together by the shared corpus at
 * `swissarmyhammer-common/tests/slug_parity_corpus.txt` — both the Rust
 * unit tests and `slugify.parity.node.test.ts` walk that corpus and
 * assert the output is stable. If you change the algorithm here,
 * change the Rust side in lockstep or the parity test will fail.
 */
export function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
