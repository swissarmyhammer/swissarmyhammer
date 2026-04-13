import { forwardRef, memo, useCallback, useMemo, useState } from "react";
import { GripVertical, Info } from "lucide-react";
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
import {
  useEntitySections,
  type ResolvedSection,
} from "@/hooks/use-entity-sections";

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
 * Renders fields grouped by the entity's declared `on_card` sections.
 * The first `on_card` section (conventionally `header`) renders inline
 * at the top of the card; any additional `on_card` sections render
 * below, separated by a thin divider. Cards never render section
 * labels — labels belong to the inspector; cards stay dense.
 *
 * When an entity omits `sections` (e.g. tag, actor), only the
 * implicit header section renders — preserving the pre-declarative
 * card layout.
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
    const cardSections = useCardSections(entity.entity_type);
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
          <CardFields sections={cardSections} entity={entity} />
          <InspectButton moniker={entity.moniker} />
        </div>
      </FocusScope>
    );
  }),
);

/**
 * Resolve the ordered list of sections that appear on the card, memoised
 * by schema identity.
 *
 * The `header` section is implicitly `on_card`-eligible for backcompat
 * with entities that omit `sections` entirely; for entities that declare
 * sections, only explicit `on_card: true` sections appear. Each returned
 * `ResolvedSection` carries the full ordered field list for that section,
 * with `section: "hidden"` fields already filtered out.
 */
function useCardSections(entityType: string): ResolvedSection[] {
  const { getSchema } = useSchema();
  const schema = getSchema(entityType);
  const entitySections = schema?.entity.sections;
  const fields = schema?.fields ?? [];
  const resolved = useEntitySections(entitySections, fields);
  return useMemo(() => {
    // When the entity omits `sections`, default to showing only the
    // implicit `header` section on the card (legacy behaviour).
    if (!entitySections || entitySections.length === 0) {
      return resolved.filter((s) => s.def.id === "header");
    }
    return resolved.filter((s) => s.def.on_card === true);
  }, [resolved, entitySections]);
}

/** Drag handle button — stops click propagation and wires drag handle props. */
function DragHandle({
  dragHandleProps,
}: {
  dragHandleProps?: Record<string, unknown>;
}) {
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

/**
 * Renders the card's `on_card` sections with icon tooltips and per-field
 * editing state. Extracted so that EntityCard itself stays compact.
 *
 * The first section renders inline; each subsequent section is preceded
 * by a thin horizontal divider so additional sections (e.g. `dates`)
 * visually separate from the header without introducing labels.
 */
function CardFields({
  sections,
  entity,
}: {
  sections: ResolvedSection[];
  entity: Entity;
}) {
  const [editingField, setEditingField] = useState<string | null>(null);
  const clearEditing = useCallback(() => setEditingField(null), []);

  /** Track whether we've already rendered a non-empty section so we know when to draw dividers. */
  let renderedAny = false;
  return (
    <div className="flex-1 min-w-0 break-words">
      {sections.map((section) => {
        if (section.fields.length === 0) return null;
        const showDivider = renderedAny;
        renderedAny = true;
        return (
          <div
            key={section.def.id}
            data-testid={`card-section-${section.def.id}`}
          >
            {showDivider && <div className="my-1.5 h-px bg-border/50" />}
            <div className="space-y-0.5">
              {section.fields.map((field) => (
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
          </div>
        );
      })}
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
  const hasIcon = !!fieldIcon(field);
  return (
    <div className={hasIcon ? "flex items-start gap-1.5" : ""}>
      <CardFieldIcon field={field} />
      <div className="flex-1 min-w-0">
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
      </div>
    </div>
  );
}

/** Tooltip-wrapped icon badge for a header field on the card. */
function CardFieldIcon({ field }: { field: FieldDef }) {
  const Icon = fieldIcon(field);
  if (!Icon) return null;
  const tip = field.description || field.name.replace(/_/g, " ");
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
