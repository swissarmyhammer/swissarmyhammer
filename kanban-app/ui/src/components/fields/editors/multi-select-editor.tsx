/**
 * CM6-based multi-select editor for reference and computed tag fields.
 *
 * Works for any field that selects from a set of entities:
 * - Reference fields (assignees, depends_on): commits array of IDs
 * - Computed fields (tags): commits array of IDs — backend DeriveHandler
 *   translates the desired state into body mutations server-side
 *
 * Uses CM6 with prefix autocomplete (e.g. `@` for actors, `#` for tags).
 * Selected items display as pills above the editor.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CodeMirror, { type ReactCodeMirrorRef } from "@uiw/react-codemirror";
import { keymap } from "@codemirror/view";
import { Prec } from "@codemirror/state";
import { invoke } from "@tauri-apps/api/core";
import { shadcnTheme, keymapExtension } from "@/lib/cm-keymap";
import { useUIState } from "@/lib/ui-state-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import {
  createMentionCompletionSource,
  createMentionAutocomplete,
  type MentionSearchResult,
} from "@/lib/cm-mention-autocomplete";
import { AvatarDisplay } from "@/components/fields/displays/avatar-display";
import { createDebouncedSearch } from "@/lib/debounced-search";
import { slugify } from "@/lib/slugify";
import { getStr } from "@/types/kanban";
import type { FieldDef, Entity } from "@/types/kanban";
import type { EditorProps } from "./markdown-editor";

interface MultiSelectEditorProps extends EditorProps {
  field: FieldDef;
  /** The entity being edited (optional, for context). */
  entity?: Entity;
}

