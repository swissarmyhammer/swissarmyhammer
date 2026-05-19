/**
 * Source-level guard for card `01KR7CDEFWWVF4WH0BCHE8Y21J` (the
 * `nav.focus` choke-point invariant): no production component file
 * outside the allowlist may call `setFocus(<non-null>)` directly.
 * Every focus claim must flow through `nav.focus` — dispatched via
 * `useDispatchCommand("nav.focus")` — so cross-cutting concerns
 * (telemetry, animations, scroll-on-focus) hang off one closure.
 *
 * Runs as a Node-mode unit test (`*.node.test.ts`) because it scans
 * source text from the filesystem; browser-mode test environments
 * have no `node:fs` or `process.cwd()`.
 */

import { describe, it, expect } from "vitest";
import { readFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";

/**
 * Files allowed to call `setFocus(<non-null>)` (or a similarly-shaped
 * kernel-facing primitive) directly:
 *
 *   - `lib/entity-focus-context.tsx` — owns the kernel-facing
 *     `setFocus` primitive itself (the action exposed by
 *     `useFocusActions`) AND registers the inner `nav.focus` execute
 *     closure that wraps it.
 *   - `lib/spatial-focus-context.tsx` — the outer `nav.focus` execute
 *     closure calls `spatial.focus(fq)` directly. This is the
 *     kernel-facing primitive `setFocus` ultimately delegates to.
 *
 * `components/app-shell.tsx` and `components/entity-inspector.tsx`
 * historically appeared on this allowlist as future-proofing slots,
 * but neither file currently contains a line that matches the
 * `(\w+\.)?setFocus\(<non-null>\)` shape — both call sites use
 * `setFocusRef.current(result)`, which does not contain the literal
 * `setFocus(` substring (the `setFocus` token is followed by `Ref`,
 * not `(`). They are therefore not included in the allowlist; if a
 * real direct call shape ever lands in either file, add the file
 * back here with a comment describing the specific call site.
 *
 * Every other production component must dispatch `nav.focus` through
 * `useDispatchCommand("nav.focus")`.
 */
const ALLOWED_DIRECT_SETFOCUS_FILES = new Set([
  "kanban-app/ui/src/lib/entity-focus-context.tsx",
  "kanban-app/ui/src/lib/spatial-focus-context.tsx",
]);

/** Production component files we scan for direct `setFocus` calls. */
const SCANNED_FILES = [
  "kanban-app/ui/src/components/board-view.tsx",
  "kanban-app/ui/src/components/column-view.tsx",
  "kanban-app/ui/src/components/data-table.tsx",
  "kanban-app/ui/src/components/grid-view.tsx",
  "kanban-app/ui/src/components/cursor-focus-bridge.tsx",
  "kanban-app/ui/src/components/fields/field.tsx",
  "kanban-app/ui/src/components/perspective-tab-bar.tsx",
  "kanban-app/ui/src/components/focus-scope.tsx",
  "kanban-app/ui/src/components/jump-to-overlay.tsx",
  "kanban-app/ui/src/components/inspectors-container.tsx",
];

/**
 * Resolve the base the `kanban-app/ui/...` entries in `SCANNED_FILES`
 * are relative to. The UI project root (`process.cwd()`, where vitest
 * runs) is `apps/kanban-app/ui`, so two levels up is `apps/` — the
 * directory the `kanban-app/ui/...` prefixes resolve against.
 */
function repoRoot(): string {
  return resolve(process.cwd(), "..", "..");
}

/**
 * Test whether `arg` looks like a TypeScript parameter-declaration
 * fragment such as `fq: FullyQualifiedMoniker | null`, where the
 * regex `setFocus\(([^)]*)\)` happens to capture an unrelated
 * parenthesised tail of a signature line. Such fragments start with
 * an identifier followed by a colon (`<ident>:`) and never with a
 * quote or a bracket / brace.
 */
function looksLikeTsParamDecl(arg: string): boolean {
  return /^\w+\s*:/.test(arg);
}

/**
 * Test whether `arg` is a string literal — `"..."`, `'...'`, or a
 * template literal `` `...` ``. String-literal arguments to
 * `setFocus` (e.g. `setFocus("entity:foo")`) are real call sites
 * that the guard MUST flag, even though they may contain a colon
 * inside the quotes (which would otherwise look like a TS parameter
 * declaration to the colon-only heuristic).
 */
function looksLikeStringLiteral(arg: string): boolean {
  return (
    /^"(?:[^"\\]|\\.)*"$/.test(arg) ||
    /^'(?:[^'\\]|\\.)*'$/.test(arg) ||
    /^`(?:[^`\\]|\\.)*`$/.test(arg)
  );
}

/**
 * Scan a single file for `setFocus(<non-null>)` call sites that
 * indicate a direct (non-`nav.focus`) focus claim. Returns the list
 * of violation strings (`<file>:<line>: <trimmed line>`).
 *
 * Heuristic:
 *   - Skip lines that are pure comments (`//`, `*`, `/*`).
 *   - Match `(<ident>.)?setFocus(<arg>)` and inspect `<arg>`.
 *   - Allow `null` and empty (no-arg).
 *   - Allow `<ident>:` shaped args (TypeScript parameter declarations
 *     like `setFocus: (fq: ...) => void`) UNLESS the arg also looks
 *     like a string literal — `setFocus("entity:foo")` is a real
 *     call site that must be flagged even though it contains a
 *     colon inside the quotes.
 */
