/**
 * CodeMirror 6 language extension for the kanban filter DSL.
 *
 * Provides syntax highlighting, bracket matching, and error recovery for
 * filter expressions like `#bug && @will || !#done`.
 */

import { LRLanguage, LanguageSupport } from "@codemirror/language";
import { parser } from "./parser";

/** The LR language definition for the filter DSL. */
export const filterLRLanguage = LRLanguage.define({
  name: "filter",
  parser,
  languageData: {
    closeBrackets: { brackets: ["("] },
  },
});

/**
 * Create a CM6 LanguageSupport extension for the filter DSL.
 *
 * Usage:
 * ```ts
 * import { filterLanguage } from "@/lang-filter";
 * const extensions = [filterLanguage()];
 * ```
 */
export function filterLanguage(): LanguageSupport {
  return new LanguageSupport(filterLRLanguage);
}
