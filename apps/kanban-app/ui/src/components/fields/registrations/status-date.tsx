/**
 * Register status-date display with the Field registry.
 *
 * `status_date` is a computed field whose value is a tagged `{ kind, timestamp }`
 * payload — see builtin/definitions/status_date.yaml. The display chooses an
 * icon + phrasing per `kind`.
 *
 * Publishes {@link isStatusDateEmpty} as the registry's `isEmpty` predicate so
 * the inspector can suppress the surrounding `FieldRow` when the backend
 * derivation returns `null` (no completed / overdue / started / scheduled /
 * created anchor). Without this the row would render the `target` icon +
 * tooltip with an empty content slot — same regression pattern card 01KP23V1
 * fixed for the `progress` display.
 */

import {
  registerDisplay,
  type FieldDisplayProps,
} from "@/components/fields/field";
import {
  StatusDateDisplay,
  statusDateIconOverride,
  statusDateTooltipOverride,
} from "@/components/fields/displays/status-date-display";
import { isStatusDateEmpty } from "@/components/fields/displays/status-date-empty";

function StatusDateDisplayAdapter({
  field,
  value,
  entity,
  mode,
}: FieldDisplayProps) {
  return (
    <StatusDateDisplay
      field={field}
      value={value}
      entity={entity!}
      mode={mode}
    />
  );
}

registerDisplay("status-date", StatusDateDisplayAdapter, {
  isEmpty: isStatusDateEmpty,
  iconOverride: statusDateIconOverride,
  tooltipOverride: statusDateTooltipOverride,
});