export function MultiSelectEditor({
  field,
  value,
  entityType,
  entityId,
  fieldName,
  onCommit,
}: MultiSelectEditorProps) {
  const { keymap_mode: mode } = useUIState();
  const { updateField } = useFieldUpdate();
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();
  const editorRef = useRef<ReactCodeMirrorRef>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Determine target entity type and mention config
  const isComputedTags =
    field.type.kind === "computed" && field.type.derive === "parse-body-tags";

  const targetEntityType = isComputedTags
    ? "tag"
    : (field.type.entity as string | undefined);

  // For computed tag fields, use display name (tag_name) as the committed value
  // since the backend DeriveHandler works with tag slugs, not entity IDs.
  const commitDisplayNames = isComputedTags;

  const mentionConfig = useMemo(
    () => mentionableTypes.find((mt) => mt.entityType === targetEntityType),
    [mentionableTypes, targetEntityType],
  );

  const prefix = mentionConfig?.prefix ?? (isComputedTags ? "#" : "");
  const displayField =
    mentionConfig?.displayField ?? (isComputedTags ? "tag_name" : "name");

  // Target entities for building maps
  const targetEntities = useMemo(
    () => (targetEntityType ? getEntities(targetEntityType) : []),
    [targetEntityType, getEntities],
  );

  // Maps: ID ↔ display name (slugified for fields with spaces like task titles)
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

  // Current selected values
  const currentIds: string[] = useMemo(() => {
    if (Array.isArray(value))
      return value.filter((v): v is string => typeof v === "string");
    return [];
  }, [value]);

  const [selectedIds, setSelectedIds] = useState<string[]>(currentIds);
  const selectedIdsRef = useRef(selectedIds);
  selectedIdsRef.current = selectedIds;

  // Focus editor on mount
  useEffect(() => {
    setTimeout(() => editorRef.current?.view?.focus(), 0);
  }, []);

  // Remove an item from the selection
  const removeItem = useCallback((id: string) => {
    setSelectedIds((prev) => prev.filter((x) => x !== id));
  }, []);

  // Commit: process any remaining text, then call onCommit with the full selection.
  // Both reference and computed fields use the same path — the backend DeriveHandler
  // handles body mutation for computed fields.
  const commit = useCallback(() => {
    const text = editorRef.current?.view?.state.doc.toString().trim();
    if (text) {
      const clean = text.replace(new RegExp(`^\\${prefix}`), "").trim();
      if (clean) {
        const slug = slugify(clean);
        const id =
          displayToId.get(slug) ??
          displayToId.get(clean.toLowerCase()) ??
          (commitDisplayNames ? slug : undefined);
        if (id && !selectedIdsRef.current.includes(id)) {
          selectedIdsRef.current = [...selectedIdsRef.current, id];
        }
      }
    }
    // For computed tags, commit display names (slugs) instead of entity IDs
    const finalValue = commitDisplayNames
      ? selectedIdsRef.current.map((id) => idToDisplay.get(id) ?? id)
      : selectedIdsRef.current;

    if (entityType && entityId && fieldName) {
      updateField(entityType, entityId, fieldName, finalValue).catch(() => {});
    }
    onCommit(finalValue);
  }, [onCommit, prefix, displayToId, commitDisplayNames, idToDisplay, entityType, entityId, fieldName, updateField]);

  const commitRef = useRef(commit);
  commitRef.current = commit;

  // Build async search for autocomplete
  const searchFn = useMemo(() => {
    if (!targetEntityType) return null;
    const rawSearch = async (query: string): Promise<MentionSearchResult[]> => {
      try {
        const results = await invoke<
          Array<{ id: string; display_name: string; color: string }>
        >("search_mentions", { entityType: targetEntityType, query });
        return results
          .filter((r) => {
            const id =
              displayToId.get(slugify(r.display_name)) ??
              displayToId.get(r.display_name.toLowerCase()) ??
              r.display_name;
            return !selectedIdsRef.current.includes(id);
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
  }, [targetEntityType, displayToId]);

  // CM6 extensions
  const extensions = useMemo(() => {
    const exts = [keymapExtension(mode)];

    if (searchFn && prefix) {
      const source = createMentionCompletionSource(prefix, searchFn);
      exts.push(createMentionAutocomplete([source]));
    }

    // Prec.highest ensures our Enter/Escape fire before basicSetup's
    // insertNewline and before any other default keymaps.
    // CM6 autocomplete uses Prec.highest too, so when the completion
    // menu is open, autocomplete's Enter handler runs first (it checks
    // for an active completion and only handles Enter if one exists).
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
              commitRef.current();
              return true;
            },
          },
        ]),
      ),
    );

    return exts;
  }, [mode, searchFn, prefix]);

  // Blur handler
  const handleBlur = useCallback(() => {
    // Small delay to allow clicks on pills to register
    setTimeout(() => {
      if (!containerRef.current?.contains(document.activeElement)) {
        commitRef.current();
      }
    }, 100);
  }, []);

  return (
    <div ref={containerRef} className="space-y-1 p-2">
      {/* Selected items — actors use AvatarDisplay (same component as grid/inspector) */}
      {selectedIds.length > 0 && (
        <div className="flex flex-wrap items-center gap-1">
          {targetEntityType === "actor"
            ? /* Each actor: AvatarDisplay renders the avatar identically to grid/inspector,
               wrapped with a remove button for editor interactivity */
              selectedIds.map((id) => (
                <span key={id} className="inline-flex items-center gap-0.5">
                  <AvatarDisplay value={[id]} />
                  <button
                    type="button"
                    className="hover:opacity-70 leading-none text-muted-foreground text-xs"
                    onClick={() => removeItem(id)}
                    onMouseDown={(e) => e.preventDefault()}
                    title={`Remove ${idToDisplay.get(id) ?? id}`}
                  >
                    &times;
                  </button>
                </span>
              ))
            : selectedIds.map((id) => {
                const name = idToDisplay.get(id) ?? id;
                const ent = targetEntities.find((e) => e.id === id);
                const color = ent ? getStr(ent, "color", "888888") : "888888";
                return (
                  <span
                    key={id}
                    className="inline-flex items-center gap-0.5 rounded-full px-1.5 py-px text-xs font-medium"
                    style={{
                      backgroundColor: `color-mix(in srgb, #${color} 20%, transparent)`,
                      color: `#${color}`,
                      border: `1px solid color-mix(in srgb, #${color} 30%, transparent)`,
                    }}
                  >
                    {prefix}
                    {name}
                    <button
                      type="button"
                      className="ml-0.5 hover:opacity-70 leading-none"
                      onClick={() => removeItem(id)}
                      onMouseDown={(e) => e.preventDefault()}
                    >
                      &times;
                    </button>
                  </span>
                );
              })}
        </div>
      )}
      {/* CM6 input */}
      <CodeMirror
        ref={editorRef}
        value=""
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
    </div>
  );
}
