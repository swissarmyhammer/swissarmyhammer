import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import { resolveEditor } from "@/components/fields/editors";
import {
  Field,
  getDisplayIsEmpty,
  getDisplayIconOverride,
} from "@/components/fields/field";
import { useSchema } from "@/lib/schema-context";
import {
  useInspectorNav,
  type UseInspectorNavReturn,
  type InspectorMode,
} from "@/hooks/use-inspector-nav";
import {
  useEntitySections,
  type ResolvedSection,
} from "@/hooks/use-entity-sections";
import type { FieldDef, Entity } from "@/types/kanban";
import {
  useFocusActions,
  useFocusedMonikerRef,
  useIsFocused,
} from "@/lib/entity-focus-context";
import { fieldMoniker } from "@/lib/moniker";
import { useFullyQualifiedMoniker } from "@/components/fully-qualified-moniker-context";
import {
  asSegment,
  composeFq,
  type FullyQualifiedMoniker,
} from "@/types/spatial";

interface EntityInspectorProps {
  entity: Entity;
}

/**
 * Generic entity inspector — renders all fields for any entity type,
 * grouped by the declarative sections on the entity schema.
 *
 * Section layout comes from `entity.sections` in the YAML schema: each
 * entry carries an `id`, optional `label`, and `on_card` flag. Fields
 * reference a section by their own `section: "<id>"` value. Entities
 * that omit `sections` fall back to the implicit `header`/`body`/`footer`
 * layout so legacy schemas render as before.
 *
 * Fields with `section: "hidden"` are not rendered. Fields whose
 * `section` value is not in the declared list fall through to `body`
 * so schema typos stay visible.
 *
 * Navigation is structural: each `<Field>` registers itself as a
 * `<FocusZone>` keyed by `field:{type}:{id}.{name}` (see
 * `01KQ5QB6F4MTD35GBTARJH4JEW`). The label and pills inside become
 * leaves. Within-field nav (e.g. between pills) is iter 0 of the
 * unified cascade (same-level peers inside the field zone); across-field
 * nav is iter 1 (the cascade escalates to the parent zone and lands on
 * the neighbouring field zone, which the React adapter drills back
 * into). A mount effect focuses the first field via `setFocus` so the
 * inspector opens with a sensible cursor.
 *
 * Pulls everything from context:
 * - Field definitions and ordering from SchemaContext
 * - Save function from FieldUpdateContext (used internally by FieldRow)
 */
export function EntityInspector({ entity }: EntityInspectorProps) {
  const { getSchema } = useSchema();
  const schema = getSchema(entity.entity_type);
  const fields = schema?.fields ?? [];
  const visibleFields = useVisibleFields(entity, fields);
  const sections = useEntitySections(schema?.entity.sections, visibleFields);
  // The inspector mounts directly under the inspector layer's FQM; field
  // zones register at that layer root, so the first-field FQM is composed
  // by appending the field segment under the enclosing FQM (the inspector
  // layer FQ, since no intermediate zone wraps the inspector body).
  const parentFq = useFullyQualifiedMoniker();
  const firstFieldFq = useMemo<FullyQualifiedMoniker | undefined>(() => {
    const first = sections.flatMap((s) => s.fields)[0];
    if (!first) return undefined;
    const segment = asSegment(
      fieldMoniker(entity.entity_type, entity.id, first.name),
    );
    return composeFq(parentFq, segment);
  }, [parentFq, sections, entity.entity_type, entity.id]);

  const nav = useInspectorNav();

  useFirstFieldFocus(firstFieldFq);

  if (fields.length === 0) {
    return <p className="text-sm text-muted-foreground">Loading schema...</p>;
  }
  return <InspectorSections entity={entity} sections={sections} nav={nav} />;
}

/**
 * Filter out non-editable fields whose current value is "empty" per the
 * display's registered `isEmpty` predicate.
 *
 * Only fields with `editor: "none"` (computed / display-only) are eligible —
 * editable fields with empty values must still render so the user can click
 * to edit them. A display without a registered `isEmpty` predicate is always
 * considered non-empty (opt-in behaviour).
 *
 * Filtering runs *before* sectioning and predicate assembly so keyboard nav
 * indexes stay contiguous — there is no way to focus a hidden row.
 */
