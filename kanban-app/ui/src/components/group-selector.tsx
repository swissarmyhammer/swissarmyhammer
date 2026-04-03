/**
 * Group-by field selector dropdown for perspective tabs.
 *
 * Renders a list of available fields from the entity schema. Selecting
 * a field dispatches `perspective.group` to set the group expression;
 * selecting "None" dispatches `perspective.clearGroup`.
 *
 * The field list is metadata-driven: it reads FieldDef[] from the schema
 * context rather than hardcoding field names.
 */

import { useCallback } from "react";
import { X } from "lucide-react";
import { backendDispatch } from "@/lib/command-scope";
import type { FieldDef } from "@/types/kanban";

interface GroupSelectorProps {
  /** Currently selected group field name, or undefined/empty for none. */
  group: string | undefined;
  /** Perspective ID to update. */
  perspectiveId: string;
  /** Available fields from the entity schema. */
  fields: FieldDef[];
  /** Called after a selection to close the popover. */
  onClose: () => void;
}

/**
 * Dropdown list of fields for setting the perspective group-by expression.
 *
 * - Clicking a field dispatches `perspective.group` with the field name.
 * - "None" option dispatches `perspective.clearGroup`.
 * - Active field is visually highlighted.
 */
export function GroupSelector({
  group,
  perspectiveId,
  fields,
  onClose,
}: GroupSelectorProps) {
  /** Set group to a field name. */
  const handleSelect = useCallback(
    (fieldName: string) => {
      backendDispatch({
        cmd: "perspective.group",
        args: { group: fieldName, perspective_id: perspectiveId },
      }).catch(console.error);
      onClose();
    },
    [perspectiveId, onClose],
  );

  /** Clear the group expression. */
  const handleClear = useCallback(() => {
    backendDispatch({
      cmd: "perspective.clearGroup",
      args: { perspective_id: perspectiveId },
    }).catch(console.error);
    onClose();
  }, [perspectiveId, onClose]);

  // Filter to groupable fields — exclude hidden fields
  const groupableFields = fields.filter((f) => f.section !== "hidden");

  return (
    <div className="w-48" data-testid="group-selector">
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-xs font-medium text-muted-foreground">
          Group By
        </span>
        {group && (
          <button
            onClick={handleClear}
            className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
            aria-label="Clear group"
          >
            <X className="h-3 w-3" />
            Clear
          </button>
        )}
      </div>
      <div className="flex flex-col gap-0.5">
        <button
          onClick={handleClear}
          className={`text-left text-xs px-2 py-1 rounded transition-colors ${
            !group
              ? "bg-accent text-accent-foreground"
              : "hover:bg-muted text-muted-foreground hover:text-foreground"
          }`}
          data-testid="group-none"
        >
          None
        </button>
        {groupableFields.map((field) => {
          const isActive = group === field.name;
          return (
            <button
              key={field.id}
              onClick={() => handleSelect(field.name)}
              className={`text-left text-xs px-2 py-1 rounded transition-colors ${
                isActive
                  ? "bg-accent text-accent-foreground"
                  : "hover:bg-muted text-muted-foreground hover:text-foreground"
              }`}
              data-testid={`group-field-${field.name}`}
            >
              {field.name}
            </button>
          );
        })}
      </div>
    </div>
  );
}
