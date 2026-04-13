/**
 * Register progress display with the Field registry.
 *
 * Publishes {@link isProgressEmpty} as the registry's `isEmpty` predicate so
 * the inspector can suppress the surrounding `FieldRow` when there are no
 * subtasks to visualise.
 */

import {
  registerDisplay,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { ProgressDisplay } from "@/components/fields/displays/progress-display";
import { isProgressEmpty } from "@/components/fields/displays/progress-empty";

function ProgressDisplayAdapter({
  field,
  value,
  entity,
  mode,
}: FieldDisplayProps) {
  return (
    <ProgressDisplay field={field} value={value} entity={entity!} mode={mode} />
  );
}

registerDisplay("progress", ProgressDisplayAdapter, {
  isEmpty: isProgressEmpty,
});
