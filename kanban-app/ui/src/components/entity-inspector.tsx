import { useState, useCallback, useEffect, useMemo } from "react";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { resolveEditor } from "@/components/fields/editors";
import { Field } from "@/components/fields/field";
import { useSchema } from "@/lib/schema-context";
import {
  useInspectorNav,
  type UseInspectorNavReturn,
  type InspectorMode,
} from "@/hooks/use-inspector-nav";
import type { FieldDef, Entity } from "@/types/kanban";
import { FocusScope, FocusClaim } from "@/components/focus-scope";
import { useIsFocused, type ClaimPredicate } from "@/lib/entity-focus-context";
import { fieldMoniker } from "@/lib/moniker";
import { icons, HelpCircle } from "lucide-react";
import type { LucideIcon } from "lucide-react";

/** Convert kebab-case icon name (e.g. "file-text") to PascalCase key (e.g. "FileText"). */
function kebabToPascal(s: string): string {
  return s.replace(/(^|-)([a-z])/g, (_, _dash, c) => c.toUpperCase());
}

/** Resolve the lucide icon component from a field's `icon` property. */
function fieldIcon(field: FieldDef): LucideIcon {
  if (field.icon) {
    const key = kebabToPascal(field.icon);
    const Icon = icons[key as keyof typeof icons];
    if (Icon) return Icon;
  }
  return HelpCircle;
}

interface EntityInspectorProps {
  entity: Entity;
  /** Ref callback to expose the nav state to parent (InspectorFocusBridge). */
  navRef?: React.RefObject<UseInspectorNavReturn | null>;
}

