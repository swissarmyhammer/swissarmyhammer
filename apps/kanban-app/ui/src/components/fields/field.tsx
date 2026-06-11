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
 * The zone defaults to `showFocus={false}`. The default exists for
 * grid-cell consumers — they wrap each `<Field>` in their own
 * `<FocusScope>` that already renders a cursor ring around the cell, so
 * a second indicator at the field zone would be redundant. Every other
 * consumer opts in by passing `showFocus={true}` when they want the
 * inner field zone to advertise focus:
 *   - The inspector row (`EntityInspector` → `FieldRow` → `<Field
 *     showFocus />`). Inspector rows fill the panel width and have
 *     no enclosing focus chrome, so the per-row bar is the user's only
 *     focus cue at the row level.
 *   - The card body (`EntityCard` → `CardField` → `<Field
 *     showFocus />`). Card fields render inside a card-zone bar,
 *     but the per-field bar is still the user's only cue for which
 *     atom of the card carries focus (title vs. status vs. tags)
 *     — the card-zone bar fires on the card itself, not on its
 *     descendants. Badge-list pill leaves inside these card fields
 *     advertise their own focus through `MentionView`'s `<FocusScope>`
 *     default of `showFocus={true}`.
 *   - The nav-bar's `<Field>` per-pill children, when their parent
 *     does not already mount a focus indicator.
 *
 * The zone defaults to `handleEvents={true}`. The grid-cell case passes
 * `handleEvents={false}` so the surrounding `grid_cell:R:K`
 * `<FocusScope>` keeps owning click → cursor-ring updates. See the
 * "Decision: Option A" note in card `01KQ5QB6F4MTD35GBTARJH4JEW`.
 */

import {
  useCallback,
  useMemo,
  useRef,
  type ComponentType,
  type ReactNode,
} from "react";
import { useEntityStore, useFieldValue } from "@/lib/entity-store-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useDebouncedSave } from "@/lib/use-debounced-save";
import { isFieldEditable, resolveEditor } from "@/components/fields/editors";
import type { EditorProps } from "@/components/fields/editors";
import { FocusScope } from "@/components/focus-scope";
import { Inspectable } from "@/components/inspectable";
import { CommandScopeProvider, useDispatchCommand } from "@/lib/command-scope";
import { useFocusedWebviewCommandHandlers } from "@/lib/use-focused-webview-command-handlers";
import type { WebviewCommandHandler } from "@/lib/webview-command-bus";
import { fieldMoniker } from "@/lib/moniker";
import { asSegment } from "@/types/spatial";
import { fieldIcon } from "@/components/fields/field-icon";
import { FieldIconBadge } from "@/components/fields/field-icon-badge";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { HelpCircle, type LucideIcon } from "lucide-react";
import type { FieldDef, Entity } from "@/types/kanban";

/**
 * The constant marker moniker every registering `<Field>` mounts into the
 * command scope chain, directly above its `<FocusScope>` zone.
 *
 * Field zones carry dynamic per-field monikers
 * (`field:{type}:{id}.{name}`), so the plugin-defined edit commands cannot
 * be scope-gated on a literal zone moniker the way the grid's `ui:grid`
 * zone is. The marker gives every field zone one shared literal moniker;
 * the `ui-commands` plugin's `field.edit` / `field.editEnter` declare
 * `scope: ["ui:field"]` against it, so their Enter / `i` keys bind exactly
 * while a field zone is in the focused chain — and nowhere else.
 */
