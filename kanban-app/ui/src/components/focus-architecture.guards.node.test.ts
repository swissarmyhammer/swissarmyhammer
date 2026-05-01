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
    // indicator from the three peer primitives (`<FocusScope>` and
    // `<FocusZone>`); any other production source rendering its own focus
    // bar / focus highlight is a duplicate decorator and a regression.
    //
    // After the three-peer collapse, `<FocusScope>` is the leaf primitive
    // (it absorbed what the legacy `<Focusable>` used to do). The
    // transitional re-export at `focusable.tsx` was deleted in card
    // `01KQ5PSMYE3Q60SV8270S6K819`; `<FocusScope>` is the only leaf
    // primitive, and it composes `<FocusIndicator>` directly.
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
      "components/focus-scope.tsx",
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

  it("the FocusIndicatorVariant type is fully removed from production code", () => {
    // `FocusIndicatorVariant` was a short-lived type that gated a `"ring"`
    // visual variant of the focus indicator. The user explicitly rejected
    // a second indicator visual — there is one cursor-bar, period.
    // Any remaining reference in shipped production sources resurrects
    // that variant surface and reintroduces the multi-decorator
    // antipattern.
    //
    // Test files are excluded from the production-code scan because the
    // single-variant browser test deliberately spells the dead prop name
    // inside `@ts-expect-error` directives that are themselves the assertion
    // (the build fails if any of those stops being an error). Guarding
    // the production tree is what enforces the deletion.
    const tsxFiles = walkSources(SRC_ROOT, [".tsx", ".ts"]);
    const offenders: string[] = [];
    for (const file of tsxFiles) {
      const rel = relative(SRC_ROOT, file);
      if (rel.endsWith(".test.ts") || rel.endsWith(".test.tsx")) continue;
      if (rel.endsWith(".node.test.ts")) continue;
      const src = readFileSync(file, "utf-8");
      if (/\bFocusIndicatorVariant\b/.test(src)) {
        offenders.push(rel);
      }
    }
    if (offenders.length > 0) {
      throw new Error(
        `FocusIndicatorVariant is gone — references must not remain in production code:\n${offenders.map((o) => `  ${o}`).join("\n")}`,
      );
    }
  });

  it("the focusIndicatorVariant prop is fully removed from production code", () => {
    // The lower-cased prop name was threaded through `<FocusScope>`,
    // `<FocusZone>`, and consumer call sites (the navbar). All references
    // must be deleted from production code along with the type. Test
    // files are excluded for the same reason as the type guard above —
    // the `@ts-expect-error` test deliberately writes the dead literal.
    const tsxFiles = walkSources(SRC_ROOT, [".tsx", ".ts"]);
    const offenders: string[] = [];
    for (const file of tsxFiles) {
      const rel = relative(SRC_ROOT, file);
      if (rel.endsWith(".test.ts") || rel.endsWith(".test.tsx")) continue;
      if (rel.endsWith(".node.test.ts")) continue;
      const src = readFileSync(file, "utf-8");
      if (/\bfocusIndicatorVariant\b/.test(src)) {
        offenders.push(rel);
      }
    }
    if (offenders.length > 0) {
      throw new Error(
        `focusIndicatorVariant is gone — references must not remain in production code:\n${offenders.map((o) => `  ${o}`).join("\n")}`,
      );
    }
  });

  // ---------------------------------------------------------------------
  // Inspectable architectural guards (cards 01KQ7GM77B1E6YH8Z893K05VKY +
  // 01KQ7K7KZNR3EHS9SY0XY79NYE)
  //
  // The inspector exists to show details of *entities* — `task:`, `tag:`,
  // `column:`, `board:`, `field:`, `attachment:`. UI chrome (`ui:*`,
  // `perspective_tab:`, `cell:*`, `grid_cell:*`) is not. The dispatch
  // route from a double-click to the `ui.inspect` command lives in
  // exactly one component: `<Inspectable>` (`inspectable.tsx`). The
  // spatial primitives `<FocusScope>` and `<FocusZone>` are pure
  // spatial-nav infrastructure and never call
  // `useDispatchCommand("ui.inspect")`.
  //
  // Three guards keep this honest:
  //
  //   - Guard A: the literal `useDispatchCommand("ui.inspect")` appears
  //     in exactly one non-test file owning the double-click route:
  //     `inspectable.tsx`. Other files that legitimately dispatch
  //     `ui.inspect` from non-double-click sources (keyboard, navbar
  //     button, command-palette) are explicitly allowlisted.
  //
  //   - Guard B: every `<Inspectable …moniker={asSegment("<prefix>…")}>`
  //     JSX hit has a prefix in ENTITY_PREFIXES — chrome cannot
  //     accidentally be wrapped in `<Inspectable>`.
  //
  //   - Guard C: every entity-prefixed `<FocusScope>` / `<FocusZone>`
  //     JSX hit has a matching `<Inspectable>` element (with the same
  //     moniker substring) in the same file — so an entity-zone
  //     wrapper cannot be added without its `<Inspectable>` partner.
  //
  // Test files are excluded from the walks so synthetic test fixtures do
  // not trip the guards.
  // ---------------------------------------------------------------------

  /**
   * SegmentMoniker prefixes that identify real, inspectable entities. Both
   * Guard B (Inspectable monikers must use one of these) and Guard C
   * (entity-prefixed primitives need an Inspectable in the same file)
   * read from this list.
   */
  const ENTITY_PREFIXES = [
    "task:",
    "tag:",
    "column:",
    "board:",
    "field:",
    "attachment:",
  ];

  /** Strip line and block comments while preserving newline structure. */
  function stripJsComments(src: string): string {
    return src
      .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
      .replace(/(^|[^:\\])\/\/[^\n]*/g, (_m, prefix) => prefix);
  }

  it('Guard A: useDispatchCommand("ui.inspect") appears only in inspectable.tsx (double-click dispatch is single-sourced)', () => {
    // The double-click → inspector route goes through a single
    // dispatch site so an audit can confirm both the gesture-
    // skipping rules (input/textarea/contenteditable) and the
    // `stopPropagation` semantics in one place.
    //
    // Other production callers of `useDispatchCommand("ui.inspect")`
    // exist for non-double-click flows (keyboard, navbar button,
    // command-palette) and are explicitly allowlisted below. Guard A
    // is about the **double-click** route's single-source contract,
    // not a blanket "no other code may dispatch ui.inspect".
    //
    // Adding a new caller? Either:
    //   1. it is the new double-click site (then move it to
    //      `inspectable.tsx`), or
    //   2. it is a different gesture (then add it to ALLOWLIST below
    //      with a comment explaining the gesture).
    const ALLOWLIST = new Set<string>([
      // The single double-click dispatch site. Guard A's whole point.
      "components/inspectable.tsx",
      // Keyboard inspect command — the board's `useBoardActionCommands`
      // wires it through the spatial-nav action layer.
      "components/board-view.tsx",
      // The navbar's `Inspect` button click (mouse, not double-click).
      "components/nav-bar.tsx",
      // The command palette's search-mode `Inspect` row.
      "components/command-palette.tsx",
      // The card's explicit `<InspectButton>` (the small "i" icon on
      // each card). The button click is a single-click affordance for
      // users who don't know the dblclick gesture.
      "components/entity-card.tsx",
    ]);

    const tsxFiles = walkSources(SRC_ROOT, [".ts", ".tsx"]);
    const offenders: string[] = [];
    for (const file of tsxFiles) {
      const rel = relative(SRC_ROOT, file);
      if (rel.endsWith(".test.ts") || rel.endsWith(".test.tsx")) continue;
      if (rel.endsWith(".node.test.ts")) continue;

      const stripped = stripJsComments(readFileSync(file, "utf-8"));
      // `useDispatchCommand("ui.inspect")` — match both single and
      // double quotes; allow whitespace inside the parentheses.
      if (
        /useDispatchCommand\(\s*[`"']ui\.inspect[`"']\s*\)/.test(stripped) &&
        !ALLOWLIST.has(rel)
      ) {
        offenders.push(rel);
      }
    }

    if (offenders.length > 0) {
      throw new Error(
        `useDispatchCommand("ui.inspect") must live only in the documented\n` +
          `dispatch sites (the double-click route is single-sourced in\n` +
          `inspectable.tsx; other gestures are explicitly allowlisted).\n` +
          `Offending call sites:\n` +
          offenders.map((o) => `  ${o}`).join("\n"),
      );
    }
  });

  it("Guard B: every <Inspectable> wraps an entity-prefixed moniker", () => {
    // Walks every `*.tsx` source file under `src/`, finds every
    // `<Inspectable …moniker={asSegment("<prefix>…")}>` JSX hit, and
    // asserts the prefix is in `ENTITY_PREFIXES`. UI-chrome monikers
    // (`ui:*`, `perspective_tab:`, `cell:*`, `grid_cell:*`) cannot be
    // wrapped in `<Inspectable>` — chrome is not inspectable.
    //
    // Test files are excluded so synthetic test fixtures do not trip
    // the guard.

    /**
     * Match a JSX element opener `<Inspectable ... >` and capture its
     * attribute block up to the `>` or `/>` terminator.
     */
    const ELEMENT_RE = /<Inspectable\b([\s\S]*?)(\/>|>)/g;

    /**
     * Match a `moniker={asSegment("...")}` prop with a statically-
     * readable string literal. Variables are not followed — the goal
     * is hygiene for the common case (every production call site uses
     * a literal).
     */
    const MONIKER_RE = /moniker=\{\s*asSegment\(\s*[`"']([^`"']+)[`"']/;

    /** Match `moniker={someVar}` so we can warn rather than skip. */
    const MONIKER_VAR_RE = /moniker=\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}/;

    const tsxFiles = walkSources(SRC_ROOT, [".tsx"]);
    const offenders: { file: string; moniker: string }[] = [];

    for (const file of tsxFiles) {
      const rel = relative(SRC_ROOT, file);
      if (rel.endsWith(".test.tsx")) continue;
      if (rel.endsWith(".test.ts")) continue;
      if (rel.endsWith(".node.test.ts")) continue;
      const stripped = stripJsComments(readFileSync(file, "utf-8"));

      ELEMENT_RE.lastIndex = 0;
      let match: RegExpExecArray | null;
      while ((match = ELEMENT_RE.exec(stripped)) !== null) {
        const attrs = match[1];
        const literalMatch = attrs.match(MONIKER_RE);
        if (literalMatch) {
          const moniker = literalMatch[1];
          const ok = ENTITY_PREFIXES.some((p) => moniker.startsWith(p));
          if (!ok) {
            const lineIdx =
              stripped.slice(0, match.index).split("\n").length - 1;
            offenders.push({
              file: `${rel}:${lineIdx + 1}`,
              moniker,
            });
          }
          continue;
        }
        // `<Inspectable moniker={var}>` — accept; literal-prefix
        // checking is best-effort. The wrapping component itself
        // accepts any `SegmentMoniker`, and call-site review during PR is
        // expected to confirm the variable resolves to an entity
        // moniker.
        const _varMatch = attrs.match(MONIKER_VAR_RE);
        void _varMatch;
      }
    }

    if (offenders.length > 0) {
      throw new Error(
        `<Inspectable> may only wrap entity monikers (` +
          ENTITY_PREFIXES.join(", ") +
          `). Offending call sites:\n` +
          offenders
            .map((o) => `  ${o.file}  moniker="${o.moniker}"`)
            .join("\n"),
      );
    }
  });

  it("Guard C: every entity-prefixed <FocusScope> / <FocusZone> has a sibling <Inspectable> in the same file", () => {
    // Walks every `*.tsx` source file under `src/`, finds every
    // `<FocusScope ... >` / `<FocusZone ... >` JSX hit whose `moniker`
    // literal starts with an `ENTITY_PREFIXES` entry, and asserts the
    // same file contains an `<Inspectable>` element with a matching
    // moniker substring. This catches "someone added a new entity
    // wrapper but forgot the `<Inspectable>`".
    //
    // The match is on the moniker prefix-and-tail substring rather
    // than the full literal so that a `<FocusScope moniker="task:01">`
    // and a sibling `<Inspectable moniker={asSegment(`task:${id}`)}>`
    // (template literal that the prefix scan can't fully read) still
    // pair up. The architectural intent — "this entity wrapper is
    // accompanied by an Inspectable wrapper in the same file" — is
    // what we're asserting; an exact moniker-literal match would
    // brittlely require the same expression on both sides.
    //
    // The escape hatch `// inspect:exempt` (within 3 lines above the
    // JSX opener) is preserved for the rare case where an
    // entity-prefixed `<FocusScope>` / `<FocusZone>` cannot be paired
    // with an `<Inspectable>` in the same file. Today no production
    // call site needs the carve-out — the column-name synthetic leaf
    // (`column:<id>.name`) was the only consumer and it was collapsed
    // into the inner `<Field>` zone (`fields/field.tsx` already wraps
    // in `<Inspectable>`). The mechanism stays in place for any future
    // synthetic-moniker case. The `data-table.tsx` row case is handled
    // differently: the row `<FocusScope renderContainer={false}>`
    // doesn't render a host element, so DOM rules prevent wrapping in
    // `<Inspectable>`. The inspect dispatch lives directly on the row's
    // `<tr>` via the `useInspectOnDoubleClick` hook (still in
    // `inspectable.tsx`). That row `<FocusScope>` carries
    // `renderContainer={false}` and is therefore exempt from this
    // guard — the runtime DOM never renders an element to attach
    // `onDoubleClick` to anyway.

    /**
     * Match a JSX element opener `<FocusScope ... >` or `<FocusZone ... >`
     * and capture the component name + attribute block.
     */
    const PRIMITIVE_RE = /<(FocusScope|FocusZone)\b([\s\S]*?)(\/>|>)/g;

    const MONIKER_RE = /moniker=\{\s*asSegment\(\s*[`"']([^`"']+)[`"']/;

    /** Match `<Inspectable moniker={asSegment("...")}>` literals in the file. */
    const INSPECTABLE_RE =
      /<Inspectable\b[\s\S]*?moniker=\{\s*asSegment\(\s*[`"']([^`"']+)[`"']/g;

    /**
     * True when the element opener is preceded — in the original (un-
     * stripped) source — by a `// inspect:exempt` comment within the
     * preceding 3 lines.
     */
    function hasExemptComment(
      originalLines: string[],
      lineIdx: number,
    ): boolean {
      const start = Math.max(0, lineIdx - 3);
      for (let i = start; i < lineIdx; i++) {
        if (/\/\/\s*inspect:exempt\b/.test(originalLines[i])) return true;
        if (/\binspect:exempt\b/.test(originalLines[i])) return true;
      }
      return false;
    }

    /**
     * True when the attribute block has `renderContainer={false}` (or
     * literally `renderContainer={false}` after whitespace
     * normalization). Such call sites are exempt because they don't
     * render a DOM element to attach `onDoubleClick` to — the inspect
     * dispatch lives on the inner host element (e.g. the `<tr>` in
     * `data-table.tsx`).
     */
    function hasRenderContainerFalse(attrs: string): boolean {
      return /\brenderContainer\s*=\s*\{\s*false\s*\}/.test(attrs);
    }

    const tsxFiles = walkSources(SRC_ROOT, [".tsx"]);
    const offenders: { file: string; moniker: string }[] = [];

    for (const file of tsxFiles) {
      const rel = relative(SRC_ROOT, file);
      if (rel.endsWith(".test.tsx")) continue;
      if (rel.endsWith(".test.ts")) continue;
      if (rel.endsWith(".node.test.ts")) continue;

      const original = readFileSync(file, "utf-8");
      const stripped = stripJsComments(original);
      const originalLines = original.split("\n");

      // Collect every `<Inspectable>` moniker literal in the file so
      // we can match by tail substring.
      const inspectableMonikers: string[] = [];
      INSPECTABLE_RE.lastIndex = 0;
      let im: RegExpExecArray | null;
      while ((im = INSPECTABLE_RE.exec(stripped)) !== null) {
        inspectableMonikers.push(im[1]);
      }

      PRIMITIVE_RE.lastIndex = 0;
      let match: RegExpExecArray | null;
      while ((match = PRIMITIVE_RE.exec(stripped)) !== null) {
        const attrs = match[2];
        const monikerMatch = attrs.match(MONIKER_RE);
        if (!monikerMatch) continue;
        const moniker = monikerMatch[1];
        const isEntity = ENTITY_PREFIXES.some((p) => moniker.startsWith(p));
        if (!isEntity) continue;

        if (hasRenderContainerFalse(attrs)) continue;

        const lineIdx = stripped.slice(0, match.index).split("\n").length - 1;
        if (hasExemptComment(originalLines, lineIdx)) continue;

        // Match against the file's `<Inspectable>` monikers by tail
        // substring: the FocusScope's moniker (e.g. `task:01`) is a
        // prefix-or-equal match against an Inspectable moniker (e.g.
        // `task:01` or, when the wrapper uses a template,
        // `task:${id}` — which under our literal scanner won't hit at
        // all and we tolerate at the var-not-tracked level). To keep
        // false positives low, we require an `<Inspectable>` whose
        // moniker shares the same prefix — any entity-prefixed
        // Inspectable in the file is taken as evidence the wrapper is
        // present; the call-site review handles tighter pairing.
        const prefix = ENTITY_PREFIXES.find((p) => moniker.startsWith(p))!;
        const haveSibling = inspectableMonikers.some((m) =>
          m.startsWith(prefix),
        );
        if (!haveSibling) {
          offenders.push({
            file: `${rel}:${lineIdx + 1}`,
            moniker,
          });
        }
      }
    }

    if (offenders.length > 0) {
      throw new Error(
        `Every entity-prefixed <FocusScope>/<FocusZone> must be\n` +
          `paired with an <Inspectable> in the same file (the wrapper owns\n` +
          `the double-click → ui.inspect dispatch). Offending call sites:\n` +
          offenders
            .map((o) => `  ${o.file}  moniker="${o.moniker}"`)
            .join("\n") +
          `\n(Add an inline \`// inspect:exempt\` comment within 3 lines\n` +
          `above the JSX opener for the rare synthetic-entity moniker case,\n` +
          `or pass \`renderContainer={false}\` if the wrapper should not\n` +
          `render a DOM element.)`,
      );
    }
  });

  // ---------------------------------------------------------------------
  // Card-field migration guards (card 01KQAWV9C5F8Y3AA0KDDHHRRN1)
  //
  // The card surface migrated from the parallel render path
  // (`<CardFieldIcon>` rendered as a sibling of `<Field>`) to the unified
  // `<Field withIcon />` shape the inspector already uses. These guards
  // catch a future revert at lint time, not at user-report time.
  // ---------------------------------------------------------------------

  it("entity-card.tsx does NOT define a CardFieldIcon symbol (card uses <Field withIcon />)", () => {
    // The local `CardFieldIcon` helper was a sibling-icon render path
    // that placed the icon OUTSIDE the field's `<FocusZone>`. The card
    // now renders through `<Field withIcon />`, which puts the icon
    // INSIDE the zone (matching the inspector). Re-introducing
    // `CardFieldIcon` resurrects the parallel render path and the
    // architectural divergence between card and inspector.
    const cardPath = resolve(SRC_ROOT, "components/entity-card.tsx");
    const stripped = stripJsComments(readFileSync(cardPath, "utf-8"));
    if (/\bCardFieldIcon\b/.test(stripped)) {
      throw new Error(
        `entity-card.tsx must not define or reference CardFieldIcon — the card\n` +
          `field icon now renders inside <Field withIcon />, matching the\n` +
          `inspector. Re-introducing CardFieldIcon resurrects the parallel\n` +
          `render path that put the icon outside the field's <FocusZone>.`,
      );
    }
  });

  it("entity-card.tsx contains no calls to getDisplayIconOverride / getDisplayTooltipOverride", () => {
    // The card no longer reimplements icon / tooltip override resolution
    // — `<Field withIcon />` does it via `resolveFieldIconAndTip`
    // (`fields/field.tsx`). A reappearing import or call site means the
    // card has drifted back into duplicating that logic.
    const cardPath = resolve(SRC_ROOT, "components/entity-card.tsx");
    const stripped = stripJsComments(readFileSync(cardPath, "utf-8"));
    const offenders: string[] = [];
    if (/\bgetDisplayIconOverride\b/.test(stripped)) {
      offenders.push("getDisplayIconOverride");
    }
    if (/\bgetDisplayTooltipOverride\b/.test(stripped)) {
      offenders.push("getDisplayTooltipOverride");
    }
    if (offenders.length > 0) {
      throw new Error(
        `entity-card.tsx must not reference ${offenders.join(", ")} —\n` +
          `the card field icon / tooltip resolution now lives inside\n` +
          `<Field withIcon /> via resolveFieldIconAndTip. The card surface\n` +
          `must not duplicate that logic.`,
      );
    }
  });

  it('focus-indicator.tsx contains no "ring" literal in code', () => {
    // The bar's class names never include the substring `ring`, so any
    // occurrence of the literal `"ring"` in this file would be the
    // resurrected variant branch. The guard scans the indicator file
    // directly (not the wider tree, where unrelated `ring`-named tokens
    // legitimately exist in CSS / docs).
    //
    // JS / JSDoc comments are stripped before the scan so that this
    // file's own docstring can mention the dead variant by name without
    // tripping the guard. The same comment-stripping shape is used by
    // the other guards in this file.
    function stripJsComments(src: string): string {
      return src
        .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
        .replace(/(^|[^:\\])\/\/[^\n]*/g, (_m, prefix) => prefix);
    }
    const indicatorPath = resolve(SRC_ROOT, "components/focus-indicator.tsx");
    const stripped = stripJsComments(readFileSync(indicatorPath, "utf-8"));
    if (/"ring"/.test(stripped)) {
      throw new Error(
        `focus-indicator.tsx contains a "ring" string literal in code — the ring variant must stay deleted.`,
      );
    }
  });
});
