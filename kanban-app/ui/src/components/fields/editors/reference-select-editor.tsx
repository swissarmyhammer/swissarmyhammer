/**
 * Searchable combobox editor for scalar reference fields.
 *
 * Uses a search input and filtered list to provide type-ahead search for
 * entity references (e.g. position_column -> column entities).
 * Calls search_mentions for backend-filtered results and commits
 * the resolved entity ID.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useUIState } from "@/lib/ui-state-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { getStr } from "@/types/kanban";
import type { FieldDef } from "@/types/kanban";
import type { EditorProps } from ".";

/** Result shape returned by the search_mentions Tauri command. */
interface MentionResult {
  id: string;
  display_name: string;
  color: string;
}

interface ReferenceSelectEditorProps extends EditorProps {
  field: FieldDef;
}

/** Hook encapsulating the debounced search state for reference entities. */
function useReferenceSearch(targetEntityType: string | undefined) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<MentionResult[]>([]);
  const [highlightIndex, setHighlightIndex] = useState(0);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const doSearch = useCallback(
    async (q: string) => {
      if (!targetEntityType) return;
      try {
        const res = await invoke<MentionResult[]>("search_mentions", {
          entityType: targetEntityType,
          query: q,
        });
        setResults(res);
        setHighlightIndex(0);
      } catch {
        setResults([]);
      }
    },
    [targetEntityType],
  );

  const debouncedSearch = useCallback(
    (q: string) => {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => doSearch(q), 150);
    },
    [doSearch],
  );

  useEffect(() => {
    doSearch("");
  }, [doSearch]);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return {
    query,
    setQuery,
    results,
    highlightIndex,
    setHighlightIndex,
    debouncedSearch,
  };
}

/** Hook encapsulating commit/cancel guards and keyboard navigation. */
function useCommitHandlers(
  onCommit: (v: string) => void,
  onCancel: () => void,
  onChange: ((v: string | null) => void) | undefined,
) {
  const committedRef = useRef(false);

  const commit = useCallback(
    (val: string) => {
      if (committedRef.current) return;
      committedRef.current = true;
      onCommit(val);
    },
    [onCommit],
  );

  const cancel = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCancel();
  }, [onCancel]);

  const handleSelect = useCallback(
    (id: string) => {
      onChange?.(id || null);
      commit(id);
    },
    [commit, onChange],
  );

  return { committedRef, commit, cancel, handleSelect };
}

/** Hook resolving the current entity's display name and color from the store. */
function useCurrentEntityDisplay(
  targetEntityType: string | undefined,
  currentValue: string,
) {
  const { getEntity } = useEntityStore();
  const entity = useMemo(() => {
    if (!targetEntityType || !currentValue) return null;
    return getEntity(targetEntityType, currentValue) ?? null;
  }, [targetEntityType, currentValue, getEntity]);

  return {
    displayName: entity ? getStr(entity, "name") || entity.id : "",
    color: entity ? getStr(entity, "color", "888888") : "",
  };
}

/** Hook for keyboard navigation (arrows, enter, escape) in the combobox. */
function useKeyboardNav(
  search: ReturnType<typeof useReferenceSearch>,
  handleSelect: (id: string) => void,
  commit: (val: string) => void,
  cancel: () => void,
  currentValue: string,
) {
  const { keymap_mode: mode } = useUIState();
  return useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        search.setHighlightIndex((p) => Math.min(p + 1, search.results.length));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        search.setHighlightIndex((p) => Math.max(p - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        const item =
          search.highlightIndex === 0
            ? null
            : search.results[search.highlightIndex - 1];
        handleSelect(item?.id ?? "");
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        if (mode === "vim") commit(currentValue);
        else cancel();
      }
    },
    [search, handleSelect, commit, cancel, mode, currentValue],
  );
}

/** Display the current value as a colored dot + name. */
function ReferenceTrigger({
  displayName,
  color,
}: {
  displayName: string;
  color: string;
}) {
  if (!displayName) {
    return <span className="text-muted-foreground">-</span>;
  }
  return (
    <>
      <span
        data-ref-dot
        className="inline-block w-2 h-2 rounded-full shrink-0"
        style={{ backgroundColor: `#${color}` }}
      />
      <span className="truncate">{displayName}</span>
    </>
  );
}

/** A single selectable item in the results list. */
function ReferenceItem({
  item,
  highlighted,
  onSelect,
  onHover,
}: {
  item: MentionResult;
  highlighted: boolean;
  onSelect: () => void;
  onHover: () => void;
}) {
  return (
    <div
      data-ref-item
      role="option"
      aria-selected={highlighted}
      className={`flex items-center gap-1.5 px-2 py-1 text-sm cursor-pointer ${
        highlighted ? "bg-accent text-accent-foreground" : ""
      }`}
      onMouseDown={(e) => e.preventDefault()}
      onClick={onSelect}
      onMouseEnter={onHover}
    >
      {item.color && (
        <span
          data-ref-dot
          className="inline-block w-2 h-2 rounded-full shrink-0"
          style={{ backgroundColor: `#${item.color}` }}
        />
      )}
      <span className="truncate">{item.display_name}</span>
    </div>
  );
}