function scanFileForDirectSetFocus(relPath: string, text: string): string[] {
  const violations: string[] = [];
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    if (
      trimmed.startsWith("//") ||
      trimmed.startsWith("*") ||
      trimmed.startsWith("/*")
    ) {
      continue;
    }
    const match = line.match(/(\w+\.)?setFocus\(([^)]*)\)/);
    if (!match) continue;
    const arg = match[2].trim();
    if (arg === "" || arg === "null") continue;
    if (looksLikeTsParamDecl(arg) && !looksLikeStringLiteral(arg)) {
      continue;
    }
    violations.push(`${relPath}:${i + 1}: ${trimmed}`);
  }
  return violations;
}

describe("nav.focus source-level guard", () => {
  it("no scanned production component calls setFocus(<non-null>) directly outside the allowlist", () => {
    const root = repoRoot();
    const violations: string[] = [];
    for (const relPath of SCANNED_FILES) {
      if (ALLOWED_DIRECT_SETFOCUS_FILES.has(relPath)) continue;
      const absPath = resolve(root, relPath);
      if (!existsSync(absPath)) continue;
      const text = readFileSync(absPath, "utf8");
      violations.push(...scanFileForDirectSetFocus(relPath, text));
    }

    expect(
      violations,
      `Direct setFocus(<non-null>) calls found in production components.\n` +
        `Per card 01KR7CDEFWWVF4WH0BCHE8Y21J every focus claim must flow ` +
        `through dispatchNavFocus({ args: { fq } }) — the single ` +
        `auditable command that wraps the kernel-facing setFocus ` +
        `primitive. Move these call sites to dispatch nav.focus instead.\n\n` +
        `Violations:\n${violations.join("\n")}`,
    ).toEqual([]);
  });

  // Probe tests for the heuristic itself. They feed inline source
  // text to `scanFileForDirectSetFocus` so a regression that loosens
  // the heuristic — for example, re-introducing the original
  // `arg.includes(":")` blanket skip — would surface here even if no
  // production file currently exhibits the bad pattern.
  describe("scanFileForDirectSetFocus heuristic", () => {
    it("flags string-literal direct calls (the colon-in-arg blind spot)", () => {
      // The original heuristic skipped any arg containing a colon,
      // which would have let `setFocus("entity:foo")` slip past as
      // if it were a TypeScript parameter declaration. The tightened
      // heuristic distinguishes string literals from `<ident>:`
      // shaped TS sig fragments.
      const sourceWithDoubleQuote = `
        function bad() {
          setFocus("entity:foo");
        }
      `;
      const sourceWithSingleQuote = `
        function bad() {
          setFocus('entity:foo');
        }
      `;
      const sourceWithTemplateLiteral = `
        function bad() {
          setFocus(\`entity:\${id}\`);
        }
      `;

      for (const text of [
        sourceWithDoubleQuote,
        sourceWithSingleQuote,
        sourceWithTemplateLiteral,
      ]) {
        const violations = scanFileForDirectSetFocus("probe.tsx", text);
        expect(
          violations.length,
          `Expected at least one violation for source:\n${text}`,
        ).toBeGreaterThan(0);
      }
    });

    it("flags identifier-arg direct calls", () => {
      const text = `
        function bad(fq) {
          setFocus(fq);
        }
      `;
      const violations = scanFileForDirectSetFocus("probe.tsx", text);
      expect(violations.length).toBeGreaterThan(0);
    });

    it("flags qualified direct calls like actions.setFocus(fq)", () => {
      const text = `
        function bad(actions, fq) {
          actions.setFocus(fq);
        }
      `;
      const violations = scanFileForDirectSetFocus("probe.tsx", text);
      expect(violations.length).toBeGreaterThan(0);
    });

    it("does NOT flag setFocus(null) or setFocus()", () => {
      const text = `
        function ok() {
          setFocus(null);
          setFocus();
        }
      `;
      const violations = scanFileForDirectSetFocus("probe.tsx", text);
      expect(violations).toEqual([]);
    });

    it("does NOT flag TypeScript signature fragments like setFocus(fq: T)", () => {
      // This shape only matches the regex when wrapped inside a
      // function-typed parameter list, e.g.
      //   `(setFocus(fq: FullyQualifiedMoniker | null) => void)`.
      // The captured arg is `fq: FullyQualifiedMoniker | null` — a
      // TS parameter declaration, not a call.
      const text = `
        type Foo = {
          callback: (setFocus(fq: FullyQualifiedMoniker | null) => void);
        };
      `;
      const violations = scanFileForDirectSetFocus("probe.tsx", text);
      expect(violations).toEqual([]);
    });

    it("does NOT flag setFocus mentions inside line or block comments", () => {
      const text = `
        // setFocus(foo)
        /* setFocus(bar) */
        * setFocus(baz)
      `;
      const violations = scanFileForDirectSetFocus("probe.tsx", text);
      expect(violations).toEqual([]);
    });
  });
});
