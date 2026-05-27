/**
 * Generic CM6 mention autocomplete — parameterized by prefix character.
 * Supports both sync (Map-based) and async (search function) modes.
 */

import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";

/** Result item from a mention search — an entity carrying a display color. */
export interface MentionSearchResult {
  slug: string;
  displayName: string;
  color: string;
}

/**
 * Result item from a slash-command search.
 *
 * The slash-command flavor of the same shared completion source: a command
 * carries a human-readable `description` instead of a color, so its dropdown
 * info widget shows the description text and skips the colored dot.
 */
export interface CommandSearchResult {
  name: string;
  description: string;
}

/** Either flavor of completion result the shared source can render. */
export type CompletionSearchResult = MentionSearchResult | CommandSearchResult;

/** Narrow a completion result to the slash-command flavor. */
function isCommandResult(
  result: CompletionSearchResult,
): result is CommandSearchResult {
  return "name" in result;
}

/** Sync search function type — filters a local map */
export type MentionSearchSync = (query: string) => CompletionSearchResult[];

/** Async search function type — calls backend */
export type MentionSearchAsync = (
  query: string,
) => Promise<CompletionSearchResult[]>;

/**
 * Build a CM6 completion option for an entity-mention result.
 *
 * The dropdown previews the rendered pill (`displayName`); `apply` writes the
 * slug (the underlying identifier); the info widget is a colored dot followed
 * by the display name.
 */
function buildMentionOption(
  prefix: string,
  r: MentionSearchResult,
  query: string,
): Completion {
  return {
    // Dropdown label previews what the widget will show after insertion
    // (displayName, not slug) so the dropdown matches the buffer's rendered
    // pill.
    label: `${prefix}${r.displayName}`,
    // `apply` overrides `label` for what actually lands in the document: the
    // slug is the underlying identifier, even though the widget shows the
    // display name.
    apply: `${prefix}${r.slug}`,
    // Secondary hint showing the slug so users can see what will be written.
    detail: r.slug,
    type: "keyword",
    boost: r.slug.startsWith(query) ? 1 : 0,
    info: () => {
      const dom = document.createElement("span");
      dom.style.display = "inline-flex";
      dom.style.alignItems = "center";
      dom.style.gap = "6px";
      const dot = document.createElement("span");
      dot.style.width = "8px";
      dot.style.height = "8px";
      dot.style.borderRadius = "50%";
      dot.style.backgroundColor = `#${r.color}`;
      dom.appendChild(dot);
      dom.appendChild(document.createTextNode(r.displayName));
      return dom;
    },
  };
}

/**
 * Build a CM6 completion option for a slash-command result.
 *
 * A command has no slug/color: both the dropdown label and what `apply` writes
 * are the literal `${prefix}${name}` (the agent owns command execution — the
 * composer only inserts the text). The info widget is the plain description
 * text, with no colored dot.
 */
function buildCommandOption(
  prefix: string,
  r: CommandSearchResult,
  query: string,
): Completion {
  return {
    label: `${prefix}${r.name}`,
    apply: `${prefix}${r.name}`,
    detail: r.description,
    type: "keyword",
    boost: r.name.startsWith(query) ? 1 : 0,
    info: () => {
      const dom = document.createElement("span");
      dom.textContent = r.description;
      return dom;
    },
  };
}

/**
 * Create a completion source for a given prefix. Returns just the source
 * function — callers must combine all sources into a single `autocompletion()`
 * extension to avoid CM6 config merge conflicts.
 *
 * The same source builder serves both flavors: entity mentions (`#`/`@`/`^`/`$`,
 * a {@link MentionSearchResult} with a color) and slash commands (`/`, a
 * {@link CommandSearchResult} with a description). The search function's result
 * shape selects the dropdown rendering per option.
 *
 * @param prefix - The completion prefix character (e.g. `#`, `@`, `/`)
 * @param search - Sync or async search function
 * @param options - Source tuning. `openOnBarePrefix` lets a bare prefix open
 *   the menu during auto-typing (explicit `false`) — used by the slash-command
 *   source so typing just `/` lists every command. Entity mentions leave it
 *   off so a bare `#`/`@` does not dump the full entity list.
 */
export function createMentionCompletionSource(
  prefix: string,
  search: MentionSearchSync | MentionSearchAsync,
  options?: { openOnBarePrefix?: boolean },
): (
  context: CompletionContext,
) => CompletionResult | null | Promise<CompletionResult | null> {
  const prefixRegex = new RegExp(`\\${prefix}\\S*`);
  const openOnBarePrefix = options?.openOnBarePrefix ?? false;

  return (context: CompletionContext) => {
    const word = context.matchBefore(prefixRegex);
    if (!word) return null;
    if (word.text === prefix && !context.explicit && !openOnBarePrefix)
      return null;

    const query = word.text.slice(prefix.length).toLowerCase();
    const from = word.from;

    const buildResult = (
      results: CompletionSearchResult[],
    ): CompletionResult | null => {
      // An empty result set closes the menu rather than opening an empty one.
      // The composer's Enter-yield guard depends on this: no menu means
      // `completionStatus` stays null and Enter submits normally.
      if (results.length === 0) return null;
      const completions: Completion[] = results.map((r) =>
        isCommandResult(r)
          ? buildCommandOption(prefix, r, query)
          : buildMentionOption(prefix, r, query),
      );
      return { from, options: completions, filter: false };
    };

    const result = search(query);
    if (result instanceof Promise) {
      return result.then(buildResult);
    }
    return buildResult(result);
  };
}

/**
 * Create a single autocompletion extension from multiple completion sources.
 * This avoids the CM6 "Config merge conflict for field override" error that
 * occurs when multiple `autocompletion()` calls are combined.
 */
export function createMentionAutocomplete(
  sources: Array<
    (
      context: CompletionContext,
    ) => CompletionResult | null | Promise<CompletionResult | null>
  >,
) {
  return autocompletion({
    override: sources,
    activateOnTyping: true,
    activateOnTypingDelay: 150,
  });
}

/**
 * Create a sync search function from a slug→color Map.
 * This preserves the existing tag autocomplete behavior.
 */
export function syncSearchFromMap(
  colors: Map<string, string>,
): MentionSearchSync {
  return (query: string) => {
    const results: MentionSearchResult[] = [];
    for (const [slug, color] of colors) {
      if (query && !slug.includes(query)) continue;
      results.push({ slug, displayName: slug, color });
    }
    return results;
  };
}