export const FIELD_COMMAND_SCOPE = "ui:field";

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
   * When true, the inner `<FocusZone>` shows its own visible focus
   * indicator. Defaults to false so grid-cell consumers — which already
   * wrap each field in a `<FocusScope>` — don't double up on indicators.
   * Consumers without an enclosing focus chrome opt in by passing
   * `<Field showFocus />`: inspector rows (the row fills the panel
   * and the per-row indicator is the user's only cue), card fields
   * (the card-zone indicator fires on the card itself, so the per-field
   * indicator tells the user which atom of the card carries focus). See
   * the file header for the full taxonomy of consumers and why each one
   * opts in or out.
   */
  showFocus?: boolean;
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
   *   2. The visible `<FocusIndicator>` paints inside the zone's box
   *      as a dotted border, surrounding both the icon and the content
   *      rather than appearing between them.
   *   3. Every existing `<Field>` callsite that doesn't opt in via
   *      `withIcon={true}` continues to render exactly as before —
   *      backwards-compatible by default.
   *
   * Defaults to false. The inspector row (`EntityInspector` →
   * `FieldRow`) is currently the only consumer that opts in.
   */
  withIcon?: boolean;
  /**
   * When false, omits the inner `<FocusScope>` wrapper so the field's
   * `field:{type}:{id}.{name}` moniker does NOT register as a scope in
   * the spatial-nav kernel. The `<Inspectable>` wrapper (which owns the
   * double-click → `ui.inspect` dispatch and the Space keybinding) is
   * preserved.
   *
   * Defaults to true.
   *
   * The grid-cell case ({@link file://../data-table.tsx GridCellFocusable})
   * passes `register={false}` because each cell already wraps the
   * `<Field>` in its own `grid_cell:R:K` `<FocusScope>` leaf. The
   * kernel's scope-is-leaf invariant rejects nested scopes — when both
   * the cell scope AND the field scope register, the cell scope is
   * dropped as `scope-not-leaf` (logged to `just logs`) and the cell
   * disappears from the spatial registry, so beam search and
   * click-focus lose their target. Suppressing the inner field scope
   * leaves the cell scope as the sole leaf, which is what the cursor /
   * beam search expects.
   *
   * Inspector rows and card fields keep the default (`true`) — they
   * have no enclosing `<FocusScope>`, so the field scope is the only
   * one in the subtree and registers cleanly.
   */
  register?: boolean;
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
 * # Enter ownership (Card D — plugin-defined, webview-bus handled)
 *
 * The `field.edit` / `field.editEnter` command DEFINITIONS (id / name /
 * keys / scope) live in the `ui-commands` builtin plugin, gated to the
 * constant `ui:field` marker moniker ({@link FIELD_COMMAND_SCOPE}) this
 * component mounts via a `CommandScopeProvider` directly above its
 * `<FocusScope>`. The keymap layer's depth-interleaved chain walk
 * claims Enter (cua) / `i` + Enter (vim) for them whenever the
 * `ui:field` marker appears anywhere in the focused scope chain (the
 * field zone itself or a pill inside it) — shadowing the global
 * `nav.drillIn: Enter` only there, leaving the drill-in default in
 * place for every other focusable. The live BEHAVIOR (drill into pills, else enter
 * edit mode) registers on the webview command bus while spatial focus
 * is anywhere within this zone's subtree (the zone itself or a pill
 * inside it) — matching the keymap's marker-in-chain gate, so Enter on
 * a focused pill still reaches this field's closure; in edit mode the
 * handler short-circuits (the editor element owns Enter via its own
 * keymap).
 *
 * # Read-only/computed metadata gate
 *
 * Editability is metadata-driven: a field whose YAML declares no
 * editor (`editor: "none"` — the shape of computed fields like
 * `status_date` and `virtual_tags`) is display-only. The interpreter
 * enforces this in one place, regardless of what the caller passes:
 * the `onEdit` prop is dropped (so neither click-to-edit nor the
 * `field.edit` Enter closure can arm editing) and the `editing` prop
 * is ignored (so an armed read-only field still renders its display
 * instead of a missing editor that would blank the value).
 */
