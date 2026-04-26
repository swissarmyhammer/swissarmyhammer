/**
 * MentionView — CM6-based mention pill renderer.
 *
 * The single rendering path for mention pills outside of editable text
 * fields. Callers pass one or more `{entityType, id | slug}` references;
 * MentionView resolves each to its mention-text form (`${prefix}${slug}`),
 * mounts a read-only `TextViewer` with the scoped mention extensions, and
 * wraps everything in `FocusScope` + entity commands so clicks, context
 * menus, and keyboard nav still work.
 *
 * Two modes:
 * - Single mode (`entityType` + `id`): one FocusScope + one TextViewer.
 * - List mode (`items`): a flex-wrap container with one FocusScope +
 *   TextViewer per item. Each pill is its own leaf in the spatial-nav
 *   graph; within-list keyboard navigation (nav.left/nav.right) is
 *   driven by the spatial beam search rule 1 (in-zone candidates) when
 *   the pills sit inside a parent zone such as an inspector field row.
 *
 * Unknown entities fall back to `${prefix}${rawValue}` — the CM6 widget
 * pipeline renders these as muted raw-slug marks automatically.
 */

import { useMemo } from "react";
import type { Extension } from "@codemirror/state";
import { FocusScope } from "@/components/focus-scope";
import { TextViewer } from "@/components/text-viewer";
import { useEntityStore } from "@/lib/entity-store-context";
import { asMoniker } from "@/types/spatial";
import { useSchema, type MentionableType } from "@/lib/schema-context";
import { createMentionDecorations } from "@/lib/cm-mention-decorations";
import { buildMentionMetaMap } from "@/hooks/use-mention-extensions";
import type { MentionMeta } from "@/lib/mention-meta";
import { moniker as buildMoniker } from "@/lib/moniker";
import { slugify } from "@/lib/slugify";
import type { CommandDef } from "@/lib/command-scope";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

/** A single mention reference — either an entity id or an existing slug. */
export interface MentionItem {
  entityType: string;
  id?: string;
  slug?: string;
}

/** Props for MentionView — single-mode or list-mode are mutually exclusive. */
export interface MentionViewProps {
  /** Single mention: entity type (paired with `id`). */
  entityType?: string;
  /** Single mention: entity id. */
  id?: string;
  /** List mode: multiple mention references. */
  items?: MentionItem[];
  /** Optional className passed through to each TextViewer. */
  className?: string;
  /**
   * Override the FocusScope moniker (single mode only). When provided,
   * it replaces the default `${entityType}:${id}` moniker — use this to
   * make a pill uniquely focusable in a specific context.
   */
  focusMoniker?: string;
  /** When false, suppresses the focus bar on click (still participates in commands). */
  showFocusBar?: boolean;
  /** Extra commands (e.g. task.untag) added to the context menu. */
  extraCommands?: CommandDef[];
  /**
   * Rendering mode — `"full"` enables keyboard navigation between list items.
   * Defaults to `"full"` in list mode.
   */
  mode?: "full" | "compact";
}

/** Resolved form of a mention reference — ready for CM6 rendering. */
interface ResolvedMention {
  entityType: string;
  /** The entity that was resolved, if any. */
  entity: Entity | undefined;
  /** Mention prefix from schema (e.g. "#", "@", "$"). */
  prefix: string;
  /** Display-field name used for slug synthesis. */
  displayField: string;
  /** The slug appearing in the CM6 doc (after the prefix). */
  slug: string;
  /** The id used to build a FocusScope moniker. */
  monikerId: string;
}

/**
 * Resolve a single mention reference against the entity store and schema.
 *
 * Looks up the entity by id or by slugified display-field value. For
 * misses, the raw id/slug is used so the CM6 widget pipeline renders
 * the muted-mark fallback.
 */
