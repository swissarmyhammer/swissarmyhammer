---
assignees:
- claude-code
depends_on:
- 01KRE1SSN9AX8R67XC58HHQKKB
position_column: doing
position_ordinal: '8180'
title: Relocate DynamicSources and friends out of swissarmyhammer-kanban
---
## Architectural decision (2026-05-12)

A prior implementation pass surfaced an unresolved contradiction in this task description: it required `DynamicSources` to live in `swissarmyhammer-commands`, but the struct's fields reference `BoardInfo` (which is staying in `-kanban`) and other domain types, which would force `-commands` to depend on `-kanban` and create a cycle.

**Resolution: Option A.** `DynamicSources` (the aggregator) lives in `kanban-app` (the GUI assembly point), not in `-commands`. `commands_for_scope` still moves to `-commands` and consumes the aggregator via `&dyn Any` (already wired by task 01KRE1SSN9AX8R67XC58HHQKKB). This matches migration step 6 below ("the aggregator in a thin assembly crate or `kanban-app` itself, which is already the assembly point for the GUI"). All other acceptance criteria stand.

## What

`swissarmyhammer-kanban/src/scope_commands.rs` and `swissarmyhammer-kanban/src/dynamic_sources.rs` host a cluster of types that are **not** kanban-specific. They were placed in the kanban crate when the command-scope work first landed because that's where `commands_for_scope` lived, but conceptually each belongs to its domain crate. Kanban becomes one consumer (and producer of `BoardInfo`) among several.

### Current home → proposed home

| Type | Today | Belongs in | Why |
|---|---|---|---|
| `DynamicSources` (aggregator) | `swissarmyhammer-kanban::scope_commands` | `kanban-app` | Resolved per decision above — the aggregator references types from every domain crate, so it must live above them in the dep graph. `kanban-app` is the only place that already sees all the domain crates and is where `build_dynamic_sources` is invoked from today. |
| `ViewInfo` | `swissarmyhammer-kanban::scope_commands` | `swissarmyhammer-views` | Describes a view by id/name/kind/entity_type — pure view-domain data. |
| `PerspectiveInfo` | `swissarmyhammer-kanban::scope_commands` | `swissarmyhammer-perspectives` | Lightweight perspective descriptor — the perspectives crate already owns the heavyweight `Perspective` type. |
| `PerspectiveFieldInfo` | `swissarmyhammer-kanban::scope_commands` | `swissarmyhammer-perspectives` (or `swissarmyhammer-fields`) | Denormalized field-id + display-name pair for a perspective's columns. The display-name comes from `FieldsContext`, so either home is defensible; perspectives crate is closer to the consumer. |
| `WindowInfo` | `swissarmyhammer-kanban::scope_commands` | `swissarmyhammer-commands` (or new ui-state) | GUI runtime descriptor that the scope dispatcher consumes; it has nothing to do with kanban specifically. |
| `BoardInfo` | `swissarmyhammer-kanban::scope_commands` | **stays in kanban** | Boards are the only genuinely kanban-specific concept here. |
| `PerspectiveFieldsResolver` | `swissarmyhammer-kanban::commands::options_resolvers` | `swissarmyhammer-perspectives` | Builds options from a perspective's field list — perspective-domain logic. |
| `ViewKindsResolver` | `swissarmyhammer-kanban::commands::options_resolvers` | `swissarmyhammer-views` | Constant list of `ViewKind` variants via `as_kebab_str()` — pure view-domain. |
| `SortDirectionsResolver` | `swissarmyhammer-kanban::commands::options_resolvers` | `swissarmyhammer-commands` | Two static entries (asc, desc); no domain dependency. |
| `default_options_registry()` | kanban | refactored — each domain crate registers its own resolvers; kanban only registers its kanban-specific ones (none, currently) | The dependency direction is the test: kanban knows about views/perspectives, not the other way around. |
| `commands_for_scope`, `filter_by_view_kind`, `enrich_options` | `swissarmyhammer-kanban::scope_commands` | `swissarmyhammer-commands` | Already consumer-agnostic in spirit (uses `&dyn Any` for option-resolver source data); the implementation just hasn't moved yet. Takes `&dyn Any` so it never needs `DynamicSources` by type. |
| `build_dynamic_sources` + `DynamicSourcesInputs` | `swissarmyhammer-kanban::dynamic_sources` | split: aggregator lives in `kanban-app` along with `DynamicSources`; per-domain `gather_*` helpers move to their respective crates and are called by the kanban-app assembly function. | The aggregator is necessarily cross-domain, and `kanban-app` is the assembly point. |

