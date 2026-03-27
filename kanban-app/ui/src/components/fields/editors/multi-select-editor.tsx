/**
 * CM6-based multi-select editor for reference and computed tag fields.
 *
 * The document IS the selection — selected items appear as prefixed tokens
 * (e.g. `#bug #feature`) decorated as inline colored pills. Typing after
 * the last token triggers prefix autocomplete. Backspace naturally deletes
 * into the previous token.
 *
 * - Tags (#): unknown slugs are valid — auto-created on commit
 * - Actors (@), Tasks (^): only resolved entity IDs are kept on commit
 */

import { useCallback, useEffect, useMemo, useRef } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap } from "@codemirror/view";
import { Prec } from "@codemirror/state";
import { invoke } from "@tauri-apps/api/core";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { createMentionDecorations } from "@/lib/cm-mention-decorations";
import {
  createMentionCompletionSource,
  createMentionAutocomplete,
  type MentionSearchResult,
} from "@/lib/cm-mention-autocomplete";
import { createDebouncedSearch } from "@/lib/debounced-search";
import { slugify } from "@/lib/slugify";
import { getStr } from "@/types/kanban";
import type { FieldDef, Entity } from "@/types/kanban";
import type { EditorProps } from ".";

interface MultiSelectEditorProps extends EditorProps {
  field: FieldDef;
  /** The entity being edited (optional, for context). */
  entity?: Entity;
}

/** Parse doc text into resolved IDs (and auto-create slugs for tags). */
function parseDocTokens(
  text: string,
  prefix: string,
  displayToId: Map<string, string>,
  commitDisplayNames: boolean,
): string[] {
  const ids: string[] = [];
  const tokens = prefix
    ? text
        .split(prefix)
        .map((t) => t.trim())
        .filter(Boolean)
    : [text.trim()].filter(Boolean);
  for (const token of tokens) {
    const slug = slugify(token);
    if (!slug) continue;
    const id =
      displayToId.get(slug) ??
      displayToId.get(token.toLowerCase()) ??
      (commitDisplayNames ? slug : undefined);
    if (id && !ids.includes(id)) {
      ids.push(id);
    }
  }
  return ids;
}