function resolveMention(
  item: MentionItem,
  mentionableTypes: MentionableType[],
  getEntities: (type: string) => Entity[],
): ResolvedMention {
  const config = mentionableTypes.find(
    (mt) => mt.entityType === item.entityType,
  );
  const prefix = config?.prefix ?? "";
  const displayField = config?.displayField ?? "name";

  const entities = getEntities(item.entityType);
  let entity: Entity | undefined;
  if (item.id !== undefined) {
    entity = entities.find((e) => e.id === item.id);
  } else if (item.slug !== undefined) {
    entity = entities.find((e) => {
      const val = getStr(e, displayField);
      return val !== "" && (val === item.slug || slugify(val) === item.slug);
    });
  }

  let slug: string;
  if (entity) {
    const raw = getStr(entity, displayField);
    slug = raw ? slugify(raw) : (item.slug ?? item.id ?? "");
  } else {
    slug = item.slug ?? item.id ?? "";
  }

  const monikerId = entity?.id ?? item.id ?? item.slug ?? "";

  return {
    entityType: item.entityType,
    entity,
    prefix,
    displayField,
    slug,
    monikerId,
  };
}

/**
 * Collect unresolved slugs grouped by entity type. The widget pipeline needs
 * placeholder entries in each type's metaMap so `findMentionsInText` discovers
 * them and renders muted-mark fallbacks.
 */
function collectUnresolvedByType(
  resolved: ResolvedMention[],
): Map<string, Set<string>> {
  const unresolvedByType = new Map<string, Set<string>>();
  for (const r of resolved) {
    if (r.entity || !r.prefix || !r.slug) continue;
    let set = unresolvedByType.get(r.entityType);
    if (!set) {
      set = new Set();
      unresolvedByType.set(r.entityType, set);
    }
    set.add(r.slug);
  }
  return unresolvedByType;
}

/**
 * Merge unresolved-slug placeholder entries into a metaMap.
 * Empty color triggers the muted-mark fallback in `decorateLine`.
 */
function addUnresolvedPlaceholders(
  metaMap: Map<string, MentionMeta>,
  unresolved: Set<string> | undefined,
): void {
  if (!unresolved) return;
  for (const slug of unresolved) {
    if (metaMap.has(slug)) continue;
    const placeholder: MentionMeta = { color: "", displayName: slug };
    metaMap.set(slug, placeholder);
  }
}

/**
 * Build a minimal scoped CM6 extension array covering only the entity
 * types present in `resolved`. Each type gets its own decoration bundle
 * with a metaMap containing both the resolved entities AND placeholder
 * entries for any unresolved slugs.
 */
function buildScopedExtensions(
  resolved: ResolvedMention[],
  getEntities: (type: string) => Entity[],
): Extension[] {
  const exts: Extension[] = [];
  const unresolvedByType = collectUnresolvedByType(resolved);
  const seen = new Set<string>();

  for (const r of resolved) {
    const key = `${r.prefix}:${r.entityType}`;
    if (seen.has(key) || !r.prefix) continue;
    seen.add(key);

    const cssClass = `cm-${r.entityType}-pill`;
    const colorVar = `--${r.entityType}-color`;
    const { extension } = createMentionDecorations(
      r.prefix,
      cssClass,
      colorVar,
    );
    const entities = getEntities(r.entityType);
    const metaMap = buildMentionMetaMap(entities, r.displayField);
    addUnresolvedPlaceholders(metaMap, unresolvedByType.get(r.entityType));
    exts.push(extension(metaMap));
  }

  return exts;
}

/**
 * Props for the inner single-mention renderer used by both modes.
 *
 * In list mode the parent `MentionView` mints a per-item moniker and
 * passes it down so each pill is its own leaf in the spatial graph; in
 * single mode the caller's props are forwarded directly.
 */
interface SingleMentionProps {
  resolved: ResolvedMention;
  extensions: Extension[];
  className?: string;
  scopeMoniker: string;
  showFocusBar?: boolean;
  extraCommands?: CommandDef[];
}

