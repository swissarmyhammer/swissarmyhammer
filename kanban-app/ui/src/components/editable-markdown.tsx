import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { EditorView } from "@codemirror/view";
import { Compartment, type Extension } from "@codemirror/state";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { getCM, Vim } from "@replit/codemirror-vim";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { invoke } from "@tauri-apps/api/core";
import { useKeymap } from "@/lib/keymap-context";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { buildSubmitCancelExtensions } from "@/lib/cm-submit-cancel";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { createMentionDecorations } from "@/lib/cm-mention-decorations";
import {
  createMentionCompletionSource,
  createMentionAutocomplete,
  type MentionSearchResult,
} from "@/lib/cm-mention-autocomplete";
import { createDebouncedSearch } from "@/lib/debounced-search";
import {
  createMentionTooltips,
  type MentionMeta,
} from "@/lib/cm-mention-tooltip";
import { remarkMentions } from "@/lib/remark-mentions";
import { MentionPill } from "@/components/mention-pill";
import { slugify } from "@/lib/slugify";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

interface EditableMarkdownProps {
  value: string;
  onCommit: (value: string) => void;
  className?: string;
  inputClassName?: string;
  multiline?: boolean;
  placeholder?: string;
  /** @deprecated Tag entities — now read from context automatically for multiline editors. */
  tags?: Entity[];
}

/** Regex matching a GFM task list checkbox in markdown source */
const CHECKBOX_RE = /- \[([ xX])\]/g;

/**
 * Toggle the Nth checkbox in a markdown string.
 * Returns the updated string or null if the index is out of range.
 */
function toggleCheckbox(source: string, index: number): string | null {
  let count = 0;
  return source.replace(CHECKBOX_RE, (match, check) => {
    if (count++ === index) {
      return check === " " ? "- [x]" : "- [ ]";
    }
    return match;
  });
}

// ---------------------------------------------------------------------------
// Pre-built mention infrastructure per entity type (created once per prefix).
// Keyed by "prefix:entityType" — bounded by schema-defined mentionable types.
// Size cap prevents unbounded growth if entity types are ever dynamic.
// ---------------------------------------------------------------------------

const INFRA_CACHE_LIMIT = 20;

const mentionInfra = new Map<
  string,
  ReturnType<typeof createMentionDecorations>
>();
const tooltipInfra = new Map<
  string,
  ReturnType<typeof createMentionTooltips>
>();

/** Get or create decoration infrastructure, clearing cache if it exceeds the size cap. */
function getDecoInfra(prefix: string, entityType: string) {
  const key = `${prefix}:${entityType}`;
  if (!mentionInfra.has(key)) {
    if (mentionInfra.size >= INFRA_CACHE_LIMIT) mentionInfra.clear();
    const cssClass = `cm-${entityType}-pill`;
    const colorVar = `--${entityType}-color`;
    mentionInfra.set(key, createMentionDecorations(prefix, cssClass, colorVar));
  }
  return mentionInfra.get(key)!;
}

/** Get or create tooltip infrastructure, clearing cache if it exceeds the size cap. */
function getTooltipInfra(prefix: string, entityType: string) {
  const key = `${prefix}:${entityType}`;
  if (!tooltipInfra.has(key)) {
    if (tooltipInfra.size >= INFRA_CACHE_LIMIT) tooltipInfra.clear();
    const cssClass = `cm-${entityType}-tooltip`;
    tooltipInfra.set(key, createMentionTooltips(prefix, cssClass));
  }
  return tooltipInfra.get(key)!;
}

/** Build a slug→color map for a mentionable entity type. Slugifies keys for fields with spaces. */
function buildColorMap(
  entities: Entity[],
  displayField: string,
): Map<string, string> {
  const map = new Map<string, string>();
  for (const e of entities) {
    const raw = getStr(e, displayField);
    const color = getStr(e, "color", "888888");
    if (raw) map.set(slugify(raw), color);
  }
  return map;
}

/** Build a slug→meta map for tooltips. Slugifies keys for fields with spaces. */
function buildMetaMap(
  entities: Entity[],
  displayField: string,
): Map<string, MentionMeta> {
  const map = new Map<string, MentionMeta>();
  for (const e of entities) {
    const raw = getStr(e, displayField);
    const color = getStr(e, "color", "888888");
    const description = getStr(e, "description") || undefined;
    if (raw) map.set(slugify(raw), { color, description });
  }
  return map;
}

