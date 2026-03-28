/**
 * Hook that builds CM6 extensions for mention decorations, autocomplete, and tooltips.
 *
 * Reads mentionable types from SchemaContext and entities from EntityStoreContext,
 * then builds the CM6 extension array. Returns a stable array reference that only
 * changes when the underlying entity data changes.
 */

import { useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Extension } from "@codemirror/state";
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
import { slugify } from "@/lib/slugify";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Pre-built mention infrastructure per entity type (created once per prefix).
// Keyed by "prefix:entityType" — bounded by schema-defined mentionable types.
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

/** Build a slug→color map for a mentionable entity type. */
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

/** Build a slug→meta map for tooltips. */
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

/** Build a debounced async search function that calls the Tauri backend. */
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

/**
 * Build CM6 extensions for mention decorations, autocomplete, and tooltips.
 *
 * Returns a stable Extension[] that only changes when mentionable entity data changes.
 * Returns an empty array when there are no mentionable types in the schema.
 */
export function useMentionExtensions(): Extension[] {
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  const mentionData = useMemo(() => {
    return mentionableTypes.map((mt) => {
      const entities = getEntities(mt.entityType);
      return {
        ...mt,
        entities,
        colorMap: buildColorMap(entities, mt.displayField),
        metaMap: buildMetaMap(entities, mt.displayField),
      };
    });
  }, [mentionableTypes, getEntities]);

  return useMemo((): Extension[] => {
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
  }, [mentionData]);
}
