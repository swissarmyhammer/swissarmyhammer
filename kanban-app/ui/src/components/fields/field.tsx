/**
 * Field — metadata-driven, data-bound control for any entity field.
 *
 * Subscribes to field-level changes in the entity store. Re-renders only when
 * its specific field value changes — not when other fields on the same entity
 * change. Works regardless of change source (local edit, other window, file
 * watcher, etc.).
 *
 * Resolves editor and display components from registries — no switch statements,
 * no hardcoded field types. Adding a new field type means registering a component,
 * not touching this file.
 *
 * # Spatial-nav participation
 *
 * `<Field>` is a `<FocusZone>` whose moniker is `field:{type}:{id}.{name}`.
 * Display-mode children render as leaves (`<FocusScope>` per pill in
 * multi-value displays; the zone itself in single-value displays). Edit
 * mode replaces the children with the editor element, which takes DOM
 * focus directly — the editor is NOT a `<FocusScope>`, so spatial nav
 * stays out of the way during editing.
 *
 * The zone defaults to `showFocusBar={false}`. The default exists for
 * grid-cell consumers — they wrap each `<Field>` in their own
 * `<FocusScope>` that already renders a cursor ring around the cell, so
 * a second indicator at the field zone would be redundant. Every other
 * consumer opts in by passing `showFocusBar={true}` when they want the
 * inner field zone to advertise focus:
 *   - The inspector row (`EntityInspector` → `FieldRow` → `<Field
 *     showFocusBar />`). Inspector rows fill the panel width and have
 *     no enclosing focus chrome, so the per-row bar is the user's only
 *     focus cue at the row level.
 *   - The card body (`EntityCard` → `CardField` → `<Field
 *     showFocusBar />`). Card fields render inside a card-zone bar,
 *     but the per-field bar is still the user's only cue for which
 *     atom of the card carries focus (title vs. status vs. tags)
 *     — the card-zone bar fires on the card itself, not on its
 *     descendants. Badge-list pill leaves inside these card fields
 *     advertise their own focus through `MentionView`'s `<FocusScope>`
 *     default of `showFocusBar={true}`.
 *   - The nav-bar's `<Field>` per-pill children, when their parent
 *     does not already mount a focus indicator.
 *
 * The zone defaults to `handleEvents={true}`. The grid-cell case passes
 * `handleEvents={false}` so the surrounding `grid_cell:R:K`
 * `<FocusScope>` keeps owning click → cursor-ring updates. See the
 * "Decision: Option A" note in card `01KQ5QB6F4MTD35GBTARJH4JEW`.
 */

