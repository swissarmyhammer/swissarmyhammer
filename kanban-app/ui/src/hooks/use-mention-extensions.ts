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
import { useBoardData } from "@/components/window-container";
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
import type { Entity, VirtualTagMeta } from "@/types/kanban";
import { getStr } from "@/types/kanban";

/** Debounce delay for mention search queries against the Tauri backend. */
const MENTION_SEARCH_DEBOUNCE_MS = 150;

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

/**
 * Read the raw slug-field value from an entity.
 *
 * The special value `"id"` reads the top-level `entity.id` property
 * instead of `entity.fields.id`, since the id is hoisted onto the
 * `Entity` wrapper by `entityFromBag`. All other field names fall
 * through to the regular `fields` record.
 */
function getSlugFieldValue(e: Entity, slugField: string): string {
  if (slugField === "id") return e.id;
  return getStr(e, slugField);
}

/**
 * Build a slug→color map for a mentionable entity type.
 *
 * When `slugField` is provided, entries are keyed by the raw value of that
 * field (typically `id`) — no slugify. This is required for entity types
 * whose ids are free-form text that does not equal `slugify(displayField)`
 * (e.g. a project with `id: AUTH-Migration`, `name: Auth Migration System`).
 *
 * When `slugField` is absent, entries are keyed by `slugify(displayField)`,
 * preserving the existing behavior used by tags and actors whose ids are
 * already slug-shaped.
 */
export function buildColorMap(
  entities: Entity[],
  displayField: string,
  slugField?: string,
): Map<string, string> {
  const map = new Map<string, string>();
  for (const e of entities) {
    const color = getStr(e, "color", "888888");
    if (slugField) {
      const key = getSlugFieldValue(e, slugField);
      if (key) map.set(key, color);
    } else {
      const raw = getStr(e, displayField);
      if (raw) map.set(slugify(raw), color);
    }
  }
  return map;
}

/**
 * Build a slug→meta map for tooltips.
 *
 * When `slugField` is provided, entries are keyed by the raw value of that
 * field — no slugify — mirroring `buildColorMap`. When absent, entries are
 * keyed by `slugify(displayField)`.
 */
export function buildMetaMap(
  entities: Entity[],
  displayField: string,
  slugField?: string,
): Map<string, MentionMeta> {
  const map = new Map<string, MentionMeta>();
  for (const e of entities) {
    const color = getStr(e, "color", "888888");
    const description = getStr(e, "description") || undefined;
    if (slugField) {
      const key = getSlugFieldValue(e, slugField);
      if (key) map.set(key, { color, description });
    } else {
      const raw = getStr(e, displayField);
      if (raw) map.set(slugify(raw), { color, description });
    }
  }
  return map;
}

/**
 * Build a debounced async search function that calls the Tauri backend.
 *
 * When `slugField` is declared on the entity type, the returned
 * `MentionSearchResult.slug` is sourced verbatim from the corresponding
 * backend field instead of `slugify(display_name)`. Only `slugField: "id"`
 * is supported — the backend search endpoint currently projects only
 * `{id, display_name, color, avatar}`, so other slug-field names would
 * require backend changes. Any other value throws loudly so the mismatch
 * is visible at the earliest possible point.
 */