/** Render one mention: a FocusScope wrapping a one-line TextViewer. */
function SingleMention({
  resolved,
  extensions,
  className,
  scopeMoniker,
  showFocusBar,
  extraCommands,
}: SingleMentionProps) {
  const doc = `${resolved.prefix}${resolved.slug}`;

  return (
    <FocusScope
      moniker={asMoniker(scopeMoniker)}
      commands={extraCommands}
      className="inline"
      showFocusBar={showFocusBar}
    >
      <TextViewer text={doc} extensions={extensions} className={className} />
    </FocusScope>
  );
}

/**
 * MentionView — renders one or more entity mentions as CM6 widget pills.
 *
 * - Single mode: pass `entityType` + `id`.
 * - List mode: pass `items`.
 *
 * Both modes resolve entities from the store, build a CM6 doc like
 * `${prefix}${slug}`, and mount a read-only TextViewer with scoped
 * mention extensions. The visible pill text is produced by the CM6
 * widget (clipped display name), not by slug munging here.
 */
/** Resolve MentionView props into a uniform list of ResolvedMention entries. */
function useResolvedMentions(
  props: MentionViewProps,
  isListMode: boolean,
): ResolvedMention[] {
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  return useMemo<ResolvedMention[]>(() => {
    if (isListMode) {
      return (props.items ?? []).map((it) =>
        resolveMention(it, mentionableTypes, getEntities),
      );
    }
    if (props.entityType !== undefined && props.id !== undefined) {
      return [
        resolveMention(
          { entityType: props.entityType, id: props.id },
          mentionableTypes,
          getEntities,
        ),
      ];
    }
    return [];
  }, [
    isListMode,
    props.items,
    props.entityType,
    props.id,
    mentionableTypes,
    getEntities,
  ]);
}

/** Render the single-mode variant of MentionView. */
function MentionViewSingle({
  resolved,
  extensions,
  pillMoniker,
  props,
}: {
  resolved: ResolvedMention;
  extensions: Extension[];
  pillMoniker: string;
  props: MentionViewProps;
}) {
  const scopeMoniker = props.focusMoniker ?? pillMoniker;
  return (
    <SingleMention
      resolved={resolved}
      extensions={extensions}
      className={props.className}
      scopeMoniker={scopeMoniker}
      showFocusBar={props.showFocusBar}
      extraCommands={props.extraCommands}
    />
  );
}

/** Render the list-mode variant of MentionView. */
function MentionViewList({
  resolved,
  extensions,
  pillMonikers,
  mode,
  props,
}: {
  resolved: ResolvedMention[];
  extensions: Extension[];
  pillMonikers: string[];
  mode: "compact" | "full";
  props: MentionViewProps;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {resolved.map((r, i) => (
        <SingleMention
          key={pillMonikers[i]}
          resolved={r}
          extensions={extensions}
          className={props.className}
          scopeMoniker={pillMonikers[i]}
          showFocusBar={mode === "full" ? props.showFocusBar : false}
          extraCommands={props.extraCommands}
        />
      ))}
    </div>
  );
}

export function MentionView(props: MentionViewProps) {
  const { getEntities } = useEntityStore();

  const isListMode = props.items !== undefined;
  const mode = props.mode ?? "full";

  const resolved = useResolvedMentions(props, isListMode);

  const extensions = useMemo(
    () => buildScopedExtensions(resolved, getEntities),
    [resolved, getEntities],
  );

  const pillMonikers = useMemo(
    () => resolved.map((r) => buildMoniker(r.entityType, r.monikerId)),
    [resolved],
  );

  if (resolved.length === 0) return null;

  if (!isListMode) {
    return (
      <MentionViewSingle
        resolved={resolved[0]}
        extensions={extensions}
        pillMoniker={pillMonikers[0]}
        props={props}
      />
    );
  }

  return (
    <MentionViewList
      resolved={resolved}
      extensions={extensions}
      pillMonikers={pillMonikers}
      mode={mode}
      props={props}
    />
  );
}
