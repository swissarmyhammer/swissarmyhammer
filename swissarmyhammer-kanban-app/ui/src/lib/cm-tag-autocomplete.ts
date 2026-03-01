import { Facet } from "@codemirror/state";
import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";

/** Facet providing tag data for autocomplete: slug → hex color (without #) */
export const tagAutocompleteFacet = Facet.define<
  Map<string, string>,
  Map<string, string>
>({
  combine(values) {
    return values.length > 0 ? values[values.length - 1] : new Map();
  },
});

/** Completion source for #tag patterns */
function tagCompletionSource(
  context: CompletionContext
): CompletionResult | null {
  // Match `#` followed by any non-whitespace (just input detection, not tag parsing)
  const word = context.matchBefore(/#\S*/);
  if (!word) return null;

  // Don't trigger on just `#` without explicit activation
  if (word.text === "#" && !context.explicit) return null;

  const colors = context.state.facet(tagAutocompleteFacet);
  const prefix = word.text.slice(1).toLowerCase(); // strip the #

  const options: Completion[] = [];
  for (const [slug, color] of colors) {
    if (prefix && !slug.includes(prefix)) continue;
    options.push({
      label: `#${slug}`,
      detail: slug,
      type: "keyword",
      boost: slug.startsWith(prefix) ? 1 : 0,
      info: () => {
        const dom = document.createElement("span");
        dom.style.display = "inline-flex";
        dom.style.alignItems = "center";
        dom.style.gap = "6px";
        const dot = document.createElement("span");
        dot.style.width = "8px";
        dot.style.height = "8px";
        dot.style.borderRadius = "50%";
        dot.style.backgroundColor = `#${color}`;
        dom.appendChild(dot);
        dom.appendChild(document.createTextNode(slug));
        return dom;
      },
    });
  }

  return {
    from: word.from,
    options,
    filter: false, // We already filtered
  };
}

/**
 * Extension bundle for tag autocomplete.
 * Pass tag colors as a Map<slug, hexColor> (without #).
 */
export function tagAutocomplete(colors: Map<string, string>) {
  return [
    tagAutocompleteFacet.of(colors),
    autocompletion({
      override: [tagCompletionSource],
      activateOnTyping: true,
    }),
  ];
}