export function buildAsyncSearch(
  entityType: string,
  slugField?: string,
): (query: string) => Promise<MentionSearchResult[]> {
  if (slugField !== undefined && slugField !== "id") {
    throw new Error(
      `buildAsyncSearch: unsupported slugField "${slugField}" for entity type "${entityType}". Only "id" is supported; other slug fields would require the search_mentions backend endpoint to project them.`,
    );
  }
  const rawSearch = async (query: string): Promise<MentionSearchResult[]> => {
    try {
      const results = await invoke<
        Array<{ id: string; display_name: string; color: string }>
      >("search_mentions", { entityType, query });
      return results.map((r) => ({
        slug: slugField === "id" ? r.id : slugify(r.display_name),
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
 * Wraps an existing async search, prepending virtual tag matches that pass the
 * query filter. Colors come from the backend VirtualTagRegistry metadata.
 */
function buildVirtualTagSearch(
  baseSearch: (query: string) => Promise<MentionSearchResult[]>,
  vtMeta: VirtualTagMeta[],
): (query: string) => Promise<MentionSearchResult[]> {
  return async (query: string) => {
    const virtualResults: MentionSearchResult[] = vtMeta
      .filter(
        (m) => !query || m.slug.toLowerCase().includes(query.toLowerCase()),
      )
      .map((m) => ({
        slug: m.slug,
        displayName: `${m.slug} (virtual)`,
        color: m.color,
      }));
    const realResults = await baseSearch(query);
    return [...virtualResults, ...realResults];
  };
}

/** Merge virtual tag entries into a color map so they receive pill decorations. */
function mergeVirtualTagColors(
  base: Map<string, string>,
  vtMeta: VirtualTagMeta[],
): Map<string, string> {
  const merged = new Map(base);
  for (const m of vtMeta) merged.set(m.slug, m.color);
  return merged;
}

/** Merge virtual tag entries into a meta map so they receive tooltip support. */
function mergeVirtualTagTooltips(
  base: Map<string, MentionMeta>,
  vtMeta: VirtualTagMeta[],
): Map<string, MentionMeta> {
  const merged = new Map(base);
  for (const m of vtMeta)
    merged.set(m.slug, { color: m.color, description: m.description });
  return merged;
}

/** Enriched mention data with color and meta maps built from entities. */
interface MentionDatum {
  prefix: string;
  entityType: string;
  displayField: string;
  /** Raw field supplying the mention slug (e.g. `id`). See `buildAsyncSearch`. */
  slugField?: string;
  colorMap: Map<string, string>;
  metaMap: Map<string, MentionMeta>;
}

/**
 * Pure function that assembles the CM6 extension array from mention data.
 *
 * Builds decoration, autocomplete, and tooltip extensions for each mentionable
 * type. Merges virtual tags (using backend metadata) and filter sigil sources
 * when the corresponding options are enabled.
 */
function buildMentionExtensions(
  mentionData: MentionDatum[],
  includeVirtualTags: boolean,
  includeFilterSigils: boolean,
  vtMeta: VirtualTagMeta[],
): Extension[] {
  const exts: Extension[] = [];
  const completionSources: Array<
    ReturnType<typeof createMentionCompletionSource>
  > = [];

  for (const md of mentionData) {
    const addVirtual =
      includeVirtualTags && md.prefix === "#" && vtMeta.length > 0;
    const colorMap = addVirtual
      ? mergeVirtualTagColors(md.colorMap, vtMeta)
      : md.colorMap;
    const metaMap = addVirtual
      ? mergeVirtualTagTooltips(md.metaMap, vtMeta)
      : md.metaMap;

    if (colorMap.size === 0) continue;
    exts.push(getDecoInfra(md.prefix, md.entityType).extension(colorMap));

    const baseSearch = buildAsyncSearch(md.entityType, md.slugField);
    const search = addVirtual
      ? buildVirtualTagSearch(baseSearch, vtMeta)
      : baseSearch;
    completionSources.push(createMentionCompletionSource(md.prefix, search));

    exts.push(getTooltipInfra(md.prefix, md.entityType).extension(metaMap));
  }

  if (includeFilterSigils) {
    // Look up slugField for actor/task/project from the mention data (if present)
    // so filter sigils honor the same schema signal as the real mentions.
    // Actor and task entity types do not declare `mention_slug_field` today,
    // so those reduce to the legacy `slugify(display_name)` behavior. Project
    // declares `mention_slug_field: "id"` so its slug is sourced verbatim from
    // the backend `id` field.
    const actorSlugField = mentionData.find(
      (md) => md.entityType === "actor",
    )?.slugField;
    const taskSlugField = mentionData.find(
      (md) => md.entityType === "task",
    )?.slugField;
    const projectSlugField = mentionData.find(
      (md) => md.entityType === "project",
    )?.slugField;
    completionSources.push(
      createMentionCompletionSource(
        "@",
        buildAsyncSearch("actor", actorSlugField),
      ),
    );
    completionSources.push(
      createMentionCompletionSource(
        "^",
        buildAsyncSearch("task", taskSlugField),
      ),
    );
    completionSources.push(
      createMentionCompletionSource(
        "$",
        buildAsyncSearch("project", projectSlugField),
      ),
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
  const boardData = useBoardData();
  const includeVirtualTags = options?.includeVirtualTags ?? false;
  const includeFilterSigils = options?.includeFilterSigils ?? false;
  const vtMeta = boardData?.virtualTagMeta ?? [];

  const mentionData = useMemo(() => {
    return mentionableTypes.map((mt) => {
      const entities = getEntities(mt.entityType);
      return {
        ...mt,
        colorMap: buildColorMap(entities, mt.displayField, mt.slugField),
        metaMap: buildMetaMap(entities, mt.displayField, mt.slugField),
      };
    });
  }, [mentionableTypes, getEntities]);

  return useMemo(
    () =>
      buildMentionExtensions(
        mentionData,
        includeVirtualTags,
        includeFilterSigils,
        vtMeta,
      ),
    [mentionData, includeVirtualTags, includeFilterSigils, vtMeta],
  );
}