/** Consolidated state hook that wires all the editor's sub-hooks together. */
function useReferenceEditorState(props: ReferenceSelectEditorProps) {
  const { field, value, onCommit, onCancel, onChange } = props;
  const inputRef = useRef<HTMLInputElement>(null);
  const targetEntityType = field.type.entity as string | undefined;
  const currentValue = typeof value === "string" ? value : "";

  const search = useReferenceSearch(targetEntityType);
  const commits = useCommitHandlers(onCommit, onCancel, onChange);
  const entityDisplay = useCurrentEntityDisplay(targetEntityType, currentValue);
  const handleKeyDown = useKeyboardNav(
    search,
    commits.handleSelect,
    commits.commit,
    commits.cancel,
    currentValue,
  );

  useEffect(() => {
    setTimeout(() => inputRef.current?.focus(), 0);
  }, []);

  return {
    inputRef,
    currentValue,
    search,
    commits,
    entityDisplay,
    handleKeyDown,
  };
}

/** Header showing the current value as a trigger label. */
function ReferenceHeader({
  displayName,
  color,
}: {
  displayName: string;
  color: string;
}) {
  return (
    <div
      data-ref-trigger
      className="flex items-center gap-1.5 px-2 py-1 text-sm border-b border-border mb-1"
    >
      <ReferenceTrigger displayName={displayName} color={color} />
    </div>
  );
}

/** The text input for type-ahead search within the combobox. */
function ReferenceSearchInput({
  inputRef,
  query,
  onQueryChange,
  onKeyDown,
  onBlur,
}: {
  inputRef: React.RefObject<HTMLInputElement | null>;
  query: string;
  onQueryChange: (q: string) => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  onBlur: () => void;
}) {
  return (
    <input
      ref={inputRef}
      data-ref-search
      type="text"
      value={query}
      onChange={(e) => onQueryChange(e.target.value)}
      onKeyDown={onKeyDown}
      onBlur={onBlur}
      placeholder="Search..."
      className="w-full px-2 py-1 text-sm border-b border-border bg-transparent outline-none"
    />
  );
}

/** The scrollable dropdown showing clear option + search results. */
function ReferenceResultsList({
  results,
  highlightIndex,
  onSelect,
  onHighlight,
}: {
  results: MentionResult[];
  highlightIndex: number;
  onSelect: (id: string) => void;
  onHighlight: (index: number) => void;
}) {
  return (
    <div className="max-h-48 overflow-y-auto py-1">
      <div
        data-ref-clear
        role="option"
        aria-selected={highlightIndex === 0}
        className={`flex items-center gap-1.5 px-2 py-1 text-sm cursor-pointer ${
          highlightIndex === 0 ? "bg-accent text-accent-foreground" : ""
        }`}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => onSelect("")}
        onMouseEnter={() => onHighlight(0)}
      >
        <span className="text-muted-foreground">-</span>
      </div>

      {results.map((item, i) => (
        <ReferenceItem
          key={item.id}
          item={item}
          highlighted={highlightIndex === i + 1}
          onSelect={() => onSelect(item.id)}
          onHover={() => onHighlight(i + 1)}
        />
      ))}
    </div>
  );
}

/**
 * Searchable combobox for scalar reference fields.
 *
 * Opens immediately on mount with a search input and a filtered list
 * of matching entities. Selecting an item commits the entity ID.
 * Escape follows vim/CUA convention (vim: commit, CUA: cancel).
 */
export function ReferenceSelectEditor(props: ReferenceSelectEditorProps) {
  const { inputRef, currentValue, search, commits, entityDisplay, handleKeyDown } =
    useReferenceEditorState(props);

  const handleQueryChange = (q: string) => {
    search.setQuery(q);
    search.debouncedSearch(q);
  };

  const handleBlur = () => {
    setTimeout(() => {
      if (!commits.committedRef.current) commits.commit(currentValue);
    }, 150);
  };

  return (
    <div className="relative w-full">
      <ReferenceHeader
        displayName={entityDisplay.displayName}
        color={entityDisplay.color}
      />
      <ReferenceSearchInput
        inputRef={inputRef}
        query={search.query}
        onQueryChange={handleQueryChange}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
      />
      <ReferenceResultsList
        results={search.results}
        highlightIndex={search.highlightIndex}
        onSelect={commits.handleSelect}
        onHighlight={search.setHighlightIndex}
      />
    </div>
  );
}