/**
 * Generic entity inspector — renders all fields for any entity type,
 * grouped by section (header, body, footer) in entity definition order.
 *
 * Fields with `section: "hidden"` are not rendered.
 * Fields default to "body" if no section is specified.
 *
 * Navigation is pull-based: each field row's FocusScope gets claimWhen
 * predicates computed from its position in the field list. The FocusClaim
 * on mount focuses the first field. After that, navigation is purely
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

  const sections = useMemo(() => {
    const header: FieldDef[] = [];
    const body: FieldDef[] = [];
    const footer: FieldDef[] = [];
    for (const field of fields) {
      const section = field.section ?? "body";
      if (section === "hidden") continue;
      if (section === "header") header.push(field);
      else if (section === "footer") footer.push(field);
      else body.push(field);
    }
    return { header, body, footer };
  }, [fields]);

  /** Flat ordered list of all navigable fields (header → body → footer). */
  const navigableFields = useMemo(
    () => [...sections.header, ...sections.body, ...sections.footer],
    [sections],
  );

  const nav = useInspectorNav();

  // Expose nav to parent (InspectorFocusBridge) via ref
  if (navRef) navRef.current = nav;

  /** Monikers for all navigable fields, in flat order. */
  const fieldMonikers = useMemo(
    () => navigableFields.map((f) => fieldMoniker(entity.entity_type, entity.id, f.name)),
    [navigableFields, entity.entity_type, entity.id],
  );

  /** Check if a moniker is one of this inspector's fields or a descendant of one. */
  const isInspectorField = (f: string | null, isDescendantOf: (a: string) => boolean): boolean => {
    if (!f) return false;
    if (fieldMonikers.includes(f)) return true;
    // Check if focused element is a child of any field (e.g. a pill inside a badge-list)
    return fieldMonikers.some((m) => isDescendantOf(m));
  };

  /** ClaimWhen predicates for each field at index i. */
  const claimPredicates = useMemo(() => {
    return fieldMonikers.map((_, i) => {
      const predicates: ClaimPredicate[] = [];
      // nav.down: claim if the field above me (or a child of it) is focused
      if (i > 0) {
        const prev = fieldMonikers[i - 1];
        predicates.push({
          command: "nav.down",
          when: (f, isDescendantOf) => f === prev || isDescendantOf(prev),
        });
      }
      // nav.up: claim if the field below me (or a child of it) is focused
      if (i < fieldMonikers.length - 1) {
        const next = fieldMonikers[i + 1];
        predicates.push({
          command: "nav.up",
          when: (f, isDescendantOf) => f === next || isDescendantOf(next),
        });
      }
      // nav.first: claim if I'm the first field AND any sibling (or descendant) is focused
      if (i === 0) {
        predicates.push({
          command: "nav.first",
          when: (f, isDescendantOf) =>
            isInspectorField(f, isDescendantOf) && f !== fieldMonikers[0],
        });
      }
      // nav.last: claim if I'm the last field AND any sibling (or descendant) is focused
      if (i === fieldMonikers.length - 1) {
        predicates.push({
          command: "nav.last",
          when: (f, isDescendantOf) =>
            isInspectorField(f, isDescendantOf) && f !== fieldMonikers[fieldMonikers.length - 1],
        });
      }
      return predicates;
    });
  }, [fieldMonikers]);

  // FocusClaim moniker: always the first field (for initial mount focus)
  const claimMoniker = fieldMonikers[0] ?? `inspector:${entity.entity_type}:${entity.id}`;

  if (fields.length === 0) {
    return <p className="text-sm text-muted-foreground">Loading schema...</p>;
  }

  /** Track the running index across sections so each FieldRow knows its flat position. */
  let flatIndex = 0;

  const renderField = (field: FieldDef, showLabel = true) => {
    const index = flatIndex++;
    return (
      <FieldRow
        key={field.name}
        field={field}
        entity={entity}
        showLabel={showLabel}
        claimWhen={claimPredicates[index]}
        inspectorMode={nav.mode}
        onExitEdit={nav.exitEdit}
        onEnterEdit={nav.enterEdit}
      />
    );
  };

  return (
    <div data-testid="entity-inspector">
      <FocusClaim moniker={claimMoniker} />
      {sections.header.length > 0 && (
        <div className="space-y-2" data-testid="inspector-header">
          {sections.header.map((f) => renderField(f, false))}
        </div>
      )}
      {sections.header.length > 0 && sections.body.length > 0 && (
        <div className="my-3 h-px bg-border" />
      )}
      {sections.body.length > 0 && (
        <div className="space-y-3" data-testid="inspector-body">
          {sections.body.map((f) => renderField(f))}
        </div>
      )}
      {sections.footer.length > 0 && (
        <>
          <div className="my-3 h-px bg-border" />
          <div className="space-y-3" data-testid="inspector-footer">
            {sections.footer.map((f) => renderField(f))}
          </div>
        </>
      )}
    </div>
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
  const [editing, setEditing] = useState(false);

  const editable = isEditable(field);
  const scopeMoniker = fieldMoniker(entity.entity_type, entity.id, field.name);
  const isFocused = useIsFocused(scopeMoniker);
  const shouldEdit = isFocused && inspectorMode === "edit" && editable;

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

  const content = (
    <Field
      fieldDef={field}
      entityType={entity.entity_type}
      entityId={entity.id}
      mode="full"
      editing={editing && editable}
      onEdit={handleEdit}
      onDone={handleDone}
      onCancel={handleCancel}
    />
  );

  const Icon = field.icon ? fieldIcon(field) : null;
  const tip = field.description || fieldLabel(field);

  if (!showLabel && !Icon) {
    return (
      <FocusScope
        moniker={scopeMoniker}
        commands={[]}
        claimWhen={claimWhen}
        data-testid={`field-row-${field.name}`}
      >
        {content}
      </FocusScope>
    );
  }

  return (
    <FocusScope
      moniker={scopeMoniker}
      commands={[]}
      claimWhen={claimWhen}
      data-testid={`field-row-${field.name}`}
      className="flex items-start gap-2"
    >
      {Icon && (
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
      )}
      <div className="flex-1 min-w-0">{content}</div>
    </FocusScope>
  );
}

/** Check if a field is editable in the inspector — driven by the field's editor property. */
function isEditable(field: FieldDef): boolean {
  return resolveEditor(field) !== "none";
}

function fieldLabel(field: FieldDef): string {
  return field.name.replace(/_/g, " ");
}
