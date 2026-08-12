//! Test helpers shared by the `edit` module's sibling test modules.
//!
//! Each helper below is used by more than one of the test modules under `edit`,
//! so it lives here once instead of being restated per module.

use rmcp::model::CallToolResult;

/// Build an `edit files` argument map: the target `file_path`, a find/replace
/// pair under the given key names, and one optional extra member.
///
/// The two shapes below differ only in which key names carry the pair and which
/// optional member they add, so both delegate here.
fn edit_arguments(
    file_path: &str,
    find: (&str, &str),
    replace: (&str, &str),
    extra: Option<(&str, serde_json::Value)>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut args = serde_json::Map::new();
    for (key, value) in [("file_path", file_path), find, replace] {
        args.insert(
            key.to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
    if let Some((key, value)) = extra {
        args.insert(key.to_string(), value);
    }
    args
}

/// Create test arguments for the edit tool under the legacy alias keys
/// (`old_string` / `new_string`), with an optional `replace_all` flag.
pub(super) fn create_edit_arguments(
    file_path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: Option<bool>,
) -> serde_json::Map<String, serde_json::Value> {
    let flag = replace_all.map(|f| ("replace_all", serde_json::Value::Bool(f)));
    edit_arguments(
        file_path,
        ("old_string", old_string),
        ("new_string", new_string),
        flag,
    )
}

/// Build a JSON arg map with the canonical `find`/`replace` keys (and an
/// optional 1-based `occurrence` disambiguation hint).
pub(super) fn ambiguity_args(
    file_path: &str,
    find: &str,
    replace: &str,
    occurrence: Option<u64>,
) -> serde_json::Map<String, serde_json::Value> {
    let hint = occurrence.map(|n| ("occurrence", serde_json::Value::Number(n.into())));
    edit_arguments(file_path, ("find", find), ("replace", replace), hint)
}

/// Read the text payload of a `CallToolResult`.
pub(super) fn result_text(result: &CallToolResult) -> String {
    match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => t.text.clone(),
        _ => panic!("expected text content"),
    }
}
