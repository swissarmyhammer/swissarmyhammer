/**
 * Hook that builds the CM6 `/` slash-command autocomplete extension.
 *
 * The slash-command flavor of the app's shared mention autocomplete: it reuses
 * {@link createMentionCompletionSource} / {@link createMentionAutocomplete}
 * from `lib/cm-mention-autocomplete.ts` (the single completion-source assembly
 * point) with the `/` prefix over a sync search of the live ACP
 * `availableCommands`.
 *
 * Unlike {@link useMentionExtensions}, this hook reads NO React context — it is
 * driven entirely by the passed-in command list. That keeps the AI composer
 * light: it can mount the extension without the schema / entity-store /
 * board-data providers (and without transitively importing them), so the
 * composer stays a thin `TextEditor` host.
 */

import { useMemo } from "react";
import type { Extension } from "@codemirror/state";
import type { AvailableCommand } from "@agentclientprotocol/sdk";
import {
  createMentionCompletionSource,
  createMentionAutocomplete,
  type MentionSearchSync,
} from "@/lib/cm-mention-autocomplete";

/**
 * Build a synchronous slash-command search over a live `availableCommands` list.
 *
 * Filters by case-insensitive substring on the command name — the same filter
 * shape the entity-mention sources use — and maps each match to the
 * command-flavored completion result (`name` + `description`, no color).
 */
export function buildCommandSearch(
  commands: AvailableCommand[],
): MentionSearchSync {
  return (query: string) => {
    const q = query.toLowerCase();
    return commands
      .filter((c) => !q || c.name.toLowerCase().includes(q))
      .map((c) => ({ name: c.name, description: c.description }));
  };
}

/**
 * Build the CM6 autocomplete extension for `/` slash-command completions.
 *
 * Returns an empty array when there are no commands, so typing `/` opens no
 * menu and the composer's plain Enter still submits. The `openOnBarePrefix`
 * option lets a bare `/` list every command — the slash menu's discoverability
 * contract, unlike entity mentions which need a query first.
 */
export function useCommandCompletionExtension(
  commands: AvailableCommand[],
): Extension[] {
  return useMemo(() => {
    if (commands.length === 0) return [];
    const source = createMentionCompletionSource(
      "/",
      buildCommandSearch(commands),
      { openOnBarePrefix: true },
    );
    return [createMentionAutocomplete([source])];
  }, [commands]);
}
