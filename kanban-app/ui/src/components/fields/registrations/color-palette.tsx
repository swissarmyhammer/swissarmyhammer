/**
 * Register color-palette editor and color-swatch display with the Field registry.
 */

import {
  registerEditor,
  registerDisplay,
  type FieldEditorProps,
  type FieldDisplayProps,
} from "@/components/fields/field";
import { ColorPaletteEditor } from "@/components/fields/editors/color-palette-editor";
import { ColorSwatchDisplay } from "@/components/fields/displays/color-swatch-display";

function ColorPaletteEditorAdapter({
  field,
  value,
  entity,
  onCommit,
  onCancel,
  onChange,
}: FieldEditorProps) {
  return (
    <ColorPaletteEditor
      field={field}
      value={value}
      entity={entity}
      onCommit={onCommit}
      onCancel={onCancel}
      onChange={onChange}
      mode="compact"
    />
  );
}

function ColorSwatchDisplayAdapter({ value, mode }: FieldDisplayProps) {
  return <ColorSwatchDisplay value={value} mode={mode} />;
}

registerEditor("color-palette", ColorPaletteEditorAdapter);
registerDisplay("color-swatch", ColorSwatchDisplayAdapter);
