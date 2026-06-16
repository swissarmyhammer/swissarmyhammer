//! End-to-end tests for the `applies_to` capability gate on `list command`.
//!
//! `applies_to` is a declarative capability set on a command registration:
//! the entity types the command supports. When present, `list command`
//! filters the command out unless the focused object (from `ctx.target` /
//! `ctx.scope_chain`) is one of the declared types. When absent, the
//! command is unconstrained (global) exactly as before.
//!
//! This is the metadata-driven seam that stops cross-cutting clipboard
//! commands (`entity.cut` / `entity.copy` / `entity.paste`) from surfacing
//! on entity types that don't support them (views, perspectives): the
//! capability is DATA on the declaration, interpreted by one code path in
//! `list_filter_matches` — never a hardcoded `if (type === "view")` branch
//! in the UI.

mod common;

use common::{call_tool, list_ids};
use serde_json::{json, Value};
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

/// Register a command that declares it applies to the given entity types.
fn register_with_applies_to(id: &str, name: &str, applies_to: &[&str]) -> Value {
    json!({
        "op": "register command",
        "id": id,
        "name": name,
        "execute": { "$callback": "cb_x" },
        "context_menu": true,
        "params": [{ "name": "moniker", "from": "target" }],
        "applies_to": applies_to,
    })
}

/// Register the clipboard-shaped fixture: three cross-cutting commands that
/// declare they apply only to clipboard-capable entity types (task, tag,
/// column, board, attachment) — never to views or perspectives.
async fn register_clipboard_fixture(service: &CommandService, caller: &CallerId) {
    // Deliberately REDUCED fixture, NOT the canonical production set: the real
    // clipboard capability is `COPYABLE_ENTITY_TYPES`
    // (`swissarmyhammer-kanban::commands::clipboard_commands`), which also
    // includes `actor` and `project`. This isolated unit only needs a
    // representative supported set vs the unsupported view/perspective to
    // exercise the list-time gate; the production set is pinned against the
    // Rust constant by `builtin_entity_commands_e2e::assert_clipboard_applies_to`.
    let copyable = &["task", "tag", "column", "board", "attachment"];
    for (id, name) in [
        ("entity.cut", "Cut {{entity.type}}"),
        ("entity.copy", "Copy {{entity.type}}"),
        ("entity.paste", "Paste {{entity.type}}"),
    ] {
        call_tool(
            service,
            "register command",
            register_with_applies_to(id, name, copyable),
            caller,
        )
        .await
        .expect("fixture register should succeed");
    }
}

#[tokio::test]
async fn clipboard_commands_absent_when_a_view_is_the_context_menu_target() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_clipboard_fixture(&service, &caller).await;

    // Right-click on a view button: the context menu fires over `view:v1`.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "view:v1", "scope_chain": ["view:v1"] },
        }),
        &caller,
    )
    .await;

    assert!(
        !got.contains("entity.cut")
            && !got.contains("entity.copy")
            && !got.contains("entity.paste"),
        "clipboard commands must NOT appear when a view is focused; got {got:?}",
    );
}

#[tokio::test]
async fn clipboard_commands_absent_when_a_perspective_is_focused() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_clipboard_fixture(&service, &caller).await;

    // A focused perspective tab: the scope chain leaf is `perspective:p1`,
    // no target moniker (palette semantics).
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "scope_chain": ["perspective:p1"] },
        }),
        &caller,
    )
    .await;

    assert!(
        !got.contains("entity.cut")
            && !got.contains("entity.copy")
            && !got.contains("entity.paste"),
        "clipboard commands must NOT appear when a perspective is focused; got {got:?}",
    );
}

#[tokio::test]
async fn clipboard_commands_present_when_a_task_is_focused() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_clipboard_fixture(&service, &caller).await;

    // A focused task: task is a clipboard-capable type, so the commands
    // must still surface and work — the fix must not regress them.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "task:01X", "scope_chain": ["task:01X", "column:todo"] },
        }),
        &caller,
    )
    .await;

    assert!(
        got.contains("entity.cut") && got.contains("entity.copy") && got.contains("entity.paste"),
        "clipboard commands MUST appear when a task is focused; got {got:?}",
    );
}

