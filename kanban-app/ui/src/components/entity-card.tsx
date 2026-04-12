import { forwardRef, memo, useCallback, useMemo, useState } from "react";
import { GripVertical, Info, type LucideIcon } from "lucide-react";
import { FocusScope } from "@/components/focus-scope";
import { Field } from "@/components/fields/field";
import { fieldIcon } from "@/components/fields/field-icon";
import { useSchema } from "@/lib/schema-context";
import { useEntityCommands } from "@/lib/entity-commands";
import { useDispatchCommand, type CommandDef } from "@/lib/command-scope";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { ClaimPredicate } from "@/lib/entity-focus-context";
import type { Entity, FieldDef } from "@/types/kanban";

interface EntityCardProps {
  entity: Entity;
  dragHandleProps?: Record<string, unknown>;
  style?: React.CSSProperties;
  draggable?: boolean;
  onDragStart?: (e: React.DragEvent) => void;
  onDragEnd?: (e: React.DragEvent) => void;
  /** Additional commands to append to the entity's context menu. */
  extraCommands?: CommandDef[];
  /** Predicates for pull-based navigation via broadcastNavCommand. */
  claimWhen?: ClaimPredicate[];
}

/**
 * Compact card view for an entity on the board.
 *
 * Renders header-section fields from the schema using the same
 * dispatch logic as EntityInspector — no hardcoded field names.
 */
export const EntityCard = memo(
  forwardRef<HTMLDivElement, EntityCardProps>(function EntityCard(props, ref) {
    const {
      entity,
      dragHandleProps,
      style,
      draggable,
      onDragStart,
      onDragEnd,
      extraCommands,
      claimWhen,
      ...rest
    } = props;
    const cardFields = useHeaderFields(entity.entity_type);
    const commands = useEntityCommands(
      entity.entity_type,
      entity.id,
      entity,
      extraCommands,
    );

    return (
      <FocusScope
        moniker={entity.moniker}
        commands={commands}
        claimWhen={claimWhen}
        className="entity-card-focus"
      >
        <div
          ref={ref}
          style={style}
          data-entity-card={entity.id}
          draggable={draggable}
          onDragStart={onDragStart}
          onDragEnd={onDragEnd}
          className="rounded-md bg-card px-3 py-2 text-sm border border-border relative group flex items-start gap-2 overflow-hidden"
          {...rest}
        >
          <DragHandle dragHandleProps={dragHandleProps} />
          <CardFields fields={cardFields} entity={entity} />
          <InspectButton moniker={entity.moniker} />
        </div>
      </FocusScope>
    );
  }),
);

/** Header-section fields for a given entity type, memoised by schema identity. */
function useHeaderFields(entityType: string): FieldDef[] {
  const { getSchema } = useSchema();
  const schema = getSchema(entityType);
  return useMemo(
    () => (schema?.fields ?? []).filter((f) => f.section === "header"),
    [schema],
  );
}

/** Props for the drag handle button. */
interface DragHandleProps {
  dragHandleProps?: Record<string, unknown>;
}

/** Drag handle button — stops click propagation and wires drag handle props. */
function DragHandle({ dragHandleProps }: DragHandleProps) {
  return (
    <button
      type="button"
      className="shrink-0 mt-0.5 p-0 text-muted-foreground/50 hover:text-muted-foreground cursor-grab active:cursor-grabbing touch-none"
      onClick={(e) => e.stopPropagation()}
      {...dragHandleProps}
    >
      <GripVertical className="h-4 w-4" />
    </button>
  );
}

/** Props for the card field list. */
interface CardFieldsProps {
  fields: FieldDef[];
  entity: Entity;
}

/**
 * Renders the card's header-section fields with icon tooltips and
 * per-field editing state. Extracted so that EntityCard itself stays
 * compact.
 */
function CardFields({ fields, entity }: CardFieldsProps) {
  const [editingField, setEditingField] = useState<string | null>(null);
  const clearEditing = useCallback(() => setEditingField(null), []);

  return (
    <div className="flex-1 min-w-0 break-words space-y-0.5">
      {fields.map((field) => (
        <CardField
          key={field.name}
          field={field}
          entity={entity}
          editing={editingField === field.name}
          onEdit={() => setEditingField(field.name)}
          onDone={clearEditing}
          onCancel={clearEditing}
        />
      ))}
    </div>
  );
}

/** A single header field with its optional icon-tooltip and Field display. */
interface CardFieldProps {
  field: FieldDef;
  entity: Entity;
  editing: boolean;
  onEdit: () => void;
  onDone: () => void;
  onCancel: () => void;
}

function CardField({
  field,
  entity,
  editing,
  onEdit,
  onDone,
  onCancel,
}: CardFieldProps) {
  const Icon = fieldIcon(field);
  const fieldElement = (
    <Field
      fieldDef={field}
      entityType={entity.entity_type}
      entityId={entity.id}
      mode="compact"
      editing={editing}
      onEdit={onEdit}
      onDone={onDone}
      onCancel={onCancel}
    />
  );

  if (!Icon) return fieldElement;

  const tip = field.description || field.name.replace(/_/g, " ");
  return (
    <div className="flex items-start gap-1.5">
      <CardFieldIcon Icon={Icon} tip={tip} />
      <div className="flex-1 min-w-0">{fieldElement}</div>
    </div>
  );
}

/** Props for the tooltip-wrapped icon badge on a card field. */
interface CardFieldIconProps {
  Icon: LucideIcon;
  tip: string;
}

/** Tooltip-wrapped icon badge for a header field on the card. */
function CardFieldIcon({ Icon, tip }: CardFieldIconProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          aria-label={tip}
          className="mt-0.5 shrink-0 text-muted-foreground/50"
        >
          <Icon className="h-3 w-3" />
        </span>
      </TooltipTrigger>
      <TooltipContent side="left" align="start">
        {tip}
      </TooltipContent>
    </Tooltip>
  );
}

/**
 * Dispatches ui.inspect with an explicit target moniker.
 *
 * The target is passed directly so the backend uses ctx.target rather than
 * walking the scope chain (which comes from FocusedScopeContext and may
 * point to a previously-focused entity, not this card).
 */
function InspectButton({ moniker }: { moniker: string }) {
  const dispatch = useDispatchCommand("ui.inspect");
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label="Inspect"
          className="shrink-0 mt-0.5 p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            dispatch({ target: moniker }).catch(console.error);
          }}
        >
          <Info className="h-3.5 w-3.5" />
        </button>
      </TooltipTrigger>
      <TooltipContent>Inspect</TooltipContent>
    </Tooltip>
  );
}