export function MultiSelectEditor({
  field,
  value,
  onCommit,
  onCancel,
}: MultiSelectEditorProps) {
  const { keymap_mode: mode } = useUIState();
  const { mentionableTypes, loading: schemaLoading } = useSchema();
  const { getEntities } = useEntityStore();
  const editorRef = useRef<ReactCodeMirrorRef>(null);

  // Read target entity and commit mode from field type (set in YAML for both reference and computed fields)
  const targetEntityType = field.type.entity as string | undefined;
  const commitDisplayNames = !!(field.type as Record<string, unknown>)
    .commit_display_names;

  const mentionConfig = useMemo(
    () => mentionableTypes.find((mt) => mt.entityType === targetEntityType),
    [mentionableTypes, targetEntityType],
  );

  const prefix = mentionConfig?.prefix ?? "";
  const displayField = mentionConfig?.displayField ?? "name";

  // Target entities for building maps
  const targetEntities = useMemo(
    () => (targetEntityType ? getEntities(targetEntityType) : []),
    [targetEntityType, getEntities],
  );

  // Maps: ID ↔ display name (slugified)
  const idToDisplay = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of targetEntities) {
      const raw = getStr(e, displayField) || e.id;
      map.set(e.id, slugify(raw));
    }
    return map;
  }, [targetEntities, displayField]);

  const displayToId = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of targetEntities) {
      const name = getStr(e, displayField);
      if (name) {
        map.set(slugify(name), e.id);
        map.set(name.toLowerCase(), e.id);
      }
      map.set(e.id.toLowerCase(), e.id);
    }
    return map;
  }, [targetEntities, displayField]);

  // Build color map for decorations: slug → hex color
  const colorMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of targetEntities) {
      const name = getStr(e, displayField);
      if (name) {
        const color = getStr(e, "color", "888888");
        map.set(slugify(name), color);
      }
    }
    return map;
  }, [targetEntities, displayField]);

  // Initial doc text: existing selections as prefixed tokens
  const initialDoc = useMemo(() => {
    const currentIds: string[] = Array.isArray(value)
      ? value.filter((v): v is string => typeof v === "string")
      : [];
    if (currentIds.length === 0) return "";
    const tokens = currentIds.map((id) => {
      const display = idToDisplay.get(id) ?? id;
      return `${prefix}${display}`;
    });
    return tokens.join(" ") + " ";
  }, [value, idToDisplay, prefix]);

  // Stable refs for commit callback
  const displayToIdRef = useRef(displayToId);
  displayToIdRef.current = displayToId;
  const idToDisplayRef = useRef(idToDisplay);
  idToDisplayRef.current = idToDisplay;

  /** Read doc text, parse tokens, resolve to IDs, commit. */
  const commit = useCallback(() => {
    const text = editorRef.current?.view?.state.doc.toString().trim() ?? "";
    const ids = parseDocTokens(
      text,
      prefix,
      displayToIdRef.current,
      commitDisplayNames,
    );
    const finalValue = commitDisplayNames
      ? ids.map((id) => idToDisplayRef.current.get(id) ?? id)
      : ids;
    onCommit(finalValue);
  }, [onCommit, prefix, commitDisplayNames]);

  const commitRef = useRef(commit);
  commitRef.current = commit;
  const cancelRef = useRef(onCancel);
  cancelRef.current = onCancel;

  // Build async search for autocomplete
  const searchFn = useMemo(() => {
    if (!targetEntityType) return null;
    const rawSearch = async (query: string): Promise<MentionSearchResult[]> => {
      try {
        const results = await invoke<
          Array<{ id: string; display_name: string; color: string }>
        >("search_mentions", { entityType: targetEntityType, query });
        // Filter out items already in the doc
        const docText = editorRef.current?.view?.state.doc.toString() ?? "";
        return results
          .filter((r) => {
            const slug = slugify(r.display_name);
            return !docText.includes(`${prefix}${slug}`);
          })
          .map((r) => ({
            slug: slugify(r.display_name),
            displayName: r.display_name,
            color: r.color,
          }));
      } catch {
        return [];
      }
    };
    return createDebouncedSearch({ search: rawSearch, delayMs: 150 });
  }, [targetEntityType, prefix]);

  // Mention decorations — renders known #slug tokens as colored pills
  const mentionDeco = useMemo(() => {
    const cssClass = `cm-multiselect-pill`;
    const colorVar = `--pill-color`;
    const deco = createMentionDecorations(prefix, cssClass, colorVar);
    return deco;
  }, [prefix]);

  // CM6 extensions
  const extensions = useMemo(() => {
    const exts = [keymapExtension(mode)];

    // Mention pill decorations
    if (prefix) {
      exts.push(...mentionDeco.extension(colorMap));
    }

    if (searchFn && prefix) {
      const source = createMentionCompletionSource(prefix, searchFn);
      exts.push(createMentionAutocomplete([source]));
    }

    // Enter always commits. Escape: vim saves, CUA/emacs discards.
    exts.push(
      Prec.highest(
        keymap.of([
          {
            key: "Enter",
            run: () => {
              commitRef.current();
              return true;
            },
          },
          {
            key: "Escape",
            run: () => {
              if (mode === "vim") {
                commitRef.current();
              } else {
                cancelRef.current();
              }
              return true;
            },
          },
        ]),
      ),
    );

    return exts;
  }, [mode, searchFn, prefix, mentionDeco, colorMap]);

  // Focus editor on mount, place cursor at end
  useEffect(() => {
    setTimeout(() => {
      const view = editorRef.current?.view;
      if (view) {
        view.focus();
        view.dispatch({ selection: { anchor: view.state.doc.length } });
      }
    }, 0);
  }, []);

  // Blur handler — commit on focus loss
  const handleBlur = useCallback(() => {
    setTimeout(() => {
      commitRef.current();
    }, 100);
  }, []);

  // Wait for schema to load so prefix and displayField are correct before
  // CM6 initializes. Without this, the editor mounts with empty prefix and
  // wrong display names, producing "alice " instead of "@alice ".
  // Only wait if the schema is still loading — some entity types (e.g.
  // attachment) don't have a mention_prefix and that's fine.
  if (schemaLoading && !mentionConfig && targetEntityType) {
    return null;
  }

  return (
    <CodeMirror
      ref={editorRef}
      value={initialDoc}
      extensions={extensions}
      theme={shadcnTheme}
      onBlur={handleBlur}
      basicSetup={{
        lineNumbers: false,
        foldGutter: false,
        highlightActiveLine: false,
        highlightActiveLineGutter: false,
        indentOnInput: false,
        bracketMatching: false,
        autocompletion: false,
      }}
      placeholder={`Type ${prefix} to search...`}
      className="text-sm"
    />
  );
}