### The "kanban knows everything" anti-pattern

Today `swissarmyhammer-kanban` depends on `swissarmyhammer-views`, `swissarmyhammer-perspectives`, `swissarmyhammer-fields`, etc. AND owns the command-scope concept that knits them together. That makes kanban a god-crate. Inverting:

- `swissarmyhammer-commands` defines the scoping concept, `CommandDef`, `OptionsResolver`, `OptionsRegistry`, `commands_for_scope`, `WindowInfo`, `SortDirectionsResolver`. It stays the small stable base — no new domain-crate deps.
- Each domain crate (`-views`, `-perspectives`, etc.) depends on `-commands`, defines its own `*Info` types and `gather_*` helper, and registers its own resolvers.
- `swissarmyhammer-kanban` does the same for `BoardInfo` and its `gather_boards` helper.
- `kanban-app` is the assembly point: it depends on every domain crate plus `-commands`, defines `DynamicSources`, calls each domain's `gather_*`, and hands the aggregator to `commands_for_scope` via `&dyn Any`.

### Migration strategy

1. **Add the new module locations** (no moves yet). E.g. add `pub struct ViewInfo {…}` in `swissarmyhammer-views`, leave the kanban definition in place but mark it `#[deprecated]` with a `pub use swissarmyhammer_views::ViewInfo` re-export.
2. **Migrate one type at a time**, starting with the leafiest (`BoardInfo` stays; `WindowInfo`, `ViewInfo`, `PerspectiveFieldInfo`, `PerspectiveInfo` move one per commit). Each move:
   - Define in target crate.
   - Add `pub use` re-export from old location (so existing call sites compile).
   - Find every direct construction of the type; update the `use` to point at the new home; remove the re-export.
   - Compile, test, commit.
3. **Migrate the resolvers** the same way: define in the domain crate, register from the domain crate, kanban no longer owns the registration.
4. **Migrate `commands_for_scope`** + `filter_by_view_kind` + `enrich_options` to `swissarmyhammer-commands`. `OptionsContext::data: &dyn Any` already supports this — kanban-side resolvers downcast to whatever data they need.
5. **Move `DynamicSources`** to `kanban-app`. Update the lone Rust caller (the bridge that calls `commands_for_scope`) to construct the aggregator there.
6. **Decompose `build_dynamic_sources` + `DynamicSourcesInputs`** — split into per-domain `gather_views`, `gather_perspectives`, `gather_boards`, with the orchestrating function in `kanban-app`. Each `gather_*` lives in its domain crate.

### Out of scope

- Any change to the wire format of `CommandDef` / `ParamDef` / `OptionsResolver` traits. Those are stable from the prior tasks.
- The frontend `<CommandButton>` / `<CommandPopover>` work — sees only the YAML-shape `CommandDef` and is unaffected by where the Rust types live.
- Building a new "ui-state" crate for `WindowInfo` unless one already feels obvious — staging into `swissarmyhammer-commands` for now is fine.

## Acceptance Criteria

