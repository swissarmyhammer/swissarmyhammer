import { forwardRef, memo, useCallback, useMemo, useState } from "react";
import { GripVertical, Info, type LucideIcon } from "lucide-react";
import { FocusScope } from "@/components/focus-scope";
import {
  Field,
  getDisplayIconOverride,
  getDisplayTooltipOverride,
} from "@/components/fields/field";
import { fieldIcon } from "@/components/fields/field-icon";
import { useSchema } from "@/lib/schema-context";
import { useEntityCommands } from "@/lib/entity-commands";
import { useDispatchCommand, type CommandDef } from "@/lib/command-scope";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
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
}

/**
 * Compose the card's `commands` array: the per-card Enter-to-inspect
 * binding first, followed by any caller-supplied `extraCommands`. Callers
 * can override Enter by providing their own `ui.inspect` entry in
 * `extraCommands`.
 */
function useCardCommands(
  entity: EntityCardProps["entity"],
  extraCommands: CommandDef[] | undefined,
) {
  const enterCommand = useEnterInspectCommand(entity.moniker);
  const mergedExtra = useMemo(
    () => (extraCommands ? [...enterCommand, ...extraCommands] : enterCommand),
    [enterCommand, extraCommands],
  );
  return useEntityCommands(entity.entity_type, entity.id, entity, mergedExtra);
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
      ...rest
    } = props;
    const cardSections = useCardSections(entity.entity_type);
    const commands = useCardCommands(entity, extraCommands);

    return (
      <FocusScope
        moniker={entity.moniker}
        commands={commands}
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
  // Resolve the icon: prefer a value-dependent override from the display,
  // then fall back to the static YAML icon resolved by fieldIcon().
  const overrideFn = getDisplayIconOverride(field.display ?? "");
  const overrideResult = overrideFn
    ? overrideFn(entity.fields[field.name])
    : null;
  const resolvedIcon = overrideResult ?? fieldIcon(field);

  // Resolve the tooltip: prefer a value-dependent override from the display,
  // then fall back to the static YAML description or humanised field name.
  const tooltipOverrideFn = getDisplayTooltipOverride(field.display ?? "");
  const tooltipOverrideResult = tooltipOverrideFn
    ? tooltipOverrideFn(entity.fields[field.name])
    : null;

  const hasIcon = !!resolvedIcon;
  return (
    <div className={hasIcon ? "flex items-start gap-1.5" : ""}>
      <CardFieldIcon
        field={field}
        icon={resolvedIcon}
        tooltipOverride={tooltipOverrideResult}
      />
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

/**
 * Tooltip-wrapped icon badge for a field on the card.
 *
 * Accepts a pre-resolved `icon` from the parent — this may be a
 * value-dependent override from the display's `iconOverride` registration,
 * or the static icon from the field's YAML definition. If null, nothing
 * renders.
 *
 * When a `tooltipOverride` string is provided it replaces the static YAML
 * description in the tooltip so the card shows dynamic, value-dependent text
 * (e.g. "Completed 3 days ago").
 */
function CardFieldIcon({
  field,
  icon: Icon,
  tooltipOverride,
}: {
  field: FieldDef;
  icon: LucideIcon | null;
  tooltipOverride?: string | null;
}) {
  if (!Icon) return null;
  const staticTip = field.description || field.name.replace(/_/g, " ");
  const tip = tooltipOverride ?? staticTip;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          aria-label={tip}
          className="h-4 inline-flex items-center shrink-0 text-muted-foreground/50"
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
 * Build a per-card `Enter`-to-inspect command.
 *
 * Mirrors the `RowSelector` precedent (`data-table.tsx`) and the per-button
 * `view.activate.<id>` command on `ViewButton` (`left-nav.tsx`): pressing
 * Enter while the card is focused dispatches `ui.inspect` with an explicit
 * `target` equal to this card's entity moniker.
 *
 * The target is passed directly so the backend uses `ctx.target` rather than
 * walking the scope chain — matching the `InspectButton` click-path semantics
 * so keyboard and mouse activation converge on the exact same dispatch shape.
 *
 * No competing Enter binding exists on card scopes, so this does not shadow
 * anything; it simply fills the "focus a card, press Enter" gap the task
 * description identifies.
 */
function useEnterInspectCommand(moniker: string): CommandDef[] {
  const dispatchInspect = useDispatchCommand("ui.inspect");
  return useMemo<CommandDef[]>(
    () => [
      {
        // Namespace the command id per card so the schema-derived
        // `ui.inspect` entry (which has the same `Inspect` label the context
        // menu surfaces) is not shadowed inside the card's scope Map, and so
        // sibling cards' Enter commands don't collide with each other
        // through the scope chain. Matches the `view.activate.<id>` pattern
        // on `ViewButton` (`left-nav.tsx`) and the Enter binding semantics
        // the task description specifies for per-card activation.
        id: `entity.activate.${moniker}`,
        name: "Inspect",
        keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
        execute: () => {
          dispatchInspect({ target: moniker }).catch(console.error);
        },
        contextMenu: false,
      },
    ],
    [dispatchInspect, moniker],
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
