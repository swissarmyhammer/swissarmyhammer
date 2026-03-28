/**
 * Register progress-ring display with the Field registry.
 */

import {
  registerDisplay,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { ProgressRingDisplay } from "@/components/fields/displays/progress-ring-display";

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

registerDisplay("progress-ring", ProgressRingDisplayAdapter);
