/**
 * Architectural guards for the focus-decoration subsystem.
 *
 * Rust's `SpatialState::focus_by_window` is the single source of truth for
 * focus state. The React side observes it through `useFocusClaim` and
 * renders the visible decoration from React state — never from a CSS rule
 * that reads a DOM `data-*` attribute. The data attributes that ride along
 * (`data-moniker`, `data-focused`, `data-cell-cursor`) are output-only
 * debugging / e2e selectors; nothing in CSS or React reads them back as
 * state.
 *
 * These tests grep the shipped sources to keep the architecture honest:
 *
 *   1. No CSS file contains a `[data-focused]`, `[data-cell-cursor]`,
 *      `[data-moniker]`, or `[data-zone-moniker]` selector. If a CSS rule
 *      reads any of those attributes, it is a covert state channel — focus
 *      state is being smuggled out of React through the DOM and back into
 *      CSS, defeating the single-source-of-truth contract.
 *
 *   2. Exactly one component renders the visible focus indicator. The
 *      indicator is the `<FocusIndicator>` JSX element rendered from
 *      React state; sprinkling it across multiple components reintroduces
 *      the duplicate-decorator bugs the consolidation pass was designed to
 *      eliminate. Tests and the indicator's own definition file are
 *      excluded from the count — only production callers count.
 *
 * Node-only because they read source files from disk; lives under the
 * `*.node.test.ts` suffix recognized by `vite.config.ts`.
 */
import { describe, it, expect } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Absolute path to the `kanban-app/ui/src` root that contains shipped UI code. */
const SRC_ROOT = resolve(__dirname, "..");

/**
 * Walk a directory tree, returning every file path whose name matches one of
 * the given extensions. Skips `node_modules` and any directory whose name
 * starts with a dot (e.g. `__snapshots__` is fine; `.cache` is not).
 *
 * @param root - Absolute directory to walk.
 * @param exts - File extensions to include (with leading dot, e.g. `.css`).
 * @returns Sorted list of absolute file paths.
 */
function walkSources(root: string, exts: readonly string[]): string[] {
  const found: string[] = [];
  const visit = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      if (entry.startsWith(".") || entry === "node_modules") continue;
      const full = resolve(dir, entry);
      const stat = statSync(full);
      if (stat.isDirectory()) {
        visit(full);
      } else if (exts.some((ext) => entry.endsWith(ext))) {
        found.push(full);
      }
    }
  };
  visit(root);
  found.sort();
  return found;
}

