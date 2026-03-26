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
  value,
  onCommit,
  onCancel,
}: FieldEditorProps) {
  return (
    <ColorPaletteEditor
      value={value}
      onCommit={onCommit}
      onCancel={onCancel}
      mode="compact"
    />
  );
}

function ColorSwatchDisplayAdapter({ value }: FieldDisplayProps) {
  return <ColorSwatchDisplay value={value} />;
}

registerEditor("color-palette", ColorPaletteEditorAdapter);
registerDisplay("color-swatch", ColorSwatchDisplayAdapter);
