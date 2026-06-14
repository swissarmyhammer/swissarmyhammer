/**
 * Static guard: no React production path may still depend on the legacy Tauri
 * change events. The webview is now a pure MCP client — entity/board/attachment
 * change events flow through `notifications/store/changed` (and the undo/UI
 * planes), never through `listen("entity-field-changed" | "board-changed" | …)`.
 *
 * This walks the UI `src/` tree (excluding test files, which may document the
 * old contract in standalone harnesses) and fails if any production module
 * still registers a Tauri listener for one of the migrated change events.
 */
import { describe, it, expect } from "vitest";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const SRC_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

/** The legacy change events the UI must no longer `listen()` for. */
const FORBIDDEN_EVENTS = [
  "entity-field-changed",
  "entity-created",
  "entity-removed",
  "board-changed",
  "attachment-changed",
];

/**
 * Matches `listen("<event>"` / `listen<...>("<event>"` for any forbidden event,
 * with either quote style. The `[^)]*` after `listen` tolerates a generic type
 * arg before the parenthesis.
 */
const FORBIDDEN_LISTEN = new RegExp(
  `listen\\s*(?:<[^>]*>)?\\s*\\(\\s*["'](?:${FORBIDDEN_EVENTS.join("|")})["']`,
);

/** Recursively collect non-test `.ts` / `.tsx` source files under `dir`. */
function collectSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...collectSourceFiles(full));
      continue;
    }
    if (!/\.(ts|tsx)$/.test(entry)) continue;
    if (/\.test\.(ts|tsx)$/.test(entry)) continue; // test harnesses excluded
    if (/\.d\.ts$/.test(entry)) continue;
    out.push(full);
  }
  return out;
}

describe("no Tauri change-event listeners in production source", () => {
  it("no production module calls listen() for a migrated change event", () => {
    const offenders: string[] = [];
    for (const file of collectSourceFiles(SRC_ROOT)) {
      const text = readFileSync(file, "utf8");
      if (FORBIDDEN_LISTEN.test(text)) {
        offenders.push(file.slice(SRC_ROOT.length + 1));
      }
    }
    expect(offenders).toEqual([]);
  });
});
