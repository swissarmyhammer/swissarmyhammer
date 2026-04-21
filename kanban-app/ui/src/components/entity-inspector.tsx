import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { resolveEditor } from "@/components/fields/editors";
import {
  Field,
  getDisplayIsEmpty,
  getDisplayIconOverride,
  getDisplayTooltipOverride,
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
import { FocusScope } from "@/components/focus-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { useIsFocused, type ClaimPredicate } from "@/lib/entity-focus-context";
import { fieldMoniker } from "@/lib/moniker";
import { fieldIcon } from "@/components/fields/field-icon";
import { HelpCircle, type LucideIcon } from "lucide-react";

interface EntityInspectorProps {
  entity: Entity;
  /** Ref callback to expose the nav state to parent (InspectorFocusBridge). */
  navRef?: React.RefObject<UseInspectorNavReturn | null>;
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
 * Navigation is pull-based: each field row's FocusScope gets claimWhen
 * predicates computed from its position in the field list. A mount effect
 * focuses the first field via setFocus. After that, navigation is purely
 * driven by broadcastNavCommand triggering claimWhen predicates.
 *
 * Pulls everything from context:
 * - Field definitions and ordering from SchemaContext
 * - Save function from FieldUpdateContext (used internally by FieldRow)
 */
export function EntityInspector({ entity, navRef }: EntityInspectorProps) {
  const { getSchema } = useSchema();
  const schema = getSchema(entity.entity_type);
  const fields = schema?.fields ?? [];
  const visibleFields = useVisibleFields(entity, fields);
  const sections = useEntitySections(schema?.entity.sections, visibleFields);
  const navigableFields = useMemo(
    () => sections.flatMap((s) => s.fields),
    [sections],
  );
  const fieldMonikers = useMemo(
    () =>
      navigableFields.map((f) =>
        fieldMoniker(entity.entity_type, entity.id, f.name),
      ),
    [navigableFields, entity.entity_type, entity.id],
  );
  const claimPredicates = useFieldClaimPredicates(fieldMonikers);

  const nav = useInspectorNav();
  // Expose nav to parent (InspectorFocusBridge) via ref
  if (navRef) navRef.current = nav;

  useFirstFieldFocus(fieldMonikers[0]);

  if (fields.length === 0) {
    return <p className="text-sm text-muted-foreground">Loading schema...</p>;
  }
  return (
    <InspectorSections
      entity={entity}
      sections={sections}
      claimPredicates={claimPredicates}
      nav={nav}
    />
  );
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
 * Compute the `claimWhen` predicate list for every navigable field, indexed
 * by its flat position in the inspector. Predicates wire up nav.up, nav.down,
 * nav.left, nav.first, and nav.last so keyboard navigation walks the list.
 */
function useFieldClaimPredicates(fieldMonikers: string[]): ClaimPredicate[][] {
  return useMemo(
    () => fieldMonikers.map((_, i) => predicatesForField(fieldMonikers, i)),
    [fieldMonikers],
  );
}

/** Build the ClaimPredicate list for the field at index `i` in `monikers`. */
function predicatesForField(monikers: string[], i: number): ClaimPredicate[] {
  const predicates: ClaimPredicate[] = [];
  // nav.down: claim if the field above me (or a child of it) is focused
  if (i > 0) {
    const prev = monikers[i - 1];
    predicates.push({
      command: "nav.down",
      when: (f, isDescendantOf) => f === prev || isDescendantOf(prev),
    });
  }
  // nav.up: claim if the field below me (or a child of it) is focused
  if (i < monikers.length - 1) {
    const next = monikers[i + 1];
    predicates.push({
      command: "nav.up",
      when: (f, isDescendantOf) => f === next || isDescendantOf(next),
    });
  }
  // nav.left: claim if a descendant (e.g. first pill in a badge-list) is focused.
  // Pill predicates register before field rows (children before parents), so a
  // middle pill's nav.left fires first.  Only when no pill matches (the first
  // pill has no nav.left predicate) does this field-row predicate win.
  predicates.push({
    command: "nav.left",
    when: (f, isDescendantOf) =>
      f !== monikers[i] && isDescendantOf(monikers[i]),
  });
  predicates.push(...edgePredicates(monikers, i));
  return predicates;
}

/**
 * nav.first / nav.last predicates for edge fields. The first field claims
 * nav.first when any other inspector field is focused; the last claims
 * nav.last symmetrically. Middle fields return an empty array.
 */
function edgePredicates(monikers: string[], i: number): ClaimPredicate[] {
  const edges: ClaimPredicate[] = [];
  if (i === 0) {
    edges.push({
      command: "nav.first",
      when: (f, isDescendantOf) =>
        isInspectorField(monikers, f, isDescendantOf) && f !== monikers[0],
    });
  }
  if (i === monikers.length - 1) {
    edges.push({
      command: "nav.last",
      when: (f, isDescendantOf) =>
        isInspectorField(monikers, f, isDescendantOf) &&
        f !== monikers[monikers.length - 1],
    });
  }
  return edges;
}

/** Check if a moniker is one of the inspector's fields or a descendant of one. */
function isInspectorField(
  monikers: string[],
  f: string | null,
  isDescendantOf: (a: string) => boolean,
): boolean {
  if (!f) return false;
  if (monikers.includes(f)) return true;
  // Check if focused element is a child of any field (e.g. a pill inside a badge-list)
  return monikers.some((m) => isDescendantOf(m));
}

/**
 * Focus the first field on mount and restore the previously focused element
 * on unmount. Intentionally reruns only when the first-field moniker changes,
 * not when focus state changes under us (see inline eslint-disable).
 */
function useFirstFieldFocus(firstFieldMoniker: string | undefined): void {
  const { setFocus, focusedMoniker } = useEntityFocus();
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;
  const prevFocusRef = useRef<string | null>(null);
  const mountedRef = useRef(false);

  useEffect(() => {
    if (!firstFieldMoniker) return;
    // Only capture previous focus on true first mount, not on re-runs
    if (!mountedRef.current) {
      prevFocusRef.current = focusedMoniker;
      mountedRef.current = true;
    }
    setFocusRef.current(firstFieldMoniker);
    return () => {
      setFocusRef.current(prevFocusRef.current);
      mountedRef.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [firstFieldMoniker]);
}

interface InspectorSectionsProps {
  entity: Entity;
  sections: ResolvedSection[];
  claimPredicates: ClaimPredicate[][];
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
 * `flatIndex` walks the already-flattened navigable field order so each
 * `FieldRow` receives its matching pull-based claim predicates.
 */
function InspectorSections({
  entity,
  sections,
  claimPredicates,
  nav,
}: InspectorSectionsProps) {
  /** Track the running index across sections so each FieldRow knows its flat position. */
  const flatIndex = { i: 0 };
  const rowFor = (field: FieldDef, showLabel = true) => (
    <FieldRow
      key={field.name}
      field={field}
      entity={entity}
      showLabel={showLabel}
      claimWhen={claimPredicates[flatIndex.i++]}
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
  claimWhen?: ClaimPredicate[];
  inspectorMode?: InspectorMode;
  onExitEdit?: () => void;
  onEnterEdit?: () => void;
}

/**
 * A single field row in the inspector. Manages editing state.
 * Field handles data binding, save, and display/editor dispatch.
 *
 * Wrapped in a FocusScope so the entity-focus system drives the
 * data-focused attribute — no explicit `focused` prop needed.
 *
 * Uses useIsFocused to determine if this row is the focused field.
 * Editing is triggered when isFocused AND inspectorMode === "edit".
 *
 * @param claimWhen - Predicates for pull-based navigation via broadcastNavCommand
 * @param inspectorMode - Current inspector mode (normal or edit)
 * @param onExitEdit - Callback to tell the inspector nav that editing is done
 * @param onEnterEdit - Callback to enter edit mode on the inspector
 */
function FieldRow({
  field,
  entity,
  showLabel = true,
  claimWhen,
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

  // Inspector shows a HelpCircle fallback for icon names that don't resolve
  // to a known lucide component (legacy behavior). The card does not — see
  // fields/field-icon.ts for the shared null-returning utility.
  //
  // When the display registers an iconOverride, call it with the current value
  // to get a dynamic, value-dependent icon. Falls back to the static YAML icon.
  const staticIcon = field.icon ? (fieldIcon(field) ?? HelpCircle) : null;
  const overrideFn = getDisplayIconOverride(field.display ?? "");
  const overrideResult = overrideFn
    ? overrideFn(entity.fields[field.name])
    : null;
  const Icon = overrideResult ?? staticIcon;

  // When the display registers a tooltipOverride, call it with the current
  // value to get a dynamic, value-dependent tooltip. Falls back to the static
  // YAML description or the humanised field name.
  const staticTip = field.description || fieldLabel(field);
  const tooltipOverrideFn = getDisplayTooltipOverride(field.display ?? "");
  const overrideTip = tooltipOverrideFn
    ? tooltipOverrideFn(entity.fields[field.name])
    : null;
  const tip = overrideTip ?? staticTip;
  const content = (
    <FieldContent field={field} entity={entity} editState={editState} />
  );
  const bare = !showLabel && !Icon;

  return (
    <FocusScope
      moniker={scopeMoniker}
      claimWhen={claimWhen}
      data-testid={`field-row-${field.name}`}
      className={bare ? undefined : "flex items-start gap-2"}
    >
      {Icon && <FieldIconTooltip Icon={Icon} tip={tip} />}
      {bare ? content : <div className="flex-1 min-w-0">{content}</div>}
    </FocusScope>
  );
}

/** Renders the inner Field editor/display for a row. */
function FieldContent({
  field,
  entity,
  editState,
}: {
  field: FieldDef;
  entity: Entity;
  editState: ReturnType<typeof useFieldEditing>;
}) {
  const editable = isEditable(field);
  return (
    <Field
      fieldDef={field}
      entityType={entity.entity_type}
      entityId={entity.id}
      mode="full"
      editing={editState.editing && editable}
      onEdit={editState.handleEdit}
      onDone={editState.handleDone}
      onCancel={editState.handleCancel}
    />
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

/** Tooltip-wrapped field icon badge used in the inspector's field rows. */
function FieldIconTooltip({ Icon, tip }: { Icon: LucideIcon; tip: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="h-5 inline-flex items-center shrink-0 text-muted-foreground">
          <Icon size={14} />
        </span>
      </TooltipTrigger>
      <TooltipContent side="left" align="start">
        {tip}
      </TooltipContent>
    </Tooltip>
  );
}

/** Check if a field is editable in the inspector — driven by the field's editor property. */
function isEditable(field: FieldDef): boolean {
  return resolveEditor(field) !== "none";
}

function fieldLabel(field: FieldDef): string {
  return field.name.replace(/_/g, " ");
}