describe("focus-decoration architecture", () => {
  it("no CSS file selects on [data-focused], [data-cell-cursor], [data-moniker], or [data-zone-moniker]", () => {
    const cssFiles = walkSources(SRC_ROOT, [".css"]);
    expect(cssFiles.length).toBeGreaterThan(0); // sanity — there must be at least index.css

    // Banned attribute-selector patterns. A selector reading any of these
    // attributes is a covert state channel: focus state goes React → DOM
    // attr → CSS, when it should be React → className.
    const banned = [
      /\[data-focused\b/,
      /\[data-cell-cursor\b/,
      /\[data-moniker\b/,
      /\[data-zone-moniker\b/,
    ];

    // Strip CSS comments so the banned-pattern scan ignores prose
    // explaining *why* a rule was removed. Without this step a comment
    // like "no CSS rule reads [data-focused]" inside the file would itself
    // trip the guard.
    function stripCssComments(src: string): string {
      // Replace each comment with the same number of newlines so line
      // numbers in error messages line up with the original file.
      return src.replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "));
    }

    const offenders: { file: string; line: number; text: string }[] = [];
    for (const file of cssFiles) {
      const original = readFileSync(file, "utf-8");
      const stripped = stripCssComments(original);
      const originalLines = original.split("\n");
      const strippedLines = stripped.split("\n");
      strippedLines.forEach((text, i) => {
        for (const pattern of banned) {
          if (pattern.test(text)) {
            offenders.push({
              file: relative(SRC_ROOT, file),
              line: i + 1,
              text: originalLines[i].trim(),
            });
          }
        }
      });
    }

    if (offenders.length > 0) {
      const msg = offenders
        .map((o) => `  ${o.file}:${o.line}  ${o.text}`)
        .join("\n");
      throw new Error(
        `CSS rule reads a focus-state DOM attribute — focus state must come from React, not the DOM:\n${msg}`,
      );
    }
  });

  it("the only component that renders the focus visual is <FocusIndicator>", () => {
    // The canonical visible focus decoration lives in `<FocusIndicator>`
    // (`components/focus-indicator.tsx`). Production callers compose the
    // indicator from the spatial primitives (`<Focusable>` and
    // `<FocusZone>`); any other production source rendering its own focus
    // bar / focus highlight is a duplicate decorator and a regression.
    //
    // The check has two parts:
    //
    //   1. Only the spatial primitives are allowed to render
    //      `<FocusIndicator>`. Any other call site reintroduces the
    //      multi-decorator antipattern.
    //
    //   2. Both spatial primitives MUST render `<FocusIndicator>` —
    //      otherwise their visual contracts diverge and a regression that
    //      deletes the bar from one branch ships silently.
    const tsxFiles = walkSources(SRC_ROOT, [".tsx", ".ts"]);
    const allowedCallers = new Set([
      "components/focusable.tsx",
      "components/focus-zone.tsx",
    ]);

    // Strip line and block comments so a doc reference like
    // `// renders <FocusIndicator>` doesn't trip the JSX detector. The
    // replacement preserves newline counts so error messages line up.
    function stripJsComments(src: string): string {
      return src
        .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
        .replace(/(^|[^:\\])\/\/[^\n]*/g, (_m, prefix) => prefix);
    }

    const offenders: string[] = [];
    for (const file of tsxFiles) {
      const rel = relative(SRC_ROOT, file);
      if (rel.endsWith(".test.ts") || rel.endsWith(".test.tsx")) continue;
      if (rel.endsWith(".node.test.ts")) continue;
      if (rel.includes("__screenshots__")) continue;
      if (rel.endsWith("focus-indicator.tsx")) continue;

      const stripped = stripJsComments(readFileSync(file, "utf-8"));
      if (/<FocusIndicator[\s/>]/.test(stripped) && !allowedCallers.has(rel)) {
        offenders.push(rel);
      }
    }
    if (offenders.length > 0) {
      throw new Error(
        `Only the spatial primitives may render <FocusIndicator>; found extra callers:\n${offenders.map((o) => `  ${o}`).join("\n")}`,
      );
    }

    // Both spatial primitives must compose the indicator. Without this
    // assertion a future edit could delete the indicator from `<FocusZone>`
    // and the suite would still be green except for one inspector test.
    for (const requiredCaller of allowedCallers) {
      const fullPath = resolve(SRC_ROOT, requiredCaller);
      const stripped = stripJsComments(readFileSync(fullPath, "utf-8"));
      if (!/<FocusIndicator[\s/>]/.test(stripped)) {
        throw new Error(
          `Spatial primitive ${requiredCaller} must render <FocusIndicator>; the visual decoration belongs in exactly this place.`,
        );
      }
    }
  });

  it("FocusHighlight is fully removed", () => {
    // The legacy `<FocusHighlight>` decorator was a duplicate path for
    // emitting `data-focused`. Replaced by the spatial primitive's own
    // `useFocusClaim`-driven rendering. Any remaining reference in the
    // shipped sources is a regression: it either resurrects the duplicate
    // decorator or leaves a stale name pointing at deleted code.
    const tsxFiles = walkSources(SRC_ROOT, [".tsx", ".ts"]);
    const offenders: string[] = [];
    for (const file of tsxFiles) {
      const rel = relative(SRC_ROOT, file);
      if (rel.endsWith(".node.test.ts")) continue; // this file references it by name in comments
      const src = readFileSync(file, "utf-8");
      if (/\bFocusHighlight\b/.test(src)) {
        offenders.push(rel);
      }
    }
    if (offenders.length > 0) {
      throw new Error(
        `FocusHighlight is gone — references must not remain:\n${offenders.map((o) => `  ${o}`).join("\n")}`,
      );
    }
  });
});
