/**
 * Register progress-ring display with the Field registry.
 *
 * Publishes {@link isProgressEmpty} as the registry's `isEmpty` predicate so
 * the inspector can suppress the surrounding `FieldRow` when a board / column
 * has no tasks to summarise.
 */

import {
  registerDisplay,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { ProgressRingDisplay } from "@/components/fields/displays/progress-ring-display";
import { isProgressEmpty } from "@/components/fields/displays/progress-empty";

function ProgressRingDisplayAdapter({
  field,
  value,
  entity,
  mode,
}: FieldDisplayProps) {
  return (
    <ProgressRingDisplay
      field={field}
      value={value}
      entity={entity!}
      mode={mode}
    />
  );
}

registerDisplay("progress-ring", ProgressRingDisplayAdapter, {
  isEmpty: isProgressEmpty,
});
