/**
 * Register progress display with the Field registry.
 */

import {
  registerDisplay,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { ProgressDisplay } from "@/components/fields/displays/progress-display";

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

registerDisplay("progress", ProgressDisplayAdapter);
