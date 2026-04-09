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

/** Debounce delay for mention search queries against the Tauri backend. */
const MENTION_SEARCH_DEBOUNCE_MS = 150;

/** Virtual tag names defined by the backend VirtualTagRegistry. */
const VIRTUAL_TAG_SLUGS = ["READY", "BLOCKED", "BLOCKING"] as const;

/** Color used for virtual tags in autocomplete (distinct from real tags). */
const VIRTUAL_TAG_COLOR = "7c3aed";

/** Options for controlling autocomplete behavior in different editor contexts. */
export interface MentionExtensionOptions {
  /** Include virtual tags (READY, BLOCKED, BLOCKING) in `#` completions. */
  includeVirtualTags?: boolean;
  /** Include `@user` and `^ref` completion sources for the filter editor. */
  includeFilterSigils?: boolean;
}

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

  return createDebouncedSearch({
    search: rawSearch,
    delayMs: MENTION_SEARCH_DEBOUNCE_MS,
  });
}

/**
 * Build a search function that includes virtual tag entries alongside real results.
 *
 * Wraps an existing async search, prepending virtual tag matches (READY,
 * BLOCKED, BLOCKING) that pass the query filter. Virtual tags are styled
 * distinctly via a dedicated color.
 */
function buildVirtualTagSearch(
  baseSearch: (query: string) => Promise<MentionSearchResult[]>,
): (query: string) => Promise<MentionSearchResult[]> {
  return async (query: string) => {
    const virtualResults: MentionSearchResult[] = VIRTUAL_TAG_SLUGS.filter(
      (slug) => !query || slug.toLowerCase().includes(query.toLowerCase()),
    ).map((slug) => ({
      slug,
      displayName: `${slug} (virtual)`,
      color: VIRTUAL_TAG_COLOR,
    }));
    const realResults = await baseSearch(query);
    return [...virtualResults, ...realResults];
  };
}

/** Merge virtual tag slugs into a color map so they receive pill decorations. */
function mergeVirtualTags(base: Map<string, string>): Map<string, string> {
  const merged = new Map(base);
  for (const slug of VIRTUAL_TAG_SLUGS) {
    merged.set(slug, VIRTUAL_TAG_COLOR);
  }
  return merged;
}

/** Merge virtual tag slugs into a meta map so they receive tooltip support. */
function mergeVirtualTagMeta(
  base: Map<string, MentionMeta>,
): Map<string, MentionMeta> {
  const merged = new Map(base);
  for (const slug of VIRTUAL_TAG_SLUGS) {
    merged.set(slug, {
      color: VIRTUAL_TAG_COLOR,
      description: `${slug} (virtual tag)`,
    });
  }
  return merged;
}

/** Enriched mention data with color and meta maps built from entities. */
interface MentionDatum {
  prefix: string;
  entityType: string;
  displayField: string;
  colorMap: Map<string, string>;
  metaMap: Map<string, MentionMeta>;
}

/**
 * Pure function that assembles the CM6 extension array from mention data.
 *
 * Builds decoration, autocomplete, and tooltip extensions for each mentionable
 * type. Merges virtual tags and filter sigil sources when the corresponding
 * options are enabled.
 */
function buildMentionExtensions(
  mentionData: MentionDatum[],
  includeVirtualTags: boolean,
  includeFilterSigils: boolean,
): Extension[] {
  const exts: Extension[] = [];
  const completionSources: Array<
    ReturnType<typeof createMentionCompletionSource>
  > = [];

  for (const md of mentionData) {
    const addVirtual = includeVirtualTags && md.prefix === "#";
    const colorMap = addVirtual ? mergeVirtualTags(md.colorMap) : md.colorMap;
    const metaMap = addVirtual ? mergeVirtualTagMeta(md.metaMap) : md.metaMap;

    if (colorMap.size === 0) continue;
    exts.push(getDecoInfra(md.prefix, md.entityType).extension(colorMap));

    const baseSearch = buildAsyncSearch(md.entityType);
    const search = addVirtual ? buildVirtualTagSearch(baseSearch) : baseSearch;
    completionSources.push(createMentionCompletionSource(md.prefix, search));

    exts.push(getTooltipInfra(md.prefix, md.entityType).extension(metaMap));
  }

  if (includeFilterSigils) {
    completionSources.push(
      createMentionCompletionSource("@", buildAsyncSearch("actor")),
    );
    completionSources.push(
      createMentionCompletionSource("^", buildAsyncSearch("task")),
    );
  }
  if (completionSources.length > 0) {
    exts.push(createMentionAutocomplete(completionSources));
  }
  return exts;
}

/**
 * Build CM6 extensions for mention decorations, autocomplete, and tooltips.
 *
 * @param options — controls which completion sources are active:
 *   - `includeVirtualTags`: add READY/BLOCKED/BLOCKING to `#` completions
 *   - `includeFilterSigils`: add `@user` and `^ref` completion sources
 *
 * Returns a stable Extension[] that only changes when mentionable entity data changes.
 * Returns an empty array when there are no mentionable types in the schema.
 */
export function useMentionExtensions(
  options?: MentionExtensionOptions,
): Extension[] {
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();
  const includeVirtualTags = options?.includeVirtualTags ?? false;
  const includeFilterSigils = options?.includeFilterSigils ?? false;

  const mentionData = useMemo(() => {
    return mentionableTypes.map((mt) => {
      const entities = getEntities(mt.entityType);
      return {
        ...mt,
        colorMap: buildColorMap(entities, mt.displayField),
        metaMap: buildMetaMap(entities, mt.displayField),
      };
    });
  }, [mentionableTypes, getEntities]);

  return useMemo(
    () =>
      buildMentionExtensions(
        mentionData,
        includeVirtualTags,
        includeFilterSigils,
      ),
    [mentionData, includeVirtualTags, includeFilterSigils],
  );
}
