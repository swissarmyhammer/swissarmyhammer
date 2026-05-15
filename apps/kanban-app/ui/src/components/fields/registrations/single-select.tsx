/**
 * Register the CM6-based single-select editor with the Field registry.
 *
 * Registration key `"single-select"` mirrors the `"multi-select"` convention.
 * Current shipping field definitions (`project`, `position_column`) declare
 * `editor: select` and route through `SelectEditorAdapter` in `./select.tsx`,
 * which delegates reference fields here. This registration makes
 * `"single-select"` available as a direct editor key for future YAML defs
 * that want to bypass the adapter indirection.
 */

import {
  registerEditor,
  type FieldEditorProps,
} from "@/components/fields/field";
import { SingleSelectEditor } from "@/components/fields/editors/single-select-editor";

function SingleSelectEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
  mode,
}: FieldEditorProps) {
  return (
    <SingleSelectEditor
      field={field}
      value={value}
      entity={entity}
      onCommit={onCommit}
      onCancel={onCancel}
      onChange={onChange}
      mode={mode}
    />
  );
}

registerEditor("single-select", SingleSelectEditorAdapter);