import { useCallback, useMemo, type ComponentType, type ReactNode } from "react";
import { useEntityStore, useFieldValue } from "@/lib/entity-store-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useDebouncedSave } from "@/lib/use-debounced-save";
import { resolveEditor } from "@/components/fields/editors";
import type { EditorProps } from "@/components/fields/editors";
import { FocusZone } from "@/components/focus-zone";
import { Inspectable } from "@/components/inspectable";
import { EMPTY_COMMANDS, type CommandDef } from "@/lib/command-scope";
import { fieldMoniker } from "@/lib/moniker";
import { asMoniker } from "@/types/spatial";
import { fieldIcon } from "@/components/fields/field-icon";
import { FieldIconBadge } from "@/components/fields/field-icon-badge";
import { useOptionalFocusActions } from "@/lib/entity-focus-context";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { HelpCircle, type LucideIcon } from "lucide-react";
import type { FieldDef, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Contracts — every editor and display implements one of these
// ---------------------------------------------------------------------------

/**
 * Props that every editor component receives from Field.
 *
 * Callback signatures (onCommit, onCancel, onChange) are picked from
 * EditorProps so the two interfaces stay in sync automatically.
 */
export interface FieldEditorProps extends Pick<
  EditorProps,
  "value" | "mode" | "onCommit" | "onCancel" | "onChange"
> {
  field: FieldDef;
  entity?: Entity;
}

/** Props that every display component receives from Field. */
export interface FieldDisplayProps {
  field: FieldDef;
  value: unknown;
  entity?: Entity;
  mode: "compact" | "full";
  /** Persist a value without exiting display mode (e.g. checkbox toggle). */
  onCommit?: (value: unknown) => void;
}

// ---------------------------------------------------------------------------
// Registries — editors and displays register themselves here
// ---------------------------------------------------------------------------

/**
 * Optional metadata a display registration may expose.
 *
 * - `isEmpty`: predicate the inspector uses to suppress the surrounding row
 *   (icon, tooltip, flex gap) when the underlying display would render
 *   nothing. Consulted only for non-editable fields so editable fields with
 *   empty values stay clickable. Displays own their own notion of emptiness
 *   so the inspector stays free of hardcoded field names.
 *
 * - `iconOverride`: value-dependent icon function the parent layout (inspector
 *   row, card field) calls to replace the static YAML field icon. When the
 *   function returns a LucideIcon it replaces the static icon; when it
 *   returns null the static icon is used as fallback. This is general-purpose
 *   — any display can register one.
 *
 * - `tooltipOverride`: value-dependent tooltip text function the parent layout
 *   calls to replace the static YAML field description in the icon tooltip.
 *   When the function returns a non-null string it replaces the static text;
 *   when it returns null the static text is used as fallback. This is
 *   general-purpose — any display can register one.
 */
export interface DisplayRegistration {
  component: ComponentType<FieldDisplayProps>;
  isEmpty?: (value: unknown) => boolean;
  iconOverride?: (value: unknown) => LucideIcon | null;
  tooltipOverride?: (value: unknown) => string | null;
}

/** Options accepted by {@link registerDisplay}. */
export interface RegisterDisplayOptions {
  /** See {@link DisplayRegistration.isEmpty}. */
  isEmpty?: (value: unknown) => boolean;
  /** See {@link DisplayRegistration.iconOverride}. */
  iconOverride?: (value: unknown) => LucideIcon | null;
  /** See {@link DisplayRegistration.tooltipOverride}. */
  tooltipOverride?: (value: unknown) => string | null;
}

const editorRegistry = new Map<string, ComponentType<FieldEditorProps>>();
const displayRegistry = new Map<string, DisplayRegistration>();

/** Register an editor component for a given editor type name. */
export function registerEditor(
  name: string,
  component: ComponentType<FieldEditorProps>,
) {
  editorRegistry.set(name, component);
}

/**
 * Register a display component for a given display type name.
 *
 * @param name - Display type identifier (matches `display:` in field YAML).
 * @param component - React component rendered for values of this display.
 * @param options - Optional metadata (see {@link RegisterDisplayOptions}).
 */
export function registerDisplay(
  name: string,
  component: ComponentType<FieldDisplayProps>,
  options?: RegisterDisplayOptions,
) {
  displayRegistry.set(name, {
    component,
    isEmpty: options?.isEmpty,
    iconOverride: options?.iconOverride,
    tooltipOverride: options?.tooltipOverride,
  });
}

/**
 * Look up the `isEmpty` predicate registered for a display, if any.
 *
 * Returns `undefined` when the display is unregistered or when the
 * registration did not supply an `isEmpty` option.
 */
export function getDisplayIsEmpty(
  name: string,
): ((value: unknown) => boolean) | undefined {
  return displayRegistry.get(name)?.isEmpty;
}

/**
 * Look up the `iconOverride` function registered for a display, if any.
 *
 * Returns `undefined` when the display is unregistered or when the
 * registration did not supply an `iconOverride` option.
 */
export function getDisplayIconOverride(
  name: string,
): ((value: unknown) => LucideIcon | null) | undefined {
  return displayRegistry.get(name)?.iconOverride;
}

/**
 * Look up the `tooltipOverride` function registered for a display, if any.
 *
 * Returns `undefined` when the display is unregistered or when the
 * registration did not supply a `tooltipOverride` option.
 */
export function getDisplayTooltipOverride(
  name: string,
): ((value: unknown) => string | null) | undefined {
  return displayRegistry.get(name)?.tooltipOverride;
}

// ---------------------------------------------------------------------------
// Field component
// ---------------------------------------------------------------------------

export interface FieldProps {
  /** YAML field metadata — type, editor, display, section, etc. */
  fieldDef: FieldDef;
  /** Entity type (e.g. "task", "tag"). */
  entityType: string;
  /** Entity ID. */
  entityId: string;
  /** Presentation mode — compact (grid cells) or full (inspector rows). */
  mode: "compact" | "full";
  /** Whether the field is in edit mode. Container controls this. */
  editing: boolean;
  /** User wants to enter edit mode (click, Enter). */
  onEdit?: () => void;
  /** Editing finished — field already saved. Close the editor. */
  onDone?: () => void;
  /** Editing cancelled — discard and close. */
  onCancel?: () => void;
  /**
   * When false, the inner `<FocusZone>` skips click / right-click /
   * double-click ownership. Use when an enclosing primitive (e.g. a
   * grid-cell `<FocusScope>`) already owns the click semantics for this
   * region. Defaults to true.
   */
  handleEvents?: boolean;
  /**
   * When true, the inner `<FocusZone>` shows its own visible focus bar.
   * Defaults to false so grid-cell consumers — which already wrap each
   * field in a `<FocusScope>` cursor-ring — don't double up on
   * indicators. Consumers without an enclosing focus chrome opt in by
   * passing `<Field showFocusBar />`: inspector rows (the row fills the
   * panel and the per-row bar is the user's only cue), card fields
   * (the card-zone bar fires on the card itself, so the per-field bar
   * tells the user which atom of the card carries focus). See the file
   * header for the full taxonomy of consumers and why each one opts in
   * or out.
   */
  showFocusBar?: boolean;
  /**
   * When true, render a tooltip-wrapped lucide icon as the leftmost
   * child *inside* the `<FocusZone>`. The icon resolves from
   * `fieldDef.icon` (kebab-case lucide name) with fallback to
   * `HelpCircle` when the name doesn't map to a known component, then
   * gets replaced by the display registry's `iconOverride(value)` when
   * one is registered. The tooltip text comes from
   * `tooltipOverride(value)` or `field.description`, falling back to a
   * humanised field name.
   *
   * Folding the icon *inside* the field zone (instead of leaving it as
   * a sibling at the inspector's row level) is the single seam through
   * which:
   *   1. Clicking the icon dispatches `spatial_focus` for the field
   *      zone — the icon is now part of the zone's click target.
   *   2. The visible `<FocusIndicator>` (`-left-2` from the zone's
   *      left edge) paints to the LEFT of the icon, not between the
   *      icon and the content.
   *   3. Every existing `<Field>` callsite that doesn't opt in via
   *      `withIcon={true}` continues to render exactly as before —
   *      backwards-compatible by default.
   *
   * Defaults to false. The inspector row (`EntityInspector` →
   * `FieldRow`) is currently the only consumer that opts in.
   */
  withIcon?: boolean;
}

/**
 * Collects the commit / cancel / debounced-change callbacks a Field needs.
 *
 * Split out of {@link Field} so the component stays readable. All three
 * callbacks close over the same entity/field identity and share the debounced
 * save cancel, which is why they live together.
 */
function useFieldHandlers(
  entityType: string,
  entityId: string,
  fieldName: string,
  onDone?: () => void,
  onCancel?: () => void,
) {
  const { updateField } = useFieldUpdate();
  const { onChange: debouncedOnChange, cancel: cancelSave } = useDebouncedSave({
    updateField,
    entityType,
    entityId,
    fieldName,
  });

  const handleCommit = useCallback(
    (newValue: unknown) => {
      cancelSave();
      updateField(entityType, entityId, fieldName, newValue).catch(() => {});
      onDone?.();
    },
    [cancelSave, updateField, entityType, entityId, fieldName, onDone],
  );

  const handleDisplayCommit = useCallback(
    (newValue: unknown) => {
      updateField(entityType, entityId, fieldName, newValue).catch(() => {});
    },
    [updateField, entityType, entityId, fieldName],
  );

  const handleCancel = useCallback(() => {
    cancelSave();
    onCancel?.();
  }, [cancelSave, onCancel]);

  return { handleCommit, handleDisplayCommit, handleCancel, debouncedOnChange };
}

/**
 * Resolve the icon and tooltip a `<Field withIcon />` should render for the
 * current value.
 *
 * Mirrors the logic the inspector's `FieldRow` used before this lived
 * inside `<Field>`:
 *
 *   - **Icon priority**: display-registry `iconOverride(value)` →
 *     static `fieldIcon(field)` → `HelpCircle` (when `field.icon` is set
 *     but doesn't resolve to a known lucide component) → `null` (when
 *     `field.icon` is missing entirely).
 *   - **Tooltip priority**: display-registry `tooltipOverride(value)` →
 *     `field.description` → humanised `field.name` (underscores become
 *     spaces).
 *
 * Pure function of `(field, value)`; no React state. Callers feed in the
 * value they already subscribe to via `useFieldValue` so the resolution
 * re-runs whenever the value changes (the icon or tooltip can both be
 * value-dependent).
 */
function resolveFieldIconAndTip(
  field: FieldDef,
  value: unknown,
): { Icon: LucideIcon | null; tip: string } {
  const staticIcon = field.icon ? (fieldIcon(field) ?? HelpCircle) : null;
  const overrideFn = getDisplayIconOverride(field.display ?? "");
  const overrideResult = overrideFn ? overrideFn(value) : null;
  const Icon = overrideResult ?? staticIcon;

  const staticTip = field.description || field.name.replace(/_/g, " ");
  const tooltipOverrideFn = getDisplayTooltipOverride(field.display ?? "");
  const overrideTip = tooltipOverrideFn ? tooltipOverrideFn(value) : null;
  const tip = overrideTip ?? staticTip;

  return { Icon, tip };
}

/** Resolves the editor from the registry and renders it, or null if unregistered. */
function FieldEditor(props: {
  fieldDef: FieldDef;
  value: unknown;
  entity: Entity | undefined;
  mode: "compact" | "full";
  onCommit: (value: unknown) => void;
  onCancel: () => void;
  onChange: (value: unknown) => void;
}) {
  const Editor = editorRegistry.get(props.fieldDef.editor ?? "");
  if (!Editor) return null;
  return (
    <Editor
      field={props.fieldDef}
      value={props.value}
      entity={props.entity}
      mode={props.mode}
      onCommit={props.onCommit}
      onCancel={props.onCancel}
      onChange={props.onChange}
    />
  );
}

/**
 * Resolves the display from the registry and renders it. Wraps in a
 * click-to-edit surface when the field is editable; bare otherwise.
 */
function FieldDisplayContent(props: {
  fieldDef: FieldDef;
  value: unknown;
  entity: Entity | undefined;
  mode: "compact" | "full";
  onEdit?: () => void;
  onCommit: (value: unknown) => void;
}) {
  const Display = displayRegistry.get(
    props.fieldDef.display ?? "text",
  )?.component;
  if (!Display) return null;
  const inner = (
    <Display
      field={props.fieldDef}
      value={props.value}
      entity={props.entity}
      mode={props.mode}
      onCommit={props.onCommit}
    />
  );
  if (resolveEditor(props.fieldDef) === "none") return inner;
  return (
    <div className="text-sm cursor-text min-h-[1.25rem]" onClick={props.onEdit}>
      {inner}
    </div>
  );
}

/**
 * Data-bound field control.
 *
 * Subscribes to its specific field value via useFieldValue — re-renders
 * only when this field changes. Resolves editor/display from registries.
 *
 * Wraps display- AND edit-mode output in a `<FocusZone>` keyed by
 * `field:{entityType}:{entityId}.{fieldName}` so every consumer of
 * `<Field>` participates in spatial nav uniformly. In edit mode the
 * editor element takes DOM focus directly via its own ref-driven
 * `.focus()` call; the surrounding zone marks the moniker without
 * interfering, because its click handler short-circuits on
 * `INPUT/TEXTAREA/SELECT` and `[contenteditable]` targets — spatial
 * focus stays at the field-zone moniker while the user types. Exiting
 * edit mode (Escape, blur) returns to the same moniker without losing
 * the zone's identity in the DOM.
 *
 * # Enter ownership
 *
 * When the field is in display mode AND has an `onEdit` callback, the
 * field zone registers a scope-level `field.edit` command keyed to
 * Enter (vim + cua). The field zone's `<CommandScope>` sits closer
 * than the global root scope, so `extractScopeBindings` claims Enter
 * for `field.edit` whenever this field is the spatial focus —
 * shadowing the global `nav.drillIn: Enter` only for editable field
 * zones, leaving the drill-in default in place for every other
 * focusable. In edit mode the command is NOT registered (the editor
 * element owns Enter via its own keymap); for non-editable fields the
 * command is also not registered (no `onEdit`), so Enter is a no-op.
 */
export function Field({
  fieldDef,
  entityType,
  entityId,
  mode,
  editing,
  onEdit,
  onDone,
  onCancel,
  handleEvents = true,
  showFocusBar = false,
  withIcon = false,
}: FieldProps) {
  const value = useFieldValue(entityType, entityId, fieldDef.name);
  const entity = useEntityStore().getEntity(entityType, entityId);
  const { handleCommit, handleDisplayCommit, handleCancel, debouncedOnChange } =
    useFieldHandlers(entityType, entityId, fieldDef.name, onDone, onCancel);

  // Spatial + entity focus actions feed the `field.edit` execute closure:
  // we read the focused field-zone key from the spatial provider, then
  // dispatch `setFocus` against the entity-focus store on a successful
  // drill-in. Both providers may be absent in lightweight tests, so we
  // use the optional variants — the closure short-circuits when either
  // is missing, falling through to `onEdit?.()` (the legacy behaviour).
  const focusActions = useOptionalFocusActions();
  const spatialActions = useOptionalSpatialFocusActions();

  // Per-zone Enter binding: when the field is in display mode, register
  // a scope-level `field.edit` command keyed to Enter. The field zone's
  // `<CommandScope>` sits closer than the global root scope, so
  // `extractScopeBindings` claims Enter for `field.edit` whenever this
  // field is the spatial focus — shadowing the global `nav.drillIn:
  // Enter` only for focused field zones.
  //
  // The execute closure unifies "drill into pills" and "open editor":
  // it asks the kernel to drill into the field zone first; on a non-null
  // moniker (the field has spatial children — pills, badges, …) it
  // dispatches `setFocus(moniker)` and returns. Only when the kernel
  // returns null does it fall through to `onEdit?.()`. This means a
  // field with pills navigates to the first pill, while a field without
  // pills (and an `onEdit` callback) opens its editor — same scope-level
  // command, two outcomes driven by the registry's structural answer.
  // For non-editable fields with no spatial children, the kernel returns
  // null and `onEdit` is undefined → Enter is a no-op.
  //
  // The command is intentionally NOT registered in edit mode: the
  // editor element holds DOM focus and owns Enter via its own keymap
  // (commit on submit, newline in multiline, etc.). The global keymap
  // handler's `isEditableTarget` gate also short-circuits before any
  // scope binding resolution when the focused element is an
  // `<input>` / `<textarea>` / contenteditable / `.cm-editor`, so the
  // editor's local handling always wins; suppressing the registration
  // is belt-and-suspenders for the case where a non-editable element
  // holds focus while `editing` is true.
  const editCommands = useMemo<readonly CommandDef[]>(() => {
    if (editing) return EMPTY_COMMANDS;
    // No `onEdit` AND no provider stack to drill into → nothing the
    // command could do, leave Enter to global `nav.drillIn`.
    if (!onEdit && (!spatialActions || !focusActions)) return EMPTY_COMMANDS;
    return [
      {
        id: "field.edit",
        name: "Edit Field",
        keys: { vim: "Enter", cua: "Enter" },
        execute: async () => {
          // Drill into spatial children first — pills (`<FocusScope>`
          // leaves) win over edit mode. Read the focused key off the
          // spatial provider: the command only fires when this field
          // zone is the focused entity, so `focusedKey()` returns this
          // field's `SpatialKey`.
          if (spatialActions && focusActions) {
            const key = spatialActions.focusedKey();
            if (key !== null) {
              const moniker = await spatialActions.drillIn(key);
              if (moniker !== null) {
                focusActions.setFocus(moniker);
                return;
              }
            }
          }
          // Kernel returned null (no spatial children) — fall through
          // to the editor. `onEdit` is optional: a read-only field
          // with no children produces a no-op, which matches the
          // "Enter on a leaf with nothing to do" contract.
          onEdit?.();
        },
      },
    ];
  }, [editing, onEdit, spatialActions, focusActions]);

  const inner = editing ? (
    <FieldEditor
      fieldDef={fieldDef}
      value={value}
      entity={entity}
      mode={mode}
      onCommit={handleCommit}
      onCancel={handleCancel}
      onChange={debouncedOnChange}
    />
  ) : (
    <FieldDisplayContent
      fieldDef={fieldDef}
      value={value}
      entity={entity}
      mode={mode}
      onEdit={onEdit}
      onCommit={handleDisplayCommit}
    />
  );

  // When the consumer opts into `withIcon`, render the resolved icon as
  // the leftmost child *inside* the `<FocusZone>`. The icon, the content
  // wrapper, and the focus indicator all share the zone's containing
  // block — so a click on the icon dispatches `spatial_focus` for the
  // field's zone (it bubbles to the zone's click handler) and the
  // `<FocusIndicator>`'s `-left-2` offset paints to the LEFT of the
  // icon. Without `withIcon` we render `inner` bare, identical to every
  // pre-existing callsite (grid cells, card cells, navbar percent-
  // complete, …).
  let zoneChildren: ReactNode = inner;
  if (withIcon) {
    const { Icon, tip } = resolveFieldIconAndTip(fieldDef, value);
    zoneChildren = (
      <div className="flex items-start gap-2">
        {Icon && <FieldIconBadge Icon={Icon} tip={tip} />}
        <div className="flex-1 min-w-0">{inner}</div>
      </div>
    );
  }

  // A field row wraps a real entity field moniker
  // (`field:<type>:<id>.<name>`), so double-clicking a field row
  // (e.g. in the inspector) opens the inspector for that field. The
  // `<Inspectable>` wrapper owns the inspector dispatch; the spatial
  // primitive `<FocusZone>` stays pure-spatial. Per the architectural
  // guard (`focus-architecture.guards.node.test.ts`, Guards B + C),
  // every entity zone — including `field:` — must be wrapped.
  const fmk = asMoniker(fieldMoniker(entityType, entityId, fieldDef.name));
  return (
    <Inspectable moniker={fmk}>
      <FocusZone
        moniker={fmk}
        handleEvents={handleEvents}
        showFocusBar={showFocusBar}
        commands={editCommands}
      >
        {zoneChildren}
      </FocusZone>
    </Inspectable>
  );
}
