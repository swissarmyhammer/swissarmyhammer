/**
 * Multi-select editor for reference fields (assignees, depends_on, etc.).
 *
 * Discovers the target entity type from field.type.entity, looks up its
 * mention config from schema context, and provides autocomplete + pill display.
 *
 * Two modes:
 * - Prefix mode (entity has mention_prefix): shows @name pills with prefix autocomplete
 * - Plain mode (no prefix): shows name pills with plain text autocomplete
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { getStr } from "@/types/kanban";
import type { FieldDef } from "@/types/kanban";
import type { EditorProps } from "./markdown-editor";

interface MultiSelectEditorProps extends EditorProps {
  field: FieldDef;
}

export function MultiSelectEditor({
  field,
  value,
  onCommit,
  onCancel: _onCancel,
}: MultiSelectEditorProps) {
  const targetEntityType = (field.type as Record<string, unknown>).entity as string | undefined;
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  const mentionConfig = useMemo(
    () => mentionableTypes.find((mt) => mt.entityType === targetEntityType),
    [mentionableTypes, targetEntityType],
  );

  const prefix = mentionConfig?.prefix ?? "";
  const displayField = mentionConfig?.displayField ?? "name";

  // Current selected IDs
  const currentIds: string[] = useMemo(() => {
    if (Array.isArray(value)) return value.filter((v): v is string => typeof v === "string");
    return [];
  }, [value]);

  // Resolve IDs to display names
  const entities = useMemo(
    () => (targetEntityType ? getEntities(targetEntityType) : []),
    [targetEntityType, getEntities],
  );

  const idToDisplay = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of entities) {
      const name = getStr(e, displayField) || e.id;
      map.set(e.id, name);
    }
    return map;
  }, [entities, displayField]);

  const displayToId = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of entities) {
      const name = getStr(e, displayField);
      if (name) map.set(name.toLowerCase(), e.id);
      map.set(e.id.toLowerCase(), e.id);
    }
    return map;
  }, [entities, displayField]);

  const [selectedIds, setSelectedIds] = useState<string[]>(currentIds);
  const [query, setQuery] = useState("");
  const [suggestions, setSuggestions] = useState<Array<{ id: string; name: string; color: string }>>([]);
  const [highlightIdx, setHighlightIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Search for suggestions
  useEffect(() => {
    if (!targetEntityType) return;
    if (query.length === 0) {
      // Show all non-selected entities
      const filtered = entities
        .filter((e) => !selectedIds.includes(e.id))
        .map((e) => ({
          id: e.id,
          name: getStr(e, displayField) || e.id,
          color: getStr(e, "color", "888888"),
        }));
      setSuggestions(filtered);
      setHighlightIdx(0);
      return;
    }

    // Debounce backend search to avoid redundant calls during rapid typing
    const timer = setTimeout(() => {
      invoke<Array<{ id: string; display_name: string; color: string }>>(
        "search_mentions",
        { entityType: targetEntityType, query: query.replace(new RegExp(`^\\${prefix}`), "") },
      )
        .then((results) => {
          const filtered = results
            .filter((r) => !selectedIds.includes(displayToId.get(r.display_name.toLowerCase()) ?? ""))
            .map((r) => ({
              id: displayToId.get(r.display_name.toLowerCase()) ?? r.display_name,
              name: r.display_name,
              color: r.color,
            }));
          setSuggestions(filtered);
          setHighlightIdx(0);
        })
        .catch(() => setSuggestions([]));
    }, 150);

    return () => clearTimeout(timer);
  }, [query, targetEntityType, selectedIds, entities, displayField, prefix, displayToId]);

  const addId = useCallback((id: string) => {
    setSelectedIds((prev) => {
      if (prev.includes(id)) return prev;
      return [...prev, id];
    });
    setQuery("");
  }, []);

  const removeId = useCallback((id: string) => {
    setSelectedIds((prev) => prev.filter((x) => x !== id));
  }, []);

  const commit = useCallback(() => {
    onCommit(selectedIds);
  }, [onCommit, selectedIds]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        if (suggestions.length > 0 && highlightIdx < suggestions.length) {
          addId(suggestions[highlightIdx].id);
        }
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        if (query) {
          setQuery("");
        } else {
          commit();
        }
        return;
      }
      if (e.key === "Backspace" && query === "" && selectedIds.length > 0) {
        removeId(selectedIds[selectedIds.length - 1]);
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setHighlightIdx((i) => Math.min(i + 1, suggestions.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setHighlightIdx((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Tab") {
        e.preventDefault();
        commit();
      }
    },
    [suggestions, highlightIdx, query, selectedIds, addId, removeId, commit],
  );

  // Commit on blur (click outside)
  const handleBlur = useCallback(
    (e: React.FocusEvent) => {
      // Don't commit if focus moved within our container (e.g. clicking a suggestion)
      if (containerRef.current?.contains(e.relatedTarget as Node)) return;
      commit();
    },
    [commit],
  );

  return (
    <div ref={containerRef} className="relative">
      <div
        className="flex flex-wrap items-center gap-1 rounded-md border border-input bg-transparent px-2 py-1 text-sm min-h-[2rem]"
        onClick={() => inputRef.current?.focus()}
      >
        {selectedIds.map((id) => {
          const name = idToDisplay.get(id) ?? id;
          const entity = entities.find((e) => e.id === id);
          const color = entity ? getStr(entity, "color", "888888") : "888888";
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
              {prefix}{name}
              <button
                type="button"
                className="ml-0.5 hover:opacity-70 leading-none"
                onClick={(e) => { e.stopPropagation(); removeId(id); }}
                onMouseDown={(e) => e.preventDefault()}
              >
                &times;
              </button>
            </span>
          );
        })}
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          className="flex-1 min-w-[60px] bg-transparent outline-none text-sm"
          placeholder={selectedIds.length === 0 ? `Type ${prefix} to search...` : ""}
        />
      </div>

      {/* Suggestions dropdown */}
      {suggestions.length > 0 && (
        <div className="absolute z-50 mt-1 w-full rounded-md border border-border bg-popover shadow-md max-h-[200px] overflow-y-auto">
          {suggestions.map((s, i) => (
            <button
              key={s.id}
              type="button"
              className={`w-full text-left px-2 py-1.5 text-sm flex items-center gap-2 ${
                i === highlightIdx ? "bg-accent text-accent-foreground" : "hover:bg-accent/50"
              }`}
              onMouseDown={(e) => {
                e.preventDefault(); // prevent blur
                addId(s.id);
              }}
              onMouseEnter={() => setHighlightIdx(i)}
            >
              <span
                className="w-2 h-2 rounded-full shrink-0"
                style={{ backgroundColor: `#${s.color}` }}
              />
              {prefix}{s.name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
