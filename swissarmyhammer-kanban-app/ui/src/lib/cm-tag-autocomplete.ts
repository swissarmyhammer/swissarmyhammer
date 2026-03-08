/**
 * CM6 tag autocomplete — thin wrapper around generic mention autocomplete.
 */

import {
  createMentionAutocomplete,
  syncSearchFromMap,
} from "@/lib/cm-mention-autocomplete";

/**
 * Extension bundle for tag autocomplete.
 * Pass tag colors as a Map<slug, hexColor> (without #).
 */
export function tagAutocomplete(colors: Map<string, string>) {
  return createMentionAutocomplete("#", syncSearchFromMap(colors));
}
