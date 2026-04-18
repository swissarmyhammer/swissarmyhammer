import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { resolveEditor } from "@/components/fields/editors";
import { Field, getDisplayIsEmpty } from "@/components/fields/field";
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
import { useEntityFocus, useIsFocused } from "@/lib/entity-focus-context";
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
 * Navigation between field rows is handled by spatial nav — each row's
 * FocusScope registers its DOM rect with Rust, which resolves directional
 * movement via rect geometry. A mount effect focuses the first field via
 * setFocus.
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
  const firstMoniker = useMemo(
    () =>
      navigableFields.length > 0
        ? fieldMoniker(entity.entity_type, entity.id, navigableFields[0].name)
        : undefined,
    [navigableFields, entity.entity_type, entity.id],
  );

  const nav = useInspectorNav();
  // Expose nav to parent (InspectorFocusBridge) via ref
  if (navRef) navRef.current = nav;

  useFirstFieldFocus(firstMoniker);

  if (fields.length === 0) {
    return <p className="text-sm text-muted-foreground">Loading schema...</p>;
  }
  return (
    <InspectorSections
      entity={entity}
      sections={sections}
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
 * Filtering runs *before* sectioning so keyboard nav indexes stay
 * contiguous — there is no way to focus a hidden row.
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
 * on unmount. Intentionally reruns only when the first-field moniker changes,
 * not when focus state changes under us (see inline eslint-disable).
 */
function useFirstFieldFocus(firstFieldMoniker: string | undefined): void {
  const { setFocus, getFocusedMoniker } = useEntityFocus();
  const setFocusRef = useRef(setFocus);
  setFocusRef.current = setFocus;
  const getFocusedMonikerRef = useRef(getFocusedMoniker);
  getFocusedMonikerRef.current = getFocusedMoniker;
  const prevFocusRef = useRef<string | null>(null);
  const mountedRef = useRef(false);

  useEffect(() => {
    if (!firstFieldMoniker) return;
    // Only capture previous focus on true first mount, not on re-runs.
    // Read the focused moniker imperatively from the store — this effect
    // intentionally does not subscribe to focus changes.
    if (!mountedRef.current) {
      prevFocusRef.current = getFocusedMonikerRef.current();
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
 * Navigation between rows is handled by spatial nav (DOM rect resolution).
 */
function InspectorSections({
  entity,
  sections,
  nav,
}: InspectorSectionsProps) {
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
 * Wrapped in a FocusScope so the entity-focus system drives the
 * data-focused attribute — no explicit `focused` prop needed.
 *
 * Uses useIsFocused to determine if this row is the focused field.
 * Editing is triggered when isFocused AND inspectorMode === "edit".
 * Navigation between fields is handled by spatial nav (DOM rects in Rust).
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

  // Inspector shows a HelpCircle fallback for icon names that don't resolve
  // to a known lucide component (legacy behavior). The card does not — see
  // fields/field-icon.ts for the shared null-returning utility.
  const Icon = field.icon ? (fieldIcon(field) ?? HelpCircle) : null;
  const tip = field.description || fieldLabel(field);
  const content = (
    <FieldContent field={field} entity={entity} editState={editState} />
  );
  const bare = !showLabel && !Icon;

  return (
    <FocusScope
      moniker={scopeMoniker}
      commands={[]}
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
        <span className="mt-0.5 shrink-0 text-muted-foreground">
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