/** Build a debounced async search function that calls the Tauri backend. Slugifies display names. */
function buildAsyncSearch(
  entityType: string,
): (query: string) => Promise<MentionSearchResult[]> {
  const rawSearch = async (query: string): Promise<MentionSearchResult[]> => {
    try {
      const results = await invoke<
        Array<{ id: string; display_name: string; color: string }>
      >("search_mentions", { entityType, query });
      return results.map((r) => ({
        slug: slugify(r.display_name),
        displayName: r.display_name,
        color: r.color,
      }));
    } catch {
      return [];
    }
  };

  return createDebouncedSearch({ search: rawSearch, delayMs: 150 });
}

export function EditableMarkdown({
  value,
  onCommit,
  className,
  inputClassName,
  multiline,
  placeholder,
  tags: _legacyTags,
}: EditableMarkdownProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const clickCoordsRef = useRef<{ x: number; y: number } | null>(null);
  const keymapCompartment = useRef(new Compartment());
  const { mode } = useKeymap();
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  // Keep draft in sync when value changes externally
  useEffect(() => {
    if (!editing) setDraft(value);
  }, [value, editing]);

  // Guard against re-entrant commits (blur fires after Escape unmounts CM)
  const committedRef = useRef(false);
  useEffect(() => {
    if (editing) committedRef.current = false;
  }, [editing]);

  // Save + exit the editor
  const commitAndExit = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    const text = editorRef.current?.view
      ? editorRef.current.view.state.doc.toString().trim()
      : draft.trim();
    setEditing(false);
    if (text !== value) {
      onCommit(text);
    }
  }, [draft, value, onCommit]);

  const commitAndExitRef = useRef(commitAndExit);
  commitAndExitRef.current = commitAndExit;

  // Save current value without leaving the editor (vim insert→normal)
  const saveInPlace = useCallback(() => {
    if (!editorRef.current?.view) return;
    const text = editorRef.current.view.state.doc.toString().trim();
    if (text !== value) {
      onCommit(text);
    }
  }, [value, onCommit]);
  const saveInPlaceRef = useRef(saveInPlace);
  saveInPlaceRef.current = saveInPlace;

  // Hot-swap keymap only when mode actually changes while editor is open
  const prevModeRef = useRef<string | null>(null);
  useEffect(() => {
    if (!editing || !editorRef.current?.view) {
      prevModeRef.current = null;
      return;
    }
    if (prevModeRef.current !== null && prevModeRef.current !== mode) {
      editorRef.current.view.dispatch({
        effects: keymapCompartment.current.reconfigure(keymapExtension(mode)),
      });
    }
    prevModeRef.current = mode;
  }, [mode, editing]);

  // After editor mounts, ensure vim normal mode and position cursor at click location
  const handleCreateEditor = useCallback(
    (view: EditorView) => {
      if (mode === "vim") {
        const cm = getCM(view);
        if (cm?.state?.vim?.insertMode) {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          Vim.exitInsertMode(cm as any);
        }
      }

      const coords = clickCoordsRef.current;
      clickCoordsRef.current = null;
      if (coords) {
        try {
          const pos = view.posAtCoords(coords);
          if (pos !== null) {
            view.dispatch({ selection: { anchor: pos } });
          }
        } catch {
          // No layout available
        }
      }
    },
    [mode],
  );

  const displayRef = useRef<HTMLDivElement>(null);

  const handleCheckboxChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (!displayRef.current) return;
      const all = displayRef.current.querySelectorAll('input[type="checkbox"]');
      const idx = Array.from(all).indexOf(e.target);
      if (idx >= 0) {
        const updated = toggleCheckbox(value, idx);
        if (updated !== null) onCommit(updated);
      }
    },
    [value, onCommit],
  );

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      clickCoordsRef.current = { x: e.clientX, y: e.clientY };
      setDraft(value);
      setEditing(true);
    },
    [value],
  );

  // Build mention data for all mentionable types from context
  const mentionData = useMemo(() => {
    if (!multiline) return [];
    return mentionableTypes.map((mt) => {
      const entities = getEntities(mt.entityType);
      return {
        ...mt,
        entities,
        colorMap: buildColorMap(entities, mt.displayField),
        metaMap: buildMetaMap(entities, mt.displayField),
        slugs: entities
          .map((e) => getStr(e, mt.displayField))
          .filter(Boolean)
          .map(slugify),
      };
    });
  }, [multiline, mentionableTypes, getEntities]);

  // Build CM6 extensions for all mention types.
  // IMPORTANT: All completion sources must be collected into a single
  // autocompletion() call to avoid CM6 "Config merge conflict for field override".
  const mentionExtensions = useMemo((): Extension[] => {
    if (!multiline) return [];
    const exts: Extension[] = [];
    const completionSources: Array<
      ReturnType<typeof createMentionCompletionSource>
    > = [];
    for (const md of mentionData) {
      if (md.colorMap.size === 0) continue;
      const decoInfra = getDecoInfra(md.prefix, md.entityType);
      exts.push(decoInfra.extension(md.colorMap));
      completionSources.push(
        createMentionCompletionSource(
          md.prefix,
          buildAsyncSearch(md.entityType),
        ),
      );
      const tooltipInfraInstance = getTooltipInfra(md.prefix, md.entityType);
      exts.push(tooltipInfraInstance.extension(md.metaMap));
    }
    if (completionSources.length > 0) {
      exts.push(createMentionAutocomplete(completionSources));
    }
    return exts;
  }, [multiline, mentionData]);

  // Semantic submit/cancel refs — EditableMarkdown always commits on both
  const semanticSubmitRef = useRef<(() => void) | null>(null);
  semanticSubmitRef.current = () => commitAndExitRef.current();
  const semanticCancelRef = useRef<(() => void) | null>(null);
  semanticCancelRef.current = () => commitAndExitRef.current();

  const extensions = useMemo(
    () => [
      keymapCompartment.current.of(keymapExtension(mode)),
      EditorView.lineWrapping,
      ...(multiline
        ? [markdown({ base: markdownLanguage, codeLanguages: languages })]
        : []),
      ...buildSubmitCancelExtensions({
        mode,
        onSubmitRef: semanticSubmitRef,
        onCancelRef: semanticCancelRef,
        saveInPlaceRef,
        singleLine: !multiline,
      }),
      ...mentionExtensions,
    ],
    [mode, multiline, mentionExtensions],
  );

  // Build remark plugins for all mentionable types (must be before early return)
  const remarkPlugins = useMemo(() => {
    const plugins: Array<ReturnType<typeof remarkMentions> | typeof remarkGfm> =
      [remarkGfm];
    for (const md of mentionData) {
      if (md.slugs.length === 0) continue;
      plugins.push(
        remarkMentions(
          md.prefix,
          md.slugs,
          `${md.entityType}Pill`,
          `${md.entityType}-pill`,
        ),
      );
    }
    return plugins;
  }, [mentionData]);

  // Build custom components for all mentionable types (must be before early return)
  const mentionComponents = useMemo(() => {
    const comps: Record<string, React.ComponentType> = {};
    for (const md of mentionData) {
      comps[`${md.entityType}-pill`] = (props: { slug?: string }) =>
        (
          <MentionPill
            entityType={md.entityType}
            slug={props.slug ?? ""}
            prefix={md.prefix}
          />
        ) as // eslint-disable-next-line @typescript-eslint/no-explicit-any
        any;
    }
    return comps;
  }, [mentionData]);

  if (editing) {
    return (
      <CodeMirror
        ref={editorRef}
        autoFocus
        value={draft}
        onChange={(val) => setDraft(val)}
        onBlur={commitAndExit}
        onCreateEditor={handleCreateEditor}
        extensions={extensions}
        theme={shadcnTheme}
        basicSetup={{
          lineNumbers: false,
          foldGutter: false,
          highlightActiveLine: false,
          highlightActiveLineGutter: false,
          indentOnInput: !!multiline,
          bracketMatching: false,
          autocompletion: false,
        }}
        className={inputClassName ?? className}
      />
    );
  }

  return (
    <div
      ref={displayRef}
      className={`${className ?? ""} ${value ? "prose prose-sm dark:prose-invert max-w-none" : "text-muted-foreground italic"} cursor-text`}
      onClick={handleClick}
    >
      {value ? (
        <ReactMarkdown
          remarkPlugins={remarkPlugins}
          components={{
            input: (props) => {
              if (props.type === "checkbox") {
                return (
                  <input
                    type="checkbox"
                    checked={props.checked ?? false}
                    onChange={handleCheckboxChange}
                    onClick={(e) => e.stopPropagation()}
                  />
                );
              }
              return <input {...props} />;
            },
            ...(mentionComponents as Record<string, React.ComponentType>),
          }}
        >
          {value}
        </ReactMarkdown>
      ) : (
        placeholder
      )}
    </div>
  );
}