export function Field({
  fieldDef,
  entityType,
  entityId,
  mode,
  editing,
  onEdit: onEditProp,
  onDone,
  onCancel,
  handleEvents = true,
  showFocus = false,
  withIcon = false,
  register = true,
}: FieldProps) {
  // Metadata gate — the single interpreter-level editability check.
  // A field whose YAML metadata declares no editor (`editor: "none"`,
  // the shape of computed/read-only fields like `status_date` and
  // `virtual_tags`) must never enter edit mode, regardless of what the
  // caller passes:
  //   - `onEdit` is dropped, so neither the click-to-edit surface nor
  //     the `field.edit` Enter closure can arm editing.
  //   - `editing` is ignored, so a caller that arms it anyway still
  //     renders the display. Pre-gate, an armed read-only field mounted
  //     `FieldEditor`, which resolved no registered editor and rendered
  //     `null` — blanking the value with no editor left to fire
  //     onDone/onCancel and restore it.
  const editable = isFieldEditable(fieldDef);
  const onEdit = editable ? onEditProp : undefined;
  const isEditing = editing && editable;

  const value = useFieldValue(entityType, entityId, fieldDef.name);
  const entity = useEntityStore().getEntity(entityType, entityId);
  const { handleCommit, handleDisplayCommit, handleCancel, debouncedOnChange } =
    useFieldHandlers(entityType, entityId, fieldDef.name, onDone, onCancel);

  // Spatial actions feed the `field.edit` execute closure: we read the
  // focused field-zone key from the spatial provider on a successful
  // drill-in. The provider may be absent in lightweight tests, so we
  // use the optional variant — the closure short-circuits when it's
  // missing, falling through to `onEdit?.()` (the legacy behaviour).
  //
  // Card `01KR7CDEFWWVF4WH0BCHE8Y21J`: focus claims after a successful
  // drill-in flow through `nav.focus`, the single auditable command
  // that wraps the entity-focus `setFocus` primitive. The dispatcher
  // is pre-bound here so the closure stays cheap on the keystroke path.
  const spatialActions = useOptionalSpatialFocusActions();
  const dispatchNavFocus = useDispatchCommand("nav.focus");

  // Per-zone Enter behavior (Card D): the `field.edit` / `field.editEnter`
  // command DEFINITIONS (id / name / keys / scope) live in the `ui-commands`
  // builtin plugin, gated to the `ui:field` marker this component mounts
  // above its `<FocusScope>` (see FIELD_COMMAND_SCOPE) — so the keymap layer
  // claims Enter / `i` for them whenever the marker appears anywhere in the
  // focused scope chain (the field zone itself OR a pill inside it),
  // shadowing the global `nav.drillIn: Enter` exactly there. This component
  // registers only the live BEHAVIOR, on the webview command bus, and only
  // WHILE SPATIAL FOCUS IS WITHIN THIS ZONE'S SUBTREE — the same granularity
  // as the keymap gate, so a dispatched id always reaches the closure of the
  // one field instance containing the focus (with many fields mounted, the
  // subtrees are disjoint).
  //
  // The handler unifies "drill into pills" and "open editor":
  // it asks the kernel to drill into the field zone first; on a non-null
  // moniker (the field has spatial children — pills, badges, …) it
  // dispatches `nav.focus` and returns. Only when the kernel
  // echoes the focused FQM does it fall through to `onEdit?.()`. This means
  // a field with pills navigates to the first pill, while a field without
  // pills (and an `onEdit` callback) opens its editor — same command id,
  // two outcomes driven by the registry's structural answer. For
  // non-editable fields with no spatial children, the kernel echoes and
  // `onEdit` is undefined → Enter is a no-op.
  //
  // The handler short-circuits in edit mode: the editor element holds DOM
  // focus and owns Enter via its own keymap (commit on submit, newline in
  // multiline, etc.). The global keymap handler's `isEditableTarget` gate
  // also short-circuits before any binding resolution when the focused
  // element is an `<input>` / `<textarea>` / contenteditable / `.cm-editor`,
  // so the editor's local handling always wins; the in-handler check is
  // belt-and-suspenders for the case where a non-editable element holds
  // focus while `editing` is true.
  //
  // Latest props/actions ride in a ref so the handler registration never
  // churns on render — mirrors grid-view's `ctxRef` pattern.
  const editCtxRef = useRef({
    isEditing,
    onEdit,
    spatialActions,
    dispatchNavFocus,
  });
  editCtxRef.current = { isEditing, onEdit, spatialActions, dispatchNavFocus };
  const editHandlers = useMemo<
    Readonly<Record<string, WebviewCommandHandler>>
  >(() => {
    const editClosure = async () => {
      const {
        isEditing: editing,
        onEdit: enterEdit,
        spatialActions: spatial,
        dispatchNavFocus: navFocus,
      } = editCtxRef.current;
      if (editing) return;
      // Drill into spatial children first — pills (`<FocusScope>`
      // leaves) win over edit mode. Read the focused FQM off the
      // spatial provider: the command only fires while focus is
      // within this field's subtree, so `focusedFq()` returns the
      // field zone's FQM (drills into the first pill, if any) or a
      // pill's FQM (a leaf — the kernel echoes and the closure falls
      // through to the editor).
      //
      // Under the no-silent-dropout contract the kernel always
      // returns an FQM; we detect "no descent happened" by
      // comparing the result to the focused FQM. Equality
      // means the field has no spatial children — fall through to
      // the editor.
      if (spatial) {
        const focusedFq = spatial.focusedFq();
        if (focusedFq !== null) {
          const result = await spatial.drillIn(focusedFq, focusedFq);
          if (result !== focusedFq) {
            // Card `01KR7CDEFWWVF4WH0BCHE8Y21J`: focus claims flow
            // through `nav.focus`. The drill-in result is the FQM
            // of the first spatial child (a pill, badge, etc.).
            await navFocus({ args: { fq: result } }).catch((err) =>
              console.error("[field.edit] nav.focus dispatch failed", err),
            );
            return;
          }
        }
      }
      // Kernel echoed the focused FQM (no spatial children) —
      // fall through to the editor. `onEdit` is optional: a
      // read-only field with no children produces a no-op, which
      // matches the "Enter on a leaf with nothing to do" contract.
      enterEdit?.();
    };
    // Two ids share one body: the plugin splits `field.edit` (vim:i,
    // cua:Enter) and `field.editEnter` (vim:Enter) only because each `keys`
    // entry in the command metadata is one binding per keymap.
    return { "field.edit": editClosure, "field.editEnter": editClosure };
  }, []);

  // A field row wraps a real entity field moniker
  // (`field:<type>:<id>.<name>`); the same segment names the `<FocusScope>`
  // below and keys the focus-gated bus registration here. When
  // `register={false}` (the grid-cell case) the zone never registers, no
  // focused FQM ever sits within this moniker's subtree, and the handlers
  // never install — the enclosing grid cell owns edit-mode entry through
  // `grid.edit` instead.
  const fmk = asSegment(fieldMoniker(entityType, entityId, fieldDef.name));
  useFocusedWebviewCommandHandlers(fmk, editHandlers);

  const inner = isEditing ? (
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
  // `<FocusIndicator>`'s dotted-inset border traces the zone's bounds,
  // surrounding both the icon and the content. Without `withIcon` we
  // render `inner` bare, identical to every pre-existing callsite (grid
  // cells, card cells, navbar percent-complete, …).
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

  // Double-clicking a field row (e.g. in the inspector) opens the inspector
  // for that field. The `<Inspectable>` wrapper owns the inspector dispatch;
  // the spatial primitive `<FocusZone>` stays pure-spatial. Per the
  // architectural guard (`focus-architecture.guards.node.test.ts`, Guards
  // B + C), every entity zone — including `field:` — must be wrapped.
  //
  // The marker `CommandScopeProvider` sits directly above the
  // `<FocusScope>` so the zone's command-scope chain contains the literal
  // `ui:field` moniker — the gate the plugin-defined `field.edit` /
  // `field.editEnter` commands' `scope` names (see FIELD_COMMAND_SCOPE).
  //
  // When `register={false}` (the grid-cell case), the inner
  // `<FocusScope>` is omitted so the field's moniker does not register
  // as a scope (and the marker is omitted with it — there is no field
  // zone for the edit keys to gate on). The enclosing `grid_cell:R:K`
  // `<FocusScope>` is the sole leaf in that subtree, which is what the
  // kernel's scope-is-leaf invariant requires (a registered `<FocusScope>`
  // cannot contain another). The `<Inspectable>` wrapper is kept so
  // double-click → field inspect and the Space keybinding stay wired
  // — both are independent of the spatial registration.
  return (
    <Inspectable moniker={fmk}>
      {register ? (
        <CommandScopeProvider moniker={FIELD_COMMAND_SCOPE}>
          <FocusScope
            moniker={fmk}
            handleEvents={handleEvents}
            showFocus={showFocus}
          >
            {zoneChildren}
          </FocusScope>
        </CommandScopeProvider>
      ) : (
        zoneChildren
      )}
    </Inspectable>
  );
}