/// Register the subject-vs-paste-target fixture as the production
/// `entity-commands` bundle declares them after the subject/paste-target split:
/// the five SUBJECT commands (cut/copy/delete/archive/unarchive) gate on the
/// subject set (which deliberately EXCLUDES `board` — the root can't be the
/// subject of its own cut/copy/delete), while `entity.paste` gates on the
/// paste-TARGET set (which INCLUDES `board` — the board is a legitimate paste
/// target via the `task_into_board` / `column_into_board` handlers).
///
/// Deliberately a REDUCED representative set; the production sets are pinned
/// against the Rust constants by the e2e drift guards.
async fn register_subject_paste_split_fixture(service: &CommandService, caller: &CallerId) {
    // SUBJECT set: no `board` — these act ON the focused entity as the subject.
    let subject = &["task", "tag", "column", "actor", "project", "attachment"];
    for (id, name) in [
        ("entity.cut", "Cut {{entity.type}}"),
        ("entity.copy", "Copy {{entity.type}}"),
        ("entity.delete", "Delete {{entity.type}}"),
        ("entity.archive", "Archive {{entity.type}}"),
        ("entity.unarchive", "Unarchive {{entity.type}}"),
    ] {
        call_tool(
            service,
            "register command",
            register_with_applies_to(id, name, subject),
            caller,
        )
        .await
        .expect("fixture register should succeed");
    }
    // PASTE-TARGET set: INCLUDES `board` — clipboard contents drop INTO it. The
    // caption is plain "Paste" (clipboard-driven), never "Paste {{entity.type}}".
    let paste_targets = &["task", "attachment", "board", "column", "project"];
    call_tool(
        service,
        "register command",
        register_with_applies_to("entity.paste", "Paste", paste_targets),
        caller,
    )
    .await
    .expect("fixture register should succeed");
}

#[tokio::test]
async fn subject_commands_absent_but_paste_present_when_the_root_board_is_focused() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_subject_paste_split_fixture(&service, &caller).await;

    // Right-click the root board background: the context menu fires over the
    // board moniker. The five subject ops are meaningless on the root and must
    // NOT surface; paste MUST surface (the board is a paste target).
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "board:b1", "scope_chain": ["board:b1"] },
        }),
        &caller,
    )
    .await;

    for absent in [
        "entity.cut",
        "entity.copy",
        "entity.delete",
        "entity.archive",
        "entity.unarchive",
    ] {
        assert!(
            !got.contains(absent),
            "{absent} must NOT surface on the root board — the board cannot be \
             the subject of its own cut/copy/delete/archive; got {got:?}",
        );
    }
    assert!(
        got.contains("entity.paste"),
        "entity.paste MUST surface on the root board — it is a valid paste \
         target; got {got:?}",
    );
}

#[tokio::test]
async fn subject_commands_present_when_a_column_is_focused() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_subject_paste_split_fixture(&service, &caller).await;

    // A focused column is a real subject — all five subject ops must surface
    // (no regression from excluding `board` from the subject set).
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "column:todo", "scope_chain": ["column:todo", "board:b1"] },
        }),
        &caller,
    )
    .await;

    for present in [
        "entity.cut",
        "entity.copy",
        "entity.delete",
        "entity.archive",
        "entity.unarchive",
        "entity.paste",
    ] {
        assert!(
            got.contains(present),
            "{present} MUST surface when a column is focused; got {got:?}",
        );
    }
}

/// Register the CRUD/inspect-shaped fixture: the four cross-cutting commands
/// (`entity.delete` / `entity.archive` / `entity.unarchive` / `app.inspect`)
/// that — like the clipboard trio — declare they apply only to real
/// cross-cutting entity types (task, tag, column, board, attachment), never to
/// a `field:` projection moniker.
///
/// Deliberately a REDUCED representative set (not the canonical production
/// `COPYABLE_ENTITY_TYPES`, which also includes `actor` / `project`); this
/// isolated unit only needs a supported type (task) vs the unsupported `field`
/// projection to exercise the list-time gate. The production set is pinned
/// against the Rust constant by
/// `builtin_entity_commands_e2e::assert_clipboard_applies_to` and the e2e
/// field-target cases.
async fn register_crud_inspect_fixture(service: &CommandService, caller: &CallerId) {
    let operable = &["task", "tag", "column", "board", "attachment"];
    for (id, name) in [
        ("entity.delete", "Delete {{entity.type}}"),
        ("entity.archive", "Archive {{entity.type}}"),
        ("entity.unarchive", "Unarchive {{entity.type}}"),
        ("app.inspect", "Inspect {{entity.type}}"),
    ] {
        call_tool(
            service,
            "register command",
            register_with_applies_to(id, name, operable),
            caller,
        )
        .await
        .expect("fixture register should succeed");
    }
}

