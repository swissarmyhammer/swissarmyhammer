/**
 * CM6-based single-select editor for scalar reference fields.
 *
 * Mirrors {@link MultiSelectEditor} so every mention-style editor in the app
 * shares one CM6 foundation (decorations, autocomplete, pill widgets,
 * submit/cancel semantics). Differences vs. multi-select:
 *
 * - Doc holds **at most one** mention token (e.g. `$alpha`).
 * - Autocomplete `apply` replaces the whole doc, never appends — keeps the
 *   single-token invariant visual, not just enforced at commit time.
 * - Commits a scalar `string | null` (null when the doc is empty), never an
 *   array — the field's `type.multiple` is `false`.
 * - If multiple tokens somehow end up in the doc (paste, weird input) only
 *   the **last resolved** token is kept on commit.
 *
 * Used by `SelectEditorAdapter` for fields with `field.type.entity` set
 * (currently `project` and `position_column`). Static-options fields still
 * route to the shadcn-based `SelectEditor`.
 */

import { useCallback, useEffect, useMemo, useRef } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { invoke } from "@tauri-apps/api/core";
import { EditorState, type Extension } from "@codemirror/state";
import {
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import type { EditorView } from "@codemirror/view";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useUIState } from "@/lib/ui-state-context";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { createMentionDecorations } from "@/lib/cm-mention-decorations";
import type { MentionMeta } from "@/lib/mention-meta";
import {
  createMentionAutocomplete,
  type MentionSearchResult,
} from "@/lib/cm-mention-autocomplete";
import { createDebouncedSearch } from "@/lib/debounced-search";
import { slugify } from "@/lib/slugify";
import { getStr } from "@/types/kanban";
import type { EditorProps } from ".";

/** Debounce for autocomplete search — matches MultiSelectEditor for UX parity. */
const SEARCH_DEBOUNCE_MS = 150;

/** Delay after blur before committing; gives click-in-popover time to refocus. */
const BLUR_COMMIT_DEBOUNCE_MS = 100;

/** Fallback pill color when an entity has no `color` field set. */
const DEFAULT_PILL_COLOR_HEX = "888888";

/** Pixel dimensions for the color dot rendered in the autocomplete info panel. */
const INFO_DOT_SIZE_PX = 8;

/** Gap between the color dot and the display name in the info panel. */
const INFO_GAP_PX = 6;

/**
 * Parse the doc text into at most one resolved entity id.
 *
 * Splits on the mention prefix and walks the tokens; the **last** token that
 * resolves against `displayToId` wins. A paste like `"$alpha $beta"` therefore
 * commits `proj-beta`, matching the behavior of a user replacing the prior
 * selection. Returns `null` when no token resolves or the doc is empty.
 */
function parseLastDocToken(
  text: string,
  prefix: string,
  displayToId: Map<string, string>,
): string | null {
  const tokens = prefix
    ? text
        .split(prefix)
        .map((t) => t.trim())
        .filter(Boolean)
    : [text.trim()].filter(Boolean);

  let lastId: string | null = null;
  for (const token of tokens) {
    const slug = slugify(token);
    if (!slug) continue;
    const id =
      displayToId.get(slug) ?? displayToId.get(token.toLowerCase()) ?? null;
    if (id) lastId = id;
  }
  return lastId;
}

/** Read schema and derive prefix + display field for the target entity type. */
function useMentionConfig(targetEntityType: string | undefined) {
  const { mentionableTypes, loading: schemaLoading } = useSchema();
  const mentionConfig = useMemo(
    () => mentionableTypes.find((mt) => mt.entityType === targetEntityType),
    [mentionableTypes, targetEntityType],
  );
  // "name" is the universal default display field; specific overrides arrive
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
        const color = getStr(e, "color", DEFAULT_PILL_COLOR_HEX);
        map.set(slugify(name), { color, displayName: name });
      }
    }
    return map;
  }, [targetEntities, displayField]);

  return { idToDisplay, displayToId, metaMap };
}

/**
 * Serialize current `value` into a single-token doc string.
 *
 * Scalar `string` → `"${prefix}${slug} "`; empty/non-string → `""`. The
 * trailing space mirrors {@link MultiSelectEditor} so the caret lands past
 * the token when the editor mounts.
 */
function useInitialDoc(
  value: unknown,
  idToDisplay: Map<string, string>,
  prefix: string,
): string {
  return useMemo(() => {
    if (typeof value !== "string" || value.length === 0) return "";
    const display = idToDisplay.get(value) ?? value;
    return `${prefix}${display} `;
  }, [value, idToDisplay, prefix]);
}

interface CommitHandlers {
  commit: () => void;
  handleChange: (text: string) => void;
  submitRef: React.MutableRefObject<(() => void) | null>;
  escapeRef: React.MutableRefObject<(() => void) | null>;
}

