import {
  forwardRef,
  memo,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { GripVertical, Info, icons } from "lucide-react";
import { FocusScope } from "@/components/focus-scope";
import { Field } from "@/components/fields/field";
import { useSchema } from "@/lib/schema-context";
import { useEntityCommands } from "@/lib/entity-commands";
import { moniker } from "@/lib/moniker";
import {
  CommandScopeContext,
  resolveCommand,
  dispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import type { ClaimPredicate } from "@/lib/entity-focus-context";
import type { Entity, FieldDef } from "@/types/kanban";

/** Convert kebab-case icon name to PascalCase key for lucide-react lookup. */
function kebabToPascal(s: string): string {
  return s.replace(/(^|-)([a-z])/g, (_, _dash, c) => c.toUpperCase());
}

/** Resolve a lucide icon component from a field's `icon` property. */
function resolveIcon(field: FieldDef) {
  if (!field.icon) return null;
  const key = kebabToPascal(field.icon);
  return (
    (icons[key as keyof typeof icons] as React.ComponentType<{
      className?: string;
    }>) ?? null
  );
}

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
  forwardRef<HTMLDivElement, EntityCardProps>(function EntityCard(
    {
      entity,
      dragHandleProps,
      style,
      draggable,
      onDragStart,
      onDragEnd,
      extraCommands,
      claimWhen,
      ...rest
    },
    ref,
  ) {
    const { getSchema } = useSchema();
    const schema = getSchema(entity.entity_type);

    const entityMoniker = moniker(entity.entity_type, entity.id);

    const cardFields = useMemo(
      () => (schema?.fields ?? []).filter((f) => f.section === "header"),
      [schema],
    );

    const commands = useEntityCommands(
      entity.entity_type,
      entity.id,
      entity,
      extraCommands,
    );
    const [editingField, setEditingField] = useState<string | null>(null);

    const clearEditing = useCallback(() => setEditingField(null), []);

    return (
      <FocusScope
        moniker={entityMoniker}
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
          <button
            type="button"
            className="shrink-0 mt-0.5 p-0 text-muted-foreground/50 hover:text-muted-foreground cursor-grab active:cursor-grabbing touch-none"
            onClick={(e) => e.stopPropagation()}
            {...dragHandleProps}
          >
            <GripVertical className="h-4 w-4" />
          </button>
          <div className="flex-1 min-w-0 break-words space-y-0.5">
            {cardFields.map((field) => {
              const Icon = resolveIcon(field);
              return (
                <div
                  key={field.name}
                  className={Icon ? "flex items-start gap-1.5" : ""}
                >
                  {Icon && (
                    <span className="mt-0.5 shrink-0 text-muted-foreground/50">
                      <Icon className="h-3 w-3" />
                    </span>
                  )}
                  <div className="flex-1 min-w-0">
                    <Field
                      fieldDef={field}
                      entityType={entity.entity_type}
                      entityId={entity.id}
                      mode="compact"
                      editing={editingField === field.name}
                      onEdit={() => setEditingField(field.name)}
                      onDone={clearEditing}
                      onCancel={clearEditing}
                    />
                  </div>
                </div>
              );
            })}
          </div>
          <InspectButton />
        </div>
      </FocusScope>
    );
  }),
);

/** Dispatches entity.inspect through the scope chain instead of calling inspectEntity directly. */
function InspectButton() {
  const scope = useContext(CommandScopeContext);
  return (
    <button
      type="button"
      className="shrink-0 mt-0.5 p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
      onClick={(e) => {
        e.stopPropagation();
        const cmd = resolveCommand(scope, "ui.inspect");
        if (cmd) dispatchCommand(cmd);
      }}
      title="Inspect"
    >
      <Info className="h-3.5 w-3.5" />
    </button>
  );
}
