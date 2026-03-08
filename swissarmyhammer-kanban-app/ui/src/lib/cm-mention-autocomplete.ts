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

/** Result item from a mention search */
export interface MentionSearchResult {
  slug: string;
  displayName: string;
  color: string;
}

/** Sync search function type — filters a local map */
export type MentionSearchSync = (query: string) => MentionSearchResult[];

/** Async search function type — calls backend */
export type MentionSearchAsync = (query: string) => Promise<MentionSearchResult[]>;

/**
 * Create a completion source for a given prefix. Returns just the source
 * function — callers must combine all sources into a single `autocompletion()`
 * extension to avoid CM6 config merge conflicts.
 *
 * @param prefix - The mention prefix character (e.g. `#`, `@`)
 * @param search - Sync or async search function
 */
export function createMentionCompletionSource(
  prefix: string,
  search: MentionSearchSync | MentionSearchAsync,
): (context: CompletionContext) => CompletionResult | null | Promise<CompletionResult | null> {
  const prefixRegex = new RegExp(`\\${prefix}\\S*`);

  return (context: CompletionContext) => {
    const word = context.matchBefore(prefixRegex);
    if (!word) return null;
    if (word.text === prefix && !context.explicit) return null;

    const query = word.text.slice(prefix.length).toLowerCase();
    const from = word.from;

    const buildResult = (results: MentionSearchResult[]): CompletionResult => {
      const options: Completion[] = results.map((r) => ({
        label: `${prefix}${r.slug}`,
        detail: r.displayName,
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
      }));
      return { from, options, filter: false };
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
  sources: Array<(context: CompletionContext) => CompletionResult | null | Promise<CompletionResult | null>>,
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
export function syncSearchFromMap(colors: Map<string, string>): MentionSearchSync {
  return (query: string) => {
    const results: MentionSearchResult[] = [];
    for (const [slug, color] of colors) {
      if (query && !slug.includes(query)) continue;
      results.push({ slug, displayName: slug, color });
    }
    return results;
  };
}