/** Parse text and compute the scalar commit value from the current maps. */
function computeCommitValue(
  text: string,
  prefix: string,
  displayToId: Map<string, string>,
): string | null {
  return parseLastDocToken(text, prefix, displayToId);
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
  mode: string,
  onCommit: (value: unknown) => void,
  onCancel: () => void,
  onChange: ((value: unknown) => void) | undefined,
): CommitHandlers {
  const displayToIdRef = useRef(maps.displayToId);
  displayToIdRef.current = maps.displayToId;

  const commit = useCallback(() => {
    const text = editorRef.current?.view?.state.doc.toString().trim() ?? "";
    onCommit(computeCommitValue(text, prefix, displayToIdRef.current));
  }, [editorRef, onCommit, prefix]);

  const { submitRef, escapeRef } = useSubmitEscapeRefs(commit, onCancel, mode);

  const handleChange = useCallback(
    (text: string) => {
      if (!onChange) return;
      onChange(computeCommitValue(text.trim(), prefix, displayToIdRef.current));
    },
    [onChange, prefix],
  );

  return { commit, handleChange, submitRef, escapeRef };
}

/**
 * Build a completion source whose `apply` replaces the whole doc with the
 * accepted token, preserving the single-token invariant visually.
 *
 * This is the critical behavioral difference vs. {@link MultiSelectEditor}.
 * A multi-select autocomplete apply replaces only the matched prefix (appending
 * to any existing tokens). Here, every suggestion selection rewrites the doc
 * to `${prefix}${slug} ` so the user can never end up with two tokens.
 */
// Replace the WHOLE doc rather than just the matched prefix. This is what makes
// the single-token invariant visual: no matter the state of the doc (empty,
// partial typing, a stale token from a prior paste), accepting a suggestion
// leaves exactly one token.
function buildSingleSelectApply(applyText: string) {
  return (view: EditorView) => {
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: applyText },
      selection: { anchor: applyText.length },
    });
  };
}

function buildSingleSelectInfo(displayName: string, color: string) {
  return () => {
    const dom = document.createElement("span");
    dom.style.display = "inline-flex";
    dom.style.alignItems = "center";
    dom.style.gap = `${INFO_GAP_PX}px`;
    const dot = document.createElement("span");
    dot.style.width = `${INFO_DOT_SIZE_PX}px`;
    dot.style.height = `${INFO_DOT_SIZE_PX}px`;
    dot.style.borderRadius = "50%";
    dot.style.backgroundColor = `#${color}`;
    dom.appendChild(dot);
    dom.appendChild(document.createTextNode(displayName));
    return dom;
  };
}

function toSingleSelectCompletion(
  r: MentionSearchResult,
  prefix: string,
  query: string,
): Completion {
  return {
    label: `${prefix}${r.displayName}`,
    detail: r.slug,
    type: "keyword",
    boost: r.slug.startsWith(query) ? 1 : 0,
    apply: buildSingleSelectApply(`${prefix}${r.slug} `),
    info: buildSingleSelectInfo(r.displayName, r.color),
  };
}

function createSingleSelectCompletionSource(
  prefix: string,
  search: (query: string) => Promise<MentionSearchResult[]>,
): (
  context: CompletionContext,
) => CompletionResult | null | Promise<CompletionResult | null> {
  const prefixRegex = new RegExp(`\\${prefix}\\S*`);

  return (context: CompletionContext) => {
    const word = context.matchBefore(prefixRegex);
    if (!word) return null;
    if (word.text === prefix && !context.explicit) return null;

    const query = word.text.slice(prefix.length).toLowerCase();
    const from = word.from;

    const buildResult = (results: MentionSearchResult[]): CompletionResult => ({
      from,
      options: results.map((r) => toSingleSelectCompletion(r, prefix, query)),
      filter: false,
    });

    const result = search(query);
    return result instanceof Promise
      ? result.then(buildResult)
      : buildResult(result);
  };
}

/** Async autocomplete search against the Tauri `search_mentions` command. */
function useMentionSearch(
  targetEntityType: string | undefined,
): ((query: string) => Promise<MentionSearchResult[]>) | null {
  return useMemo(() => {
    if (!targetEntityType) return null;
    const rawSearch = async (query: string): Promise<MentionSearchResult[]> => {
      try {
        const results = await invoke<
          Array<{ id: string; display_name: string; color: string }>
        >("search_mentions", { entityType: targetEntityType, query });
        return results.map((r) => ({
          slug: slugify(r.display_name),
          displayName: r.display_name,
          color: r.color,
        }));
      } catch {
        return [];
      }
    };
    return createDebouncedSearch({
      search: rawSearch,
      delayMs: SEARCH_DEBOUNCE_MS,
    });
  }, [targetEntityType]);
}

/**
 * Transaction filter that rewrites any `\r\n`, `\n`, or `\r` inside inserted
 * text to a single space so the doc remains structurally single-line.
 *
 * Identical in spirit to {@link MultiSelectEditor}'s filter: pastes like
 * `"$alpha\n$beta"` stay resolvable because the newline becomes a space,
 * and the last-token-wins rule in {@link parseLastDocToken} then commits
 * only `$beta`.
 */
