---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd580
title: Add param shape and tab_button metadata to CommandDef
---
## What

Foundation for the command-driven tab-button refactor. Two new metadata fields on `CommandDef` so commands can declare both *how* their params should be picked at runtime AND that they should render as a tab button. No behavior changes in this task — just the shape, serde wiring, TS types, and round-trip tests. Subsequent tasks consume the metadata.

### Files to modify

- `swissarmyhammer-commands/src/types.rs` — extend `CommandDef`:
  ```rust
  /// When set, this command renders as a tab-button affordance on
  /// surfaces that consume `tab_button`-tagged commands (today: the
  /// perspective tab bar). Absent means no tab-button.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub tab_button: Option<TabButtonDef>,
  ```
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  pub struct TabButtonDef {
      /// Lucide-react icon component name (e.g. "filter", "group",
      /// "arrow-up-down"). Resolved by the frontend's icon registry
      /// at render time; an unknown name renders a fallback.
      pub icon: String,
  }
  ```

  Extend `ParamDef` (or whatever type backs `params[]`) with two optional fields:
  ```rust
  /// Shape of this param for runtime collection. When `None`, the
  /// param's `from` field (args / scope) already supplies the value —
  /// the runtime never asks the user for it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub shape: Option<ParamShape>,

  /// For enum-shaped params, names the backend resolver that
  /// supplies the concrete option list at `commands_for_scope`
  /// emission time. Resolver names are stringly-typed and looked up
  /// in a backend resolver registry (separate task).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub options_from: Option<String>,
  ```
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  #[serde(rename_all = "kebab-case")]
  pub enum ParamShape {
      /// User picks from a list of options. Options come from
      /// `options_from` resolver OR an inline `options: [...]`.
      Enum,
      /// Single-line free text.
      Text,
      /// Multiline expression (e.g. filter DSL). Frontend hosts a
      /// rich editor (CodeMirror) for this shape.
      Expression,
      Number,
      Date,
      Boolean,
  }
  ```

  Also extend `ParamDef` with optional inline options for cases where the option list is static and known at YAML write time:
  ```rust
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub options: Option<Vec<ParamOption>>,
  ```
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
  pub struct ParamOption {
      pub value: String,
      pub label: String,
  }
  ```

- `kanban-app/ui/src/types/kanban.ts` — mirror the new fields on the TS `CommandDef` interface:
  ```ts
  export interface CommandDef {
    // ... existing ...
    readonly tab_button?: { readonly icon: string };
    readonly params?: readonly ParamDef[];
  }
  export interface ParamDef {
    readonly name: string;
    readonly from?: "args" | "scope" | "picker";
    readonly shape?: "enum" | "text" | "expression" | "number" | "date" | "boolean";
    readonly options_from?: string;
    readonly options?: readonly { value: string; label: string }[];
    /** Resolved at emission time when `options_from` was set; backend supplies the populated list. */
  }
  ```

### Compatibility

- All new fields are optional. Existing YAML files parse unchanged.
- `tab_button: None` and `params[].shape: None` are the defaults; nothing rendered/picked unless opted in.

### Out of scope (handled by dependent tasks)

- Backend resolver registry that populates `options` from `options_from` — separate task.
- Frontend `<CommandButton>` / `<CommandPopover>` components — separate task.
- Annotating individual commands with `tab_button` / `shape` — one task per migration.

## Acceptance Criteria

- [x] `CommandDef` carries `tab_button: Option<TabButtonDef>` with the documented semantics.
- [x] `ParamDef` carries `shape: Option<ParamShape>`, `options_from: Option<String>`, and `options: Option<Vec<ParamOption>>`.
- [x] A `CommandDef` YAML with no new fields round-trips through `serde_yaml_ng` unchanged.
- [x] A `CommandDef` YAML with `tab_button: { icon: "filter" }` and a param `{ name: "field", shape: "enum", options_from: "perspective.fields" }` round-trips correctly.
- [x] TS types mirror the Rust shape; a TS literal omitting all new fields type-checks as `CommandDef`.
- [x] `cargo test -p swissarmyhammer-commands` and `pnpm -C kanban-app/ui test` both pass.

## Tests

- [x] Unit test in `swissarmyhammer-commands/src/types.rs` mirroring `command_def_yaml_round_trip` style: `command_def_with_tab_button_round_trips` — construct a `CommandDef` with `tab_button: Some(TabButtonDef { icon: "filter".into() })`, serialize to YAML, parse back, assert equality.
- [x] Unit test: `command_def_with_param_shape_and_options_round_trips` — param with `shape: Some(Enum), options_from: Some("perspective.fields"), options: Some(vec![ParamOption { value: "status".into(), label: "Status".into() }])` survives a YAML round-trip.
- [x] Unit test: `command_def_without_new_fields_omits_them_from_yaml` — a minimal `CommandDef` does NOT emit `tab_button:`, `shape:`, `options_from:`, or `options:` lines.
- [x] TS type test in `kanban-app/ui/src/types/kanban.test.ts`: a literal `{ id, name, scope, params: [{ name: "field", shape: "enum", options_from: "perspective.fields" }] }` satisfies `CommandDef` without errors.
- [x] Run: `cargo test -p swissarmyhammer-commands` — green (187 passed).
- [x] Run: `pnpm -C kanban-app/ui test` — green (2110 passed; the project is npm-configured so the command run was `npm test`).

## Workflow

- Use `/tdd` — write the four round-trip tests first, watch them fail (the types don't exist yet), then add the fields and make them pass.
- Match the serde-attribute style on neighboring optional fields in the same file (`menu`, `keys`, `view_kinds`) for consistency: `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- Document the `None` semantics for both new fields directly on the struct so future readers don't have to chase the migration tasks for context. #command-driven-ui