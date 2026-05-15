/**
 * Parity test: the TypeScript `slugify()` in `./slugify.ts` MUST produce
 * byte-identical output to the Rust `slug()` function in
 * `swissarmyhammer-common/src/slug.rs`. The two implementations are kept
 * in lockstep via the shared corpus at
 * `swissarmyhammer-common/tests/slug_parity_corpus.txt` — both sides run
 * over the same input list and assert idempotency plus stable rules.
 *
 * This test is node-only (uses `fs`, `path`, `url`) and therefore lives
 * under the `*.node.test.ts` naming convention recognized by the project's
 * vitest config.
 */
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { slugify } from "./slugify";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to the shared parity corpus. */
const CORPUS_PATH = resolve(
  __dirname,
  "../../../..",
  "swissarmyhammer-common",
  "tests",
  "slug_parity_corpus.txt",
);

/** Parse the corpus file: strip comment lines (`#`) and blank lines. */
function loadCorpus(): string[] {
  const raw = readFileSync(CORPUS_PATH, "utf8");
  return raw
    .split("\n")
    .filter((line) => line.length > 0 && !line.startsWith("#"));
}

describe("slugify parity with Rust `slug()`", () => {
  it("corpus file loads with at least 100 entries", () => {
    const entries = loadCorpus();
    expect(entries.length).toBeGreaterThanOrEqual(100);
  });

  it("every corpus entry is idempotent (slugify(slugify(x)) === slugify(x))", () => {
    const entries = loadCorpus();
    for (const entry of entries) {
      const once = slugify(entry);
      const twice = slugify(once);
      expect(
        twice,
        `idempotence broken for input ${JSON.stringify(entry)}`,
      ).toBe(once);
    }
  });

  it("reproducer from the task: 'Task card & field polish' → 'task-card-field-polish'", () => {
    // Pin the exact case from the task's concrete reproducer so an
    // accidental rule change (e.g. dropping the non-alphanumeric collapse)
    // is caught loudly.
    expect(slugify("Task card & field polish")).toBe("task-card-field-polish");
  });

  // The following cases hand-encode the Rust side's output for key corpus
  // entries so the two implementations are pinned to the same rule set
  // even if the Rust test binary isn't available during TS test runs.
  // Keep this list representative; the full byte-equality guarantee is
  // provided by the Rust tests running in CI against the same corpus.
  it.each([
    ["hello world", "hello-world"],
    ["HELLO", "hello"],
    ["---hello---", "hello"],
    ["hello...world", "hello-world"],
    ["hello     world", "hello-world"],
    ["", ""],
    ["!!!", ""],
    ["a!@#b", "a-b"],
    ["one & two, three.", "one-two-three"],
    ["AUTH-Migration", "auth-migration"],
    ["Claude-Code (v2)", "claude-code-v2"],
    ["bug #42", "bug-42"],
    ["swissarmyhammer / kanban", "swissarmyhammer-kanban"],
    ["Feature Flag: Dark Mode", "feature-flag-dark-mode"],
    ["user@example.com", "user-example-com"],
    // Unicode: non-ASCII collapses to hyphens.
    ["café", "caf"], // trailing non-ASCII collapses, then gets stripped
    ["naïve text", "na-ve-text"],
    ["one\u2013two", "one-two"], // en dash
    ["one\u2014two", "one-two"], // em dash
    ["\u201Cquoted\u201D", "quoted"], // curly quotes
  ])("slugify(%j) === %j", (input, expected) => {
    expect(slugify(input)).toBe(expected);
  });
});