const SINGLE_LINE_TRANSACTION_FILTER = EditorState.transactionFilter.of(
  (tr) => {
    if (!tr.docChanged) return tr;

    const sanitizedChanges: { from: number; to: number; insert: string }[] = [];
    let hasNewline = false;
    tr.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
      const text = inserted.toString();
      if (text.includes("\n") || text.includes("\r")) {
        hasNewline = true;
        sanitizedChanges.push({
          from: fromA,
          to: toA,
          insert: text.replace(/\r\n|\r|\n/g, " "),
        });
      } else {
        sanitizedChanges.push({ from: fromA, to: toA, insert: text });
      }
    });

    if (!hasNewline) return tr;

    return [
      {
        changes: sanitizedChanges,
        effects: tr.effects,
        scrollIntoView: tr.scrollIntoView,
      },
    ];
  },
);

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
    return createMentionDecorations(
      prefix,
      "cm-singleselect-pill",
      "--pill-color",
    );
  }, [prefix]);

  return useMemo(() => {
    const exts: Extension[] = [
      keymapExtension(mode),
      SINGLE_LINE_TRANSACTION_FILTER,
    ];

    if (prefix) {
      exts.push(...mentionDeco.extension(metaMap));
    }

    if (searchFn && prefix) {
      const source = createSingleSelectCompletionSource(prefix, searchFn);
      exts.push(createMentionAutocomplete([source]));
    }

    // singleLine + alwaysSubmitOnEnter together guarantee Enter commits in
    // every keymap mode (cua, emacs, vim-normal, vim-insert). Autocomplete
    // Enter is still preserved via the `completionStatus` guard inside
    // buildSubmitCancelExtensions.
    exts.push(
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: submitRef,
        onCancelRef: escapeRef,
        singleLine: true,
        alwaysSubmitOnEnter: true,
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
const SINGLE_SELECT_BASIC_SETUP = {
  lineNumbers: false,
  foldGutter: false,
  highlightActiveLine: false,
  highlightActiveLineGutter: false,
  indentOnInput: false,
  bracketMatching: false,
  autocompletion: false,
} as const;

/** Aggregated state returned by `useSingleSelectEditorState`. */
interface SingleSelectEditorState {
  /** Ref forwarded to the underlying CodeMirror component. */
  editorRef: React.RefObject<ReactCodeMirrorRef | null>;
  /** Initial document text built from the incoming `value`. */
  initialDoc: string;
  /** CM6 extension array assembled from keymap, decorations, and autocomplete. */
  extensions: ReturnType<typeof useEditorExtensions>;
  /** onChange handler that parses the single token and calls `onChange`. */
  handleChange: (text: string) => void;
  /** Blur handler that commits after a short debounce. */
  handleBlur: () => void;
  /** Mention prefix character (e.g. `$`, `%`) used in placeholder text. */
  prefix: string;
  /** False while the schema is still loading for a known target entity type. */
  shouldRender: boolean;
}

/** Compose every hook the editor needs into a single state object. */
function useSingleSelectEditorState(
  props: EditorProps,
): SingleSelectEditorState {
  const { field, value, onCommit, onCancel, onChange } = props;
  const { keymap_mode: mode } = useUIState();
  const editorRef = useRef<ReactCodeMirrorRef>(null);

  const targetEntityType = field.type.entity as string | undefined;

  const { mentionConfig, schemaLoading, prefix, displayField } =
    useMentionConfig(targetEntityType);
  const maps = useTargetEntityMaps(targetEntityType, displayField);
  const initialDoc = useInitialDoc(value, maps.idToDisplay, prefix);
  const { commit, handleChange, submitRef, escapeRef } = useCommitHandlers(
    editorRef,
    maps,
    prefix,
    mode,
    onCommit,
    onCancel,
    onChange,
  );
  const searchFn = useMentionSearch(targetEntityType);
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
    setTimeout(() => commit(), BLUR_COMMIT_DEBOUNCE_MS);
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
 * CM6-based single-select editor for scalar reference fields.
 *
 * The document IS the selection — at most one prefixed token (e.g. `$alpha`)
 * decorated as an inline colored pill via {@link createMentionDecorations}.
 * Typing the entity's `mention_prefix` triggers autocomplete against
 * `search_mentions`; accepting a suggestion replaces the whole doc, keeping
 * the single-token invariant visible. Commit serializes the token back to
 * its entity id (or `null` when the doc is empty).
 */
export function SingleSelectEditor(props: EditorProps) {
  const state = useSingleSelectEditorState(props);

  if (!state.shouldRender) return null;

  return (
    <CodeMirror
      ref={state.editorRef}
      value={state.initialDoc}
      extensions={state.extensions}
      theme={shadcnTheme}
      onBlur={state.handleBlur}
      onChange={state.handleChange}
      basicSetup={SINGLE_SELECT_BASIC_SETUP}
      placeholder={`Type ${state.prefix} to search...`}
      className="text-sm"
    />
  );
}