#[tokio::test]
async fn crud_inspect_commands_absent_when_a_field_is_the_context_menu_target() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_crud_inspect_fixture(&service, &caller).await;

    // Context menu fired over a field row: the target is the field's
    // `field:{type}:{id}.{name}` projection moniker (explicit target wins
    // verbatim, so `focused_entity_type` resolves the leading `field` type),
    // and the focused chain leaf is the same field moniker.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": {
                "target": "field:task:01ABC.title",
                "scope_chain": ["field:task:01ABC.title", "ui:field", "task:01ABC"],
            },
        }),
        &caller,
    )
    .await;

    assert!(
        !got.contains("entity.delete")
            && !got.contains("entity.archive")
            && !got.contains("entity.unarchive")
            && !got.contains("app.inspect"),
        "delete/archive/unarchive/inspect must NOT appear when a field is the \
         focus — a field is a projection, not an entity; got {got:?}",
    );
}

#[tokio::test]
async fn crud_inspect_commands_present_when_a_task_is_focused() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_crud_inspect_fixture(&service, &caller).await;

    // A focused task is a real entity, so all four must still surface and
    // work — the field-suppression must not regress real-entity focus.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "task:01ABC", "scope_chain": ["task:01ABC", "column:todo"] },
        }),
        &caller,
    )
    .await;

    assert!(
        got.contains("entity.delete")
            && got.contains("entity.archive")
            && got.contains("entity.unarchive")
            && got.contains("app.inspect"),
        "delete/archive/unarchive/inspect MUST appear when a task is focused; got {got:?}",
    );
}

/// Register `app.inspect` as the `app-shell-commands` bundle declares it after
/// the subject/inspect decoupling: gated on the INSPECTABLE set, which INCLUDES
/// `board` (the root board can never be a cut/copy/delete SUBJECT, but it IS
/// inspectable — "Inspect Board"), while still EXCLUDING the `field:` projection.
///
/// Deliberately a REDUCED representative set; the production inspect set is
/// pinned against the Rust inspectable set (subject `COPYABLE_ENTITY_TYPES` plus
/// `board`) by `builtin_app_shell_commands_e2e::assert_inspect_applies_to`.
async fn register_inspect_fixture(service: &CommandService, caller: &CallerId) {
    // INSPECTABLE set: INCLUDES `board` (Inspect Board), unlike the subject set.
    let inspectable = &["task", "tag", "column", "board", "attachment"];
    call_tool(
        service,
        "register command",
        register_with_applies_to("app.inspect", "Inspect {{entity.type}}", inspectable),
        caller,
    )
    .await
    .expect("fixture register should succeed");
}

#[tokio::test]
async fn inspect_present_on_the_root_board_but_absent_on_a_field() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_inspect_fixture(&service, &caller).await;

    // Right-click the root board background: "Inspect Board" MUST surface — the
    // board is inspectable even though it is never a cut/copy/delete subject.
    let on_board = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "board:b1", "scope_chain": ["board:b1"] },
        }),
        &caller,
    )
    .await;
    assert!(
        on_board.contains("app.inspect"),
        "app.inspect MUST surface on the root board — \"Inspect Board\" is a \
         meaningful root-board affordance; got {on_board:?}",
    );

    // A focused field is a projection, not an entity — "Inspect Field" must NOT
    // surface (board PRESENT, field ABSENT is the whole point of the split).
    let on_field = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": {
                "target": "field:task:01ABC.title",
                "scope_chain": ["field:task:01ABC.title", "ui:field", "task:01ABC"],
            },
        }),
        &caller,
    )
    .await;
    assert!(
        !on_field.contains("app.inspect"),
        "app.inspect must NOT surface when a field is focused — a field is a \
         projection, not an entity; got {on_field:?}",
    );
}