function useVisibleFields(entity: Entity, fields: FieldDef[]): FieldDef[] {
  return useMemo(() => {
    return fields.filter((field) => {
      if (resolveEditor(field) !== "none") return true;
      const isEmpty = getDisplayIsEmpty(field.display ?? "");
      if (!isEmpty) return true;
      return !isEmpty(entity.fields[field.name]);
    });
  }, [entity, fields]);
}

/**
 * Focus the first field on mount and restore the previously focused element
 * on unmount. Intentionally reruns only when the first-field FQM changes,
 * not when focus state changes under us (see inline eslint-disable).
 */
function useFirstFieldFocus(
  firstFieldFq: FullyQualifiedMoniker | undefined,
): void {
  const { setFocus } = useFocusActions();
  // Ref-style read: we capture the previously focused FQM exactly once
  // at mount, so subscribing (and re-rendering this hook's caller on every
  // focus move) would be pure waste. The ref mirrors focus via subscribeAll.
  const focusedMonikerRef = useFocusedMonikerRef();
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;
  const prevFocusRef = useRef<FullyQualifiedMoniker | null>(null);
  const mountedRef = useRef(false);

  useEffect(() => {
    if (!firstFieldFq) return;
    // Only capture previous focus on true first mount, not on re-runs
    if (!mountedRef.current) {
      prevFocusRef.current = focusedMonikerRef.current;
      mountedRef.current = true;
    }
    setFocusRef.current(firstFieldFq);
    return () => {
      setFocusRef.current(prevFocusRef.current);
      mountedRef.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firstFieldFq]);
}

interface InspectorSectionsProps {
  entity: Entity;
  sections: ResolvedSection[];
  nav: UseInspectorNavReturn;
}

/**
 * Renders the entity's fields grouped into declared sections.
 *
 * Iterates `sections` in declared order, skipping empty sections (no
 * dangling divider). A thin horizontal divider sits between consecutive
 * non-empty sections. Sections with a `label` render a small uppercase
 * heading above the rows (the inspector-only affordance — cards stay
 * dense). The `header` section renders without field labels (legacy
 * compact styling); all other sections render field labels.
 *
 * Each `FieldRow` contains a `<Field>` that registers as its own
 * `<FocusZone>` — the unified cascade drives keyboard navigation
 * between rows (iter 1 escalates to the parent zone and lands on the
 * neighbouring field zone) and within rows (iter 0 same-level peers).
 */
function InspectorSections({ entity, sections, nav }: InspectorSectionsProps) {
  const rowFor = (field: FieldDef, showLabel = true) => (
    <FieldRow
      key={field.name}
      field={field}
      entity={entity}
      showLabel={showLabel}
      inspectorMode={nav.mode}
      onExitEdit={nav.exitEdit}
      onEnterEdit={nav.enterEdit}
    />
  );

  /** Track whether we've already rendered a non-empty section so we know when to draw dividers. */
  let renderedAny = false;
  return (
    <div data-testid="entity-inspector">
      {sections.map((section) => {
        if (section.fields.length === 0) return null;
        const showDivider = renderedAny;
        renderedAny = true;
        return (
          <SectionBlock
            key={section.def.id}
            section={section}
            rowFor={rowFor}
            showDivider={showDivider}
          />
        );
      })}
    </div>
  );
}

interface SectionBlockProps {
  section: ResolvedSection;
  rowFor: (field: FieldDef, showLabel?: boolean) => React.ReactElement;
  showDivider: boolean;
}

/**
 * Renders one non-empty inspector section: an optional top divider, an
 * optional small label heading, and the section's field rows.
 *
 * The `header` section uses tighter `space-y-2` spacing and drops field
 * labels (for compact entity titles); every other section uses
 * `space-y-3` and keeps labels. This matches the pre-declarative
 * styling so default-layout entities (tag, actor, …) render unchanged.
 */
function SectionBlock({ section, rowFor, showDivider }: SectionBlockProps) {
  const { id, label } = section.def;
  const isHeader = id === "header";
  return (
    <>
      {showDivider && <div className="my-3 h-px bg-border" />}
      {label && (
        <div
          className="text-[11px] uppercase tracking-wide text-muted-foreground/70 mb-1"
          data-testid={`inspector-section-label-${id}`}
        >
          {label}
        </div>
      )}
      <div
        className={isHeader ? "space-y-2" : "space-y-3"}
        data-testid={`inspector-section-${id}`}
      >
        {section.fields.map((f) => rowFor(f, !isHeader))}
      </div>
    </>
  );
}

interface FieldRowProps {
  field: FieldDef;
  entity: Entity;
  showLabel?: boolean;
  inspectorMode?: InspectorMode;
  onExitEdit?: () => void;
  onEnterEdit?: () => void;
}

/**
 * A single field row in the inspector. Manages editing state.
 * Field handles data binding, save, and display/editor dispatch.
 *
 * The `<Field>` itself registers as a `<FocusZone>` keyed by
 * `field:{type}:{id}.{name}` (introduced in card
 * `01KQ5QB6F4MTD35GBTARJH4JEW`). The icon — when the field has one — now
 * renders *inside* that `<FocusZone>` via `<Field withIcon />`, so a
 * click on the icon dispatches `spatial_focus` for the field zone and
 * the focus bar paints to the LEFT of the icon (the indicator's
 * `-left-2` offset is relative to the zone wrapper, which now contains
 * the icon as its leftmost child).
 *
 * The row's outer `<div>` carries the `data-testid` so existing test
 * selectors continue to find it; everything visible (icon, content,
 * focus indicator) lives inside the field zone underneath.
 *
 * `showFocusBar={true}` is passed through to Field so the inspector row
 * keeps its visible focus bar — the inspector is one of the surfaces
 * where each row IS a focusable item in its own right.
 *
 * Uses useIsFocused to determine if this row is the focused field.
 * Editing is triggered when isFocused AND inspectorMode === "edit".
 *
 * @param inspectorMode - Current inspector mode (normal or edit)
 * @param onExitEdit - Callback to tell the inspector nav that editing is done
 * @param onEnterEdit - Callback to enter edit mode on the inspector
 */
function FieldRow({
  field,
  entity,
  showLabel = true,
  inspectorMode,
  onExitEdit,
  onEnterEdit,
}: FieldRowProps) {
  const editable = isEditable(field);
  const scopeMoniker = fieldMoniker(entity.entity_type, entity.id, field.name);
  const isFocused = useIsFocused(scopeMoniker);
  const shouldEdit = isFocused && inspectorMode === "edit" && editable;
  const editState = useFieldEditing(
    editable,
    shouldEdit,
    onEnterEdit,
    onExitEdit,
  );

  // Decide whether the row uses the icon-and-content layout. Mirrors the
  // legacy `bare = !showLabel && !Icon` check: when the row is in a
  // section that hides labels (header) AND the field has no resolvable
  // icon (no static YAML icon AND the display registers no
  // `iconOverride`), the row renders bare — no flex wrapper, no icon
  // slot. In every other case the row uses `<Field withIcon />` so the
  // icon (when present) renders inside the field zone.
  const hasIcon =
    field.icon !== undefined ||
    getDisplayIconOverride(field.display ?? "") !== undefined;
  return (
    <div data-testid={`field-row-${field.name}`}>
      <Field
        fieldDef={field}
        entityType={entity.entity_type}
        entityId={entity.id}
        mode="full"
        editing={editState.editing && editable}
        onEdit={editState.handleEdit}
        onDone={editState.handleDone}
        onCancel={editState.handleCancel}
        showFocusBar
        withIcon={showLabel || hasIcon}
      />
    </div>
  );
}

/** Manages a field's editing state and edit/done/cancel callbacks. */
function useFieldEditing(
  editable: boolean,
  shouldEdit: boolean,
  onEnterEdit?: () => void,
  onExitEdit?: () => void,
) {
  const [editing, setEditing] = useState(false);

  /** Sync inspector-driven edit mode into local editing state. */
  useEffect(() => {
    if (shouldEdit) {
      setEditing(true);
    }
  }, [shouldEdit]);

  const handleEdit = useCallback(() => {
    if (editable) {
      onEnterEdit?.();
      setEditing(true);
    }
  }, [editable, onEnterEdit]);

  const handleDone = useCallback(() => {
    setEditing(false);
    onExitEdit?.();
  }, [onExitEdit]);

  const handleCancel = useCallback(() => {
    setEditing(false);
    onExitEdit?.();
  }, [onExitEdit]);

  return { editing, handleEdit, handleDone, handleCancel };
}

/** Check if a field is editable in the inspector — driven by the field's editor property. */
function isEditable(field: FieldDef): boolean {
  return resolveEditor(field) !== "none";
}
