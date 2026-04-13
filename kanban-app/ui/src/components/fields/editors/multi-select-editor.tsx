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
import { invoke } from "@tauri-apps/api/core";
import type { Extension } from "@codemirror/state";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { createMentionDecorations } from "@/lib/cm-mention-decorations";
import type { MentionMeta } from "@/lib/mention-meta";
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

/** Props for the MultiSelectEditor component. */
interface MultiSelectEditorProps extends EditorProps {
  /** Field definition, providing target entity type and commit semantics. */
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

/** Read schema and derive prefix + display field for the target entity type. */
function useMentionConfig(targetEntityType: string | undefined) {
  const { mentionableTypes, loading: schemaLoading } = useSchema();
  const mentionConfig = useMemo(
    () => mentionableTypes.find((mt) => mt.entityType === targetEntityType),
    [mentionableTypes, targetEntityType],
  );
  // "name" is the universal default display field. Specific overrides arrive
  // via mentionConfig.displayField from the YAML schema.
  return {
    mentionConfig,
    schemaLoading,
    prefix: mentionConfig?.prefix ?? "",
    displayField: mentionConfig?.displayField ?? "name",
  };
}

interface EntityMaps {
  idToDisplay: Map<string, string>;
  displayToId: Map<string, string>;
  metaMap: Map<string, MentionMeta>;
}

/** Build ID↔display-name and slug→MentionMeta maps for the target entities. */
function useTargetEntityMaps(
  targetEntityType: string | undefined,
  displayField: string,
): EntityMaps {
  const { getEntities } = useEntityStore();
  const targetEntities = useMemo(
    () => (targetEntityType ? getEntities(targetEntityType) : []),
    [targetEntityType, getEntities],
  );

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

  const metaMap = useMemo(() => {
    const map = new Map<string, MentionMeta>();
    for (const e of targetEntities) {
      const name = getStr(e, displayField);
      if (name) {
        const color = getStr(e, "color", "888888");
        map.set(slugify(name), { color, displayName: name });
      }
    }
    return map;
  }, [targetEntities, displayField]);

  return { idToDisplay, displayToId, metaMap };
}

/** Serialize current `value` into a prefixed-token doc string. */
function useInitialDoc(
  value: unknown,
  idToDisplay: Map<string, string>,
  prefix: string,
): string {
  return useMemo(() => {
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
}

interface CommitHandlers {
  commit: () => void;
  handleChange: (text: string) => void;
  submitRef: React.MutableRefObject<(() => void) | null>;
  escapeRef: React.MutableRefObject<(() => void) | null>;
}

/** Parse text and compute the final commit value from the current maps. */
function computeCommitValue(
  text: string,
  prefix: string,
  displayToId: Map<string, string>,
  idToDisplay: Map<string, string>,
  commitDisplayNames: boolean,
): string[] {
  const ids = parseDocTokens(text, prefix, displayToId, commitDisplayNames);
  return commitDisplayNames
    ? ids.map((id) => idToDisplay.get(id) ?? id)
    : ids;
}

/** Build submit/escape refs used by buildSubmitCancelExtensions. */
function useSubmitEscapeRefs(
  commit: () => void,
  onCancel: () => void,
  mode: string,
) {
  const commitRef = useRef(commit);
  commitRef.current = commit;
  const cancelRef = useRef(onCancel);
  cancelRef.current = onCancel;

  const submitRef = useRef<(() => void) | null>(null);
  submitRef.current = () => commitRef.current();

  const escapeRef = useRef<(() => void) | null>(null);
  escapeRef.current =
    mode === "vim" ? () => commitRef.current() : () => cancelRef.current();

  return { submitRef, escapeRef };
}

/** Wire commit/cancel callbacks and submit/escape refs. */
function useCommitHandlers(
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
  maps: EntityMaps,
  prefix: string,
  commitDisplayNames: boolean,
  mode: string,
  onCommit: (value: unknown) => void,
  onCancel: () => void,
  onChange: ((value: unknown) => void) | undefined,
): CommitHandlers {
  const displayToIdRef = useRef(maps.displayToId);
  displayToIdRef.current = maps.displayToId;
  const idToDisplayRef = useRef(maps.idToDisplay);
  idToDisplayRef.current = maps.idToDisplay;

  const commit = useCallback(() => {
    const text = editorRef.current?.view?.state.doc.toString().trim() ?? "";
    onCommit(
      computeCommitValue(
        text,
        prefix,
        displayToIdRef.current,
        idToDisplayRef.current,
        commitDisplayNames,
      ),
    );
  }, [editorRef, onCommit, prefix, commitDisplayNames]);

  const { submitRef, escapeRef } = useSubmitEscapeRefs(commit, onCancel, mode);

  const handleChange = useCallback(
    (text: string) => {
      if (!onChange) return;
      onChange(
        computeCommitValue(
          text.trim(),
          prefix,
          displayToIdRef.current,
          idToDisplayRef.current,
          commitDisplayNames,
        ),
      );
    },
    [onChange, prefix, commitDisplayNames],
  );

  return { commit, handleChange, submitRef, escapeRef };
}

/** Async autocomplete search — excludes tokens already present in the doc. */
function useMentionSearch(
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
  targetEntityType: string | undefined,
  prefix: string,
): ((query: string) => Promise<MentionSearchResult[]>) | null {
  return useMemo(() => {
    if (!targetEntityType) return null;
    const rawSearch = async (query: string): Promise<MentionSearchResult[]> => {
      try {
        const results = await invoke<
          Array<{ id: string; display_name: string; color: string }>
        >("search_mentions", { entityType: targetEntityType, query });
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
  }, [editorRef, targetEntityType, prefix]);
}

/** Build the full CM6 extension array for the editor. */
function useEditorExtensions(
  mode: string,
  prefix: string,
  metaMap: Map<string, MentionMeta>,
  searchFn: ((query: string) => Promise<MentionSearchResult[]>) | null,
  submitRef: React.MutableRefObject<(() => void) | null>,
  escapeRef: React.MutableRefObject<(() => void) | null>,
): Extension[] {
  const mentionDeco = useMemo(() => {
    return createMentionDecorations(prefix, "cm-multiselect-pill", "--pill-color");
  }, [prefix]);

  return useMemo(() => {
    const exts: Extension[] = [keymapExtension(mode)];

    if (prefix) {
      exts.push(...mentionDeco.extension(metaMap));
    }

    if (searchFn && prefix) {
      const source = createMentionCompletionSource(prefix, searchFn);
      exts.push(createMentionAutocomplete([source]));
    }

    exts.push(
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: submitRef,
        onCancelRef: escapeRef,
        singleLine: true,
      }),
    );

    return exts;
  }, [mode, searchFn, prefix, mentionDeco, metaMap, submitRef, escapeRef]);
}

/** Focus editor on mount and place cursor at end of doc. */
function useAutoFocusEditor(
  editorRef: React.RefObject<ReactCodeMirrorRef | null>,
) {
  useEffect(() => {
    setTimeout(() => {
      const view = editorRef.current?.view;
      if (view) {
        view.focus();
        view.dispatch({ selection: { anchor: view.state.doc.length } });
      }
    }, 0);
  }, [editorRef]);
}

/** Static `basicSetup` configuration — disables all CM6 chrome. */
const MULTI_SELECT_BASIC_SETUP = {
  lineNumbers: false,
  foldGutter: false,
  highlightActiveLine: false,
  highlightActiveLineGutter: false,
  indentOnInput: false,
  bracketMatching: false,
  autocompletion: false,
} as const;

/** Aggregated state returned by `useMultiSelectEditorState`. */
interface MultiSelectEditorState {
  /** Ref forwarded to the underlying CodeMirror component. */
  editorRef: React.RefObject<ReactCodeMirrorRef | null>;
  /** Initial document text built from the incoming `value`. */
  initialDoc: string;
  /** CM6 extension array assembled from keymap, decorations, and autocomplete. */
  extensions: ReturnType<typeof useEditorExtensions>;
  /** onChange handler that parses tokens and calls `onChange`. */
  handleChange: (text: string) => void;
  /** Blur handler that commits after a short debounce. */
  handleBlur: () => void;
  /** Mention prefix character (e.g. `#`, `@`) used in placeholder text. */
  prefix: string;
  /** False while the schema is still loading for a known target entity type. */
  shouldRender: boolean;
}

/** Compose every hook the editor needs into a single state object. */
function useMultiSelectEditorState(
  props: MultiSelectEditorProps,
): MultiSelectEditorState {
  const { field, value, onCommit, onCancel, onChange } = props;
  const { keymap_mode: mode } = useUIState();
  const editorRef = useRef<ReactCodeMirrorRef>(null);

  const targetEntityType = field.type.entity as string | undefined;
  const commitDisplayNames = !!(field.type as Record<string, unknown>)
    .commit_display_names;

  const { mentionConfig, schemaLoading, prefix, displayField } =
    useMentionConfig(targetEntityType);
  const maps = useTargetEntityMaps(targetEntityType, displayField);
  const initialDoc = useInitialDoc(value, maps.idToDisplay, prefix);
  const { commit, handleChange, submitRef, escapeRef } = useCommitHandlers(
    editorRef,
    maps,
    prefix,
    commitDisplayNames,
    mode,
    onCommit,
    onCancel,
    onChange,
  );
  const searchFn = useMentionSearch(editorRef, targetEntityType, prefix);
  const extensions = useEditorExtensions(
    mode,
    prefix,
    maps.metaMap,
    searchFn,
    submitRef,
    escapeRef,
  );

  useAutoFocusEditor(editorRef);

  const handleBlur = useCallback(() => {
    setTimeout(() => commit(), 100);
  }, [commit]);

  const shouldRender = !(schemaLoading && !mentionConfig && targetEntityType);

  return {
    editorRef,
    initialDoc,
    extensions,
    handleChange,
    handleBlur,
    prefix,
    shouldRender,
  };
}

/**
 * CM6-based multi-select editor for reference and computed tag fields.
 *
 * The document IS the selection: selected items appear as prefixed tokens
 * (e.g. `#bug #feature`) decorated as inline colored pills. Typing after
 * the last token triggers prefix autocomplete against `search_mentions`.
 * Commit serializes the tokens back to entity IDs (or slugs for tag
 * fields with `commit_display_names`).
 */
export function MultiSelectEditor(props: MultiSelectEditorProps) {
  const state = useMultiSelectEditorState(props);

  if (!state.shouldRender) return null;

  return (
    <CodeMirror
      ref={state.editorRef}
      value={state.initialDoc}
      extensions={state.extensions}
      theme={shadcnTheme}
      onBlur={state.handleBlur}
      onChange={state.handleChange}
      basicSetup={MULTI_SELECT_BASIC_SETUP}
      placeholder={`Type ${state.prefix} to search...`}
      className="text-sm"
    />
  );
}