/// Register the `field.edit` registration as the `app-shell-commands` plugin
/// declares it: scope-gated to the `ui:field` marker, surfaced on the context
/// menu, NO `applies_to` (the scope marker — not the capability set — is the
/// gate; see the gating nuance in card `01KV30ZXHWPS4FZK9WEH4DMMZY`). A
/// `field:` projection moniker resolves through `focused_entity_type` to its
/// CONTAINING entity for a palette focus, but to `"field"` for an explicit
/// context-menu target, so an `applies_to: ["field"]` gate would behave
/// differently across the two surfaces — which is exactly why `field.edit`
/// relies on the scope marker instead.
fn register_field_edit(service_payload_id: &str) -> Value {
    json!({
        "op": "register command",
        "id": service_payload_id,
        "name": "Edit Field",
        "execute": { "$callback": "cb_x" },
        "scope": ["ui:field"],
        "context_menu": true,
        "context_menu_group": 0,
        "context_menu_order": 0,
        "keys": { "vim": "i", "cua": "Enter" },
    })
}

#[tokio::test]
async fn field_edit_surfaces_on_a_field_context_menu_target() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    call_tool(
        &service,
        "register command",
        register_field_edit("field.edit"),
        &caller,
    )
    .await
    .expect("register should succeed");

    // Context menu fired over a field row: the explicit target is the field's
    // `field:{type}:{id}.{name}` moniker; the focused chain carries the
    // `ui:field` marker. The client-side context-menu filter additionally
    // requires `context_menu: true` AND a scope match against the chain — but
    // the SERVER `list command` must already include the command, gated only
    // by its `scope` marker being in the chain (the `scope` arg the context
    // menu sends).
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "scope": "ui:field",
            "ctx": {
                "target": "field:task:01ABC.title",
                "scope_chain": ["field:task:01ABC.title", "ui:field", "task:01ABC"],
            },
        }),
        &caller,
    )
    .await;

    assert!(
        got.contains("field.edit"),
        "field.edit MUST surface when a field row is the context-menu target; got {got:?}",
    );
}

#[tokio::test]
async fn field_edit_surfaces_in_the_palette_for_a_focused_field() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    call_tool(
        &service,
        "register command",
        register_field_edit("field.edit"),
        &caller,
    )
    .await
    .expect("register should succeed");

    // The command palette filters `list command` by the INNERMOST focused
    // scope moniker (`scopeChain[0]`). For a focused field that leaf is the
    // `field:` zone moniker — the `ui:field` marker is its PARENT in the
    // chain. The palette surfaces a scope-gated command whenever its declared
    // scope appears anywhere in the focused chain (the marker-in-chain gate),
    // so `field.edit` must list here even though the leaf itself is not
    // `ui:field`.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "scope": "field:task:01ABC.title",
            "ctx": {
                "scope_chain": ["field:task:01ABC.title", "ui:field", "task:01ABC"],
            },
        }),
        &caller,
    )
    .await;

    assert!(
        got.contains("field.edit"),
        "field.edit MUST surface in the palette when a field is focused (the \
         ui:field marker is in the focused chain); got {got:?}",
    );
}

#[tokio::test]
async fn field_edit_absent_in_the_palette_for_a_focused_non_field_entity() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    call_tool(
        &service,
        "register command",
        register_field_edit("field.edit"),
        &caller,
    )
    .await
    .expect("register should succeed");

    // A focused task entity (no field in the chain, no `ui:field` marker):
    // "Edit Field" is nonsensical and must NOT surface.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "scope": "task:01ABC",
            "ctx": {
                "target": "task:01ABC",
                "scope_chain": ["task:01ABC", "column:todo", "board:b1"],
            },
        }),
        &caller,
    )
    .await;

    assert!(
        !got.contains("field.edit"),
        "field.edit must NOT surface on a non-field entity focus; got {got:?}",
    );
}

#[tokio::test]
async fn applies_to_absent_command_is_global_in_every_focus() {
    // A command with NO `applies_to` is unconstrained — it must still list
    // for a view focus, exactly as before. This pins that the gate only
    // restricts commands that opt in to the capability set.
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        json!({
            "op": "register command",
            "id": "app.quit",
            "name": "Quit",
            "execute": { "$callback": "cb_quit" },
        }),
        &caller,
    )
    .await
    .expect("global register should succeed");

    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "view:v1", "scope_chain": ["view:v1"] },
        }),
        &caller,
    )
    .await;

    assert!(
        got.contains("app.quit"),
        "a command with no applies_to must remain global; got {got:?}",
    );
}
