import { forwardRef, memo, useCallback, useMemo, useState } from "react";
import { GripVertical, Info } from "lucide-react";
import { FocusScope } from "@/components/focus-scope";
import { FocusZone } from "@/components/focus-zone";
import { Inspectable } from "@/components/inspectable";
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

    // The card body registers as a `<FocusZone>` — a navigable
    // container in the spatial-nav graph. The card holds multiple
    // focusable atoms (drag handle, the `<Field>` rows with their
    // own zones and pill leaves, inspect button), so it is a zone
    // by the kernel's three-peer contract: scopes are leaves, zones
    // are containers. Each interactive non-Field atom inside the card
    // is wrapped in its own inner `<FocusScope>` leaf — mirroring the
    // navbar pattern (`<FocusZone moniker="ui:navbar">` → leaf scopes
    // for inspect / search). The card's inner Field zones nest under
    // this card zone via `FocusZoneContext`, so drill-in / drill-out
    // work field → card → column → board.
    //
    // Pre-card-`01KQJDYJ4SDKK2G8FTAQ348ZHG` history: the card body was
    // a `<FocusScope>` because of an earlier kernel cross-zone-nav
    // workaround. That shape silently degraded the spatial graph: the
    // card scope was a kernel "leaf" with no children (because Scopes
    // do not push `FocusZoneContext`), but the React tree composed
    // Field zones inside it whose `parent_zone` skipped to the column
    // — so the kernel saw fields as siblings of cards under the
    // column rather than as descendants of a card. The card-as-zone
    // shape restores the topology and surfaces drill-in / drill-out
    // memory (`last_focused`) at the right level.
    //
    // When the surrounding tree mounts the spatial-nav stack
    // (`<SpatialFocusProvider>` + `<FocusLayer>` — the production path
    // in `App.tsx`) the zone registers via `spatial_register_zone`;
    // outside that stack the zone falls back to a plain `<div>` so
    // isolated unit tests don't need to spin up the spatial providers.
    // Either way the card carries the entity-focus / command-scope /
    // context-menu wiring shared with every other entity surface.
    //
    // `showFocusBar` defaults to true on `<FocusZone>`, which renders
    // the visible focus indicator on the card itself when the user
    // focuses the card body. The inner `<FocusScope>` leaves and the
    // `<Field>` zones own their own indicators when those atoms are
    // the focused FQM — same pattern as the navbar.
    return (
      // The card wraps an entity (`task:` / `tag:` moniker), so a
      // double-click on the card body should open the inspector for
      // that entity. The `<Inspectable>` wrapper owns the
      // `useDispatchCommand("ui.inspect")` hook and its `onDoubleClick`
      // handler; the spatial primitive `<FocusZone>` stays pure-spatial.
      // UI-chrome zones (`ui:*` / `perspective_tab:`) are NOT wrapped
      // in `<Inspectable>`. The architectural guard
      // (`focus-architecture.guards.node.test.ts`, Guards B + C)
      // enforces both directions.
      <Inspectable moniker={asSegment(entity.moniker)}>
        <FocusZone
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
            <DragHandle
              entityId={entity.id}
              dragHandleProps={dragHandleProps}
            />
            <CardFields sections={cardSections} entity={entity} />
            <InspectButton entityId={entity.id} moniker={entity.moniker} />
          </div>
        </FocusZone>
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
 * Wrapped in a `<FocusScope moniker="card.drag-handle">` leaf so the
 * outer card `<FocusZone>` has a navigable atom for the drag-grip
 * affordance. Mirrors the navbar's `<FocusScope moniker="ui:navbar.search">`
 * pattern: a single button atom inside a parent zone is a leaf scope.
 *
 * `entityId` parameterises the segment so each card's drag handle is
 * a distinct FQM (composes through the card's
 * `FullyQualifiedMonikerContext`). Without per-entity disambiguation the
 * registry would see two cards' drag handles collide on the same segment
 * and trip the structural-mismatch check.
 */
function DragHandle({
  entityId,
  dragHandleProps,
}: {
  entityId: string;
  dragHandleProps?: Record<string, unknown>;
}) {
  return (
    <FocusScope moniker={asSegment(`card.drag-handle:${entityId}`)}>
      <button
        type="button"
        className="shrink-0 mt-0.5 p-0 text-muted-foreground/50 hover:text-muted-foreground cursor-grab active:cursor-grabbing touch-none"
        onClick={(e) => e.stopPropagation()}
        {...dragHandleProps}
      >
        <GripVertical className="h-4 w-4" />
      </button>
    </FocusScope>
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
  // `showFocusBar={true}` makes the field zone render a visible
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
      showFocusBar
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
 * Wrapped in a `<FocusScope moniker="card.inspect:{id}">` leaf so the
 * outer card `<FocusZone>` has a navigable atom for the inspector
 * affordance. Same pattern as the navbar's
 * `<FocusScope moniker="ui:navbar.inspect">`. The `entityId` suffix
 * keeps each card's inspect leaf at a distinct FQM under the card.
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
    <FocusScope moniker={asSegment(`card.inspect:${entityId}`)}>
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
    </FocusScope>
  );
}