- [ ] `ViewInfo` is defined in `swissarmyhammer-views`. `swissarmyhammer-kanban` does not define it; its old definition is gone (not deprecated — fully removed).
- [ ] `PerspectiveInfo` and `PerspectiveFieldInfo` are defined in `swissarmyhammer-perspectives` (or `swissarmyhammer-fields`, document the choice). Kanban does not define either.
- [ ] `WindowInfo` is defined in `swissarmyhammer-commands`. Kanban does not define it.
- [ ] `DynamicSources` (the aggregator) is defined in `kanban-app`. Kanban does not define it; commands does not define it. (Revised from original criterion per the architectural decision above.)
- [ ] `BoardInfo` remains in `swissarmyhammer-kanban` — it's the only genuinely kanban-specific descriptor.
- [ ] `commands_for_scope`, `filter_by_view_kind`, and `enrich_options` live in `swissarmyhammer-commands`. Kanban consumes them like any other domain crate.
- [ ] `PerspectiveFieldsResolver` is defined and registered from `swissarmyhammer-perspectives`. `ViewKindsResolver` is defined and registered from `swissarmyhammer-views`. `SortDirectionsResolver` stays in `swissarmyhammer-commands`. The kanban crate has zero `OptionsResolver` registrations after this task — boards have no enum-shaped params today.
- [ ] `swissarmyhammer-commands` does NOT depend on `swissarmyhammer-views`, `swissarmyhammer-perspectives`, or `swissarmyhammer-kanban`. The small stable base stays small.
- [ ] `swissarmyhammer-kanban` retains its existing deps (it still depends on views, perspectives, fields, commands), BUT no symbols are re-exported from kanban as a passthrough — every consumer imports from the canonical crate.
- [ ] All existing tests still pass: `cargo test --workspace --all-targets` is green.
- [ ] No `pub use ... as ...` legacy re-exports left behind. If a path moved, every caller imports from the new path.
- [ ] `cargo check --workspace --all-targets` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.

## Tests

- [ ] Move corresponds 1:1 — every test that previously exercised `ViewInfo`/`PerspectiveInfo`/etc. still exercises the same behavior at the new home. Specifically:
  - `swissarmyhammer-views`: gains a test that confirms `ViewInfo` constructs and serializes as expected (move the relevant test from `scope_commands.rs`).
  - `swissarmyhammer-perspectives`: gains tests for `PerspectiveInfo` + `PerspectiveFieldInfo` constructors and for the `PerspectiveFieldsResolver` (the existing resolver tests move with the resolver).
  - `swissarmyhammer-commands`: gains tests for `commands_for_scope` filtering, `filter_by_view_kind`, `enrich_options`. Move the integration tests from `swissarmyhammer-kanban/tests/options_enrichment.rs` here (or leave them in kanban as cross-crate integration tests — document the choice).
  - `kanban-app`: gains tests for `DynamicSources` construction and `build_dynamic_sources` aggregation.
  - `swissarmyhammer-kanban`: loses tests for types that moved. The `dynamic_sources_headless.rs` integration test changes shape — it now calls the assembly entrypoint in `kanban-app` which itself calls each domain's `gather_*`.
- [ ] A cross-crate integration test confirms a `DynamicSources` aggregated from views, perspectives, and boards emits the same `commands_for_scope` result as today.
- [ ] Run: `cargo test --workspace --all-targets` — green.
- [ ] Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.

## Workflow

- This task is mechanical but large. Do it incrementally — one type per commit so the diff is reviewable. The migration strategy section above is the suggested commit sequence.
- After each move, run `cargo check --workspace` before moving on. A broken intermediate state is much worse than ten small commits.
- Update the migration tasks (01KRE1YA65MMG29RDQDQ0VPJQG Filter, 01KRE1ZTYJ5PPTQ29K72KE88B5 Group, 01KRE21GJMPP289N1HSTMJG5HE Add + Sort) to reference the new crate locations once this task starts touching their target files. They have annotations that point at `swissarmyhammer-kanban/...` paths today; those need to point at `swissarmyhammer-commands` / `swissarmyhammer-perspectives` / `swissarmyhammer-views` / `kanban-app` after this task lands.
- The new `<CommandButton>` / `<CommandPopover>` task (01KRE1VDTC4MNKN3YPR619NDQK) is unaffected — it only sees the YAML-shape `CommandDef` over the Tauri bridge.
- This task is best sequenced AFTER 01KRE1VDTC4MNKN3YPR619NDQK and 01KRE1WT72MJWNGQBVAD4V5VKM (so the frontend foundation lands without conflict) but BEFORE the three command-migration tasks (5–7).
#command-driven-ui