/**
 * Group an entity's fields into ordered, metadata-declared sections.
 *
 * The entity YAML may declare a `sections` list — each entry carries an
 * `id`, an optional `label`, and an `on_card` flag. Fields reference a
 * section by setting `section: "<id>"`; the renderer (inspector or card)
 * groups fields into the declared sections and places any field whose
 * `section` value is not in the declared list into the `body` section
 * (so schema typos don't vanish data).
 *
 * When the entity omits `sections` entirely, renderers fall back to the
 * implicit three-section layout (`header`, `body`, `footer`) used before
 * declarative sections existed. This keeps tag, actor, board, and column
 * inspectors rendering as they did before.
 *
 * Fields with `section: "hidden"` are never returned — use
 * `useVisibleFields` (or the inspector's filter) to drop empty
 * display-only fields before calling this hook.
 */

import { useMemo } from "react";
import type { FieldDef, SectionDef } from "@/types/kanban";

/**
 * A section definition paired with the ordered list of fields whose
 * `section` value resolves to that section.
 */
export interface ResolvedSection {
  def: SectionDef;
  fields: FieldDef[];
}

/**
 * Default section list used when an entity omits `sections` entirely.
 * Preserves the legacy header/body/footer layout for entities that
 * predate declarative sections.
 */
const DEFAULT_SECTIONS: readonly SectionDef[] = [
  { id: "header" },
  { id: "body" },
  { id: "footer" },
];

/**
 * Group `fields` into the sections declared on `entitySections`,
 * preserving declared section order and field order within each section.
 *
 * @param entitySections Ordered sections from the entity schema (may be
 * absent/empty; falls back to default layout).
 * @param fields Visible fields in declared order (already filtered for
 * `section: "hidden"` and empty-computed-field dropouts).
 * @returns The sections paired with their ordered fields. Empty sections
 * are still returned so callers can decide whether to render a
 * placeholder or skip; most callers filter them out at render time.
 */
export function resolveEntitySections(
  entitySections: readonly SectionDef[] | undefined,
  fields: readonly FieldDef[],
): ResolvedSection[] {
  const sections: readonly SectionDef[] =
    entitySections && entitySections.length > 0
      ? entitySections
      : DEFAULT_SECTIONS;

  // Build a lookup from section id -> accumulator slot.
  const buckets = new Map<string, FieldDef[]>();
  for (const section of sections) {
    buckets.set(section.id, []);
  }

  // Identify the fallback bucket (where unknown/missing section values go).
  // We prefer "body" when declared; otherwise fall through to the first
  // non-header/footer section; otherwise the first section of any kind.
  const fallbackId = pickFallbackId(sections);

  for (const field of fields) {
    const sectionId = field.section ?? "body";
    if (sectionId === "hidden") continue;
    const bucket = buckets.get(sectionId) ?? buckets.get(fallbackId);
    if (bucket) bucket.push(field);
  }

  return sections.map((def) => ({
    def,
    fields: buckets.get(def.id) ?? [],
  }));
}

/** Pick the section id that collects fields whose `section` is unknown. */
function pickFallbackId(sections: readonly SectionDef[]): string {
  for (const s of sections) {
    if (s.id === "body") return s.id;
  }
  for (const s of sections) {
    if (s.id !== "header" && s.id !== "footer") return s.id;
  }
  return sections[0]?.id ?? "body";
}

/**
 * Memoised React hook wrapper around `resolveEntitySections`.
 *
 * Re-runs only when either input identity changes, which matches the
 * stable-by-memo schema and field arrays the inspector/card receive from
 * `SchemaContext` and the per-entity visibility filter.
 */
export function useEntitySections(
  entitySections: readonly SectionDef[] | undefined,
  fields: readonly FieldDef[],
): ResolvedSection[] {
  return useMemo(
    () => resolveEntitySections(entitySections, fields),
    [entitySections, fields],
  );
}
