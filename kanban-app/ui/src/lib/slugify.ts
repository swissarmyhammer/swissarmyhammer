/**
 * Convert a display string to a URL-safe slug for mention matching.
 *
 * - Lowercases
 * - Replaces non-alphanumeric runs with a single hyphen
 * - Strips leading/trailing hyphens
 * - Idempotent: slugify("fix-login-bug") === "fix-login-bug"
 */
export function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}
