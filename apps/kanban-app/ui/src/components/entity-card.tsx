import { forwardRef, memo, useCallback, useMemo, useState } from "react";
import { GripVertical, Info } from "lucide-react";
import { FocusScope } from "@/components/focus-scope";
import { Inspectable } from "@/components/inspectable";
import { Pressable } from "@/components/pressable";
import { asSegment } from "@/types/spatial";
import { Field } from "@/components/fields/field";
import { useSchema } from "@/lib/schema-context";
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

    // The card is a navigable container: its body holds `<Field>`
    // scopes that nest under the card's FQM via `FocusScopeContext`,
    // so drill-in / drill-out walk field → card → column → board.
    return (
      <Inspectable moniker={asSegment(entity.moniker)}>
        <FocusScope
          moniker={asSegment(entity.moniker)}
          commands={extraCommands}
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
            <InspectButton entityId={entity.id} moniker={entity.moniker} />
          </div>
        </FocusScope>
      </Inspectable>
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

/**
 * Drag handle button — stops click propagation and wires drag handle props.
 *
 * The drag handle is **mouse-only**. `@dnd-kit` is configured on the
 * board with `useSensor(PointerSensor, …)` and no `KeyboardSensor`
 * (see `board-view.tsx::useSensor(PointerSensor, …)`). It is
 * intentionally NOT wrapped in a `<FocusScope>` because there is
 * nothing the keyboard user could do once focus landed there: the
 * `<button>` has no `onClick` action of its own (the handler only
 * stops propagation so the click doesn't bubble to the card body's
 * inspect dispatch), and the drag handlers from `dragHandleProps`
 * respond exclusively to pointer events. Registering it as a leaf
 * scope would create a tab-stop trap with no keyboard activation
 * story — contrast with `InspectButton` below, which IS a leaf
 * because Space/Enter on a focused inspect button dispatches
 * `ui.inspect`.
 *
 * The `onClick={(e) => e.stopPropagation()}` is preserved: it
 * prevents click-bubble to the card body, which would otherwise
 * dispatch `ui.inspect` via the `<Inspectable>` wrapper.
 */
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
  // Render through `<Field withIcon />` so the icon renders *inside*
  // the field's `<FocusZone>` — matching the inspector path
  // (`entity-inspector.tsx`'s `FieldRow`). The unified `<Field>` already
  // implements value-dependent icon and tooltip overrides via
  // `resolveFieldIconAndTip` (see `fields/field.tsx`), so the card no
  // longer needs to duplicate that logic.
  //
  // `showFocus={true}` makes the field zone render a visible
  // `<FocusIndicator>` when its `SpatialKey` becomes the focused key
  // for the window. Without this, a click on a single-value field
  // inside the card (title, status, plain text fields) would fire
  // `spatial_focus` and flip `data-focused`, but no visible decoration
  // would appear — leaving the user without feedback that they had
  // selected the field. The card body itself owns a separate focus bar
  // at the zone level; the per-field bar sits at the inner leaf so the
  // user can tell which atom of the card carries focus.
  return (
    <Field
      fieldDef={field}
      entityType={entity.entity_type}
      entityId={entity.id}
      mode="compact"
      editing={editing}
      onEdit={onEdit}
      onDone={onDone}
      onCancel={onCancel}
      showFocus
      withIcon
    />
  );
}

/**
 * Dispatches ui.inspect with an explicit target moniker.
 *
 * The target is passed directly so the backend uses ctx.target rather than
 * walking the scope chain (which comes from FocusedScopeContext and may
 * point to a previously-focused entity, not this card).
 *
 * Migrates to `<Pressable asChild>` so the inspect leaf gains both
 * keyboard reachability (`<FocusScope>` provided by Pressable) AND the
 * scope-level CommandDefs that bind Enter (vim/cua) and Space (cua) to
 * the same dispatch as a pointer click. Pre-migration the leaf was
 * focusable but Enter did NOTHING — the kernel's drillIn echoes the
 * focused FQM for a leaf, `setFocus` is idempotent, the visible effect
 * was a no-op. Pressable's CommandDefs close that gap. The `entityId`
 * suffix keeps each card's inspect leaf at a distinct FQM under the
 * card.
 *
 * The inner `<button>`'s `onClick={(e) => e.stopPropagation()}` is
 * preserved: a click on (i) must NOT bubble to the enclosing card
 * `<FocusZone>`'s own onClick (which would fire `spatial_focus(cardFq)`
 * and steal focus to the card body). Radix Slot's `mergeProps` runs
 * the child's `onClick` first, then the slot's — so
 * `e.stopPropagation()` lands BEFORE Pressable's `handleClick`
 * triggers `onPress` (dispatch). Both behaviours hold in the correct
 * order.
 */
function InspectButton({
  entityId,
  moniker,
}: {
  entityId: string;
  moniker: string;
}) {
  const dispatch = useDispatchCommand("ui.inspect");
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Pressable
          asChild
          moniker={asSegment(`card.inspect:${entityId}`)}
          ariaLabel="Inspect"
          onPress={() => {
            dispatch({ target: moniker }).catch(console.error);
          }}
        >
          <button
            type="button"
            className="shrink-0 mt-0.5 p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
            onClick={(e) => e.stopPropagation()}
          >
            <Info className="h-3.5 w-3.5" />
          </button>
        </Pressable>
      </TooltipTrigger>
      <TooltipContent>Inspect</TooltipContent>
    </Tooltip>
  );
}
