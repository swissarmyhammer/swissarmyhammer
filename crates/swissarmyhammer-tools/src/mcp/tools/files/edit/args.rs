//! Argument shaping for the `edit files` operation.
//!
//! `edit` accepts several input shapes under several alias names. This module
//! collapses all of them into one canonical list of [`EditPair`]s. Nothing here
//! touches the filesystem, so every rung is unit-testable on its own.

use rmcp::ErrorData as McpError;
use swissarmyhammer_operations::{ParamMeta, ParamType};

/// Alias keys that resolve to the canonical `file_path` parameter.
pub(super) static FILE_PATH_ALIASES: &[&str] = &["path", "filePath", "absolute_path"];

/// Alias keys that resolve to the canonical `find` parameter (the text to match).
///
/// `old_string`/`oldText` are the legacy MCP names, kept here as aliases so the
/// historical single-edit and `edits[]` shapes keep working. The remaining
/// entries are the natural-language synonyms a model is likely to emit.
pub(super) static FIND_ALIASES: &[&str] = &[
    "search",
    "old",
    "old_string",
    "oldText",
    "old_text",
    "from",
    "target",
    "match",
];

/// Alias keys that resolve to the canonical `replace` parameter (the new text).
///
/// `new_string`/`newText` are the legacy MCP names, kept here as aliases. The
/// remaining entries are natural-language synonyms.
pub(super) static REPLACE_ALIASES: &[&str] = &[
    "new",
    "new_string",
    "newText",
    "new_text",
    "to",
    "with",
    "replacement",
];

pub(super) static EDIT_FILE_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("file_path")
        .description("Absolute path to the file to modify")
        .param_type(ParamType::String)
        .aliases(FILE_PATH_ALIASES)
        .required(),
    ParamMeta::new("find")
        .description("Exact text to replace")
        .param_type(ParamType::String)
        .aliases(FIND_ALIASES)
        .required(),
    ParamMeta::new("replace")
        .description("Replacement text")
        .param_type(ParamType::String)
        .aliases(REPLACE_ALIASES)
        .required(),
    ParamMeta::new("replace_all")
        .description("Replace all occurrences (default: false)")
        .param_type(ParamType::Boolean),
    ParamMeta::new("occurrence")
        .description(
            "1-based candidate index to disambiguate when `find` has multiple confident \
             matches and `replace_all` is false. Omit it and an ambiguous `find` returns \
             the candidate list (line numbers + current text + context) instead of editing; \
             supply it to apply exactly that candidate.",
        )
        .param_type(ParamType::Integer),
    ParamMeta::new("edits")
        .description("Array of {find, replace} edit pairs to apply sequentially")
        .param_type(ParamType::Array),
];

/// One canonical edit: replace `find` with `replace`, optionally every occurrence.
///
/// This is the normalized form every accepted input shape collapses to. It
/// carries no IO — [`normalize_edit_args`] produces it purely from arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditPair {
    /// Exact text to match in the target file.
    pub find: String,
    /// Replacement text.
    pub replace: String,
    /// Replace every occurrence (`true`) instead of just the first (`false`).
    pub replace_all: bool,
    /// 1-based candidate index that disambiguates an otherwise-ambiguous `find`.
    ///
    /// `None` (the default) means "no hint": an ambiguous `find` returns the
    /// candidate listing instead of editing. When supplied and it selects exactly
    /// one of the surfaced candidates, that candidate is applied. Ignored when the
    /// `find` is unambiguous.
    pub occurrence: Option<usize>,
}

/// Read the first present key among `keys` from `map`.
pub(super) fn first_present<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    canonical: &str,
    aliases: &[&str],
) -> Option<&'a serde_json::Value> {
    if let Some(v) = map.get(canonical) {
        return Some(v);
    }
    aliases.iter().find_map(|alias| map.get(*alias))
}

/// Coerce a JSON value into a list of strings: a scalar string yields one entry,
/// an array yields each element as a string. Returns `None` for absent input and
/// an error for a non-string / non-array value (or a non-string array element).
fn collect_strings(value: Option<&serde_json::Value>) -> Result<Option<Vec<String>>, McpError> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::String(s) => Ok(Some(vec![s.clone()])),
        serde_json::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    serde_json::Value::String(s) => out.push(s.clone()),
                    other => {
                        return Err(McpError::invalid_request(
                            format!("find/replace array entries must be strings, got {other}"),
                            None,
                        ))
                    }
                }
            }
            Ok(Some(out))
        }
        other => Err(McpError::invalid_request(
            format!("find/replace must be a string or array of strings, got {other}"),
            None,
        )),
    }
}

/// Read an optional `replace_all` boolean from a map (canonical name only —
/// there are no aliases for this flag).
fn read_replace_all(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.get("replace_all")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// Read an optional 1-based `occurrence` hint from a map (canonical name only).
///
/// A value `>= 1` is kept; `0`, a negative, or a non-integer is treated as
/// absent (`None`) so a malformed hint never silently selects the wrong
/// candidate — it simply falls back to the candidate listing.
fn read_occurrence(map: &serde_json::Map<String, serde_json::Value>) -> Option<usize> {
    map.get("occurrence")
        .and_then(serde_json::Value::as_u64)
        .filter(|&n| n >= 1)
        .map(|n| n as usize)
}

/// Pair a list of finds with a list of replaces using the forgiving rules:
/// - N finds + N replaces → zip.
/// - N finds + 1 replace → broadcast the single replace to every find.
/// - anything else (including 1 find + N replaces) → zip what lines up cleanly
///   and surface the unpaired remainder in the error; never silently drop.
fn pair_finds_replaces(
    finds: Vec<String>,
    replaces: Vec<String>,
    replace_all: bool,
    occurrence: Option<usize>,
) -> Result<Vec<EditPair>, McpError> {
    // Broadcast a single replace across many finds (the delete-many shape).
    if replaces.len() == 1 && finds.len() > 1 {
        let replace = &replaces[0];
        return Ok(finds
            .into_iter()
            .map(|find| EditPair {
                find,
                replace: replace.clone(),
                replace_all,
                occurrence,
            })
            .collect());
    }

    if finds.len() == replaces.len() {
        return Ok(finds
            .into_iter()
            .zip(replaces)
            .map(|(find, replace)| EditPair {
                find,
                replace,
                replace_all,
                occurrence,
            })
            .collect());
    }

    // Mismatch: pair what zips, then report the unpaired remainder.
    let paired = finds.len().min(replaces.len());
    let leftover_finds = &finds[paired..];
    let leftover_replaces = &replaces[paired..];
    let mut remainder = Vec::new();
    if !leftover_finds.is_empty() {
        remainder.push(format!("unpaired finds: {leftover_finds:?}"));
    }
    if !leftover_replaces.is_empty() {
        remainder.push(format!("unpaired replaces: {leftover_replaces:?}"));
    }
    Err(McpError::invalid_request(
        format!(
            "mismatched find/replace counts ({} finds, {} replaces); {}",
            finds.len(),
            replaces.len(),
            remainder.join("; ")
        ),
        None,
    ))
}

/// Whether a no-`op` argument map should be dispatched to the edit operation.
///
/// True when any find-ish or replace-ish key (canonical name or alias) is
/// present, or when an `edits` array is supplied. The dispatcher in
/// [`FilesTool`](crate::mcp::tools::files::FilesTool) consults this BEFORE the `content`→write branch so a
/// canonical `{find, replace}` call is never misrouted to write.
pub fn looks_like_edit(args: &serde_json::Map<String, serde_json::Value>) -> bool {
    args.contains_key("edits")
        || first_present(args, "find", FIND_ALIASES).is_some()
        || first_present(args, "replace", REPLACE_ALIASES).is_some()
}

/// Normalize the forgiving `edit files` argument surface into a canonical list
/// of [`EditPair`]s.
///
/// Accepts three input shapes — which may be combined — under any of the
/// `find`/`replace` aliases (see [`FIND_ALIASES`] / [`REPLACE_ALIASES`]):
///
/// 1. Top-level scalar `find`/`replace`.
/// 2. Top-level parallel arrays `find: [...]` / `replace: [...]`.
/// 3. An `edits: [{ find, replace, replace_all? }, ...]` array.
///
/// Top-level finds/replaces are paired via [`pair_finds_replaces`] (zip /
/// broadcast / mismatch-remainder) and then **concatenated** with the pairs
/// drawn from `edits[]`. This is pure: it performs no IO and never touches the
/// filesystem, so it is unit-testable in isolation.
pub fn normalize_edit_args(
    args: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<EditPair>, McpError> {
    let mut pairs = Vec::new();

    // Shape 1 & 2: top-level scalar or parallel arrays.
    let finds = collect_strings(first_present(args, "find", FIND_ALIASES))?;
    let replaces = collect_strings(first_present(args, "replace", REPLACE_ALIASES))?;
    match (finds, replaces) {
        (Some(finds), Some(replaces)) => {
            pairs.extend(pair_finds_replaces(
                finds,
                replaces,
                read_replace_all(args),
                read_occurrence(args),
            )?);
        }
        (Some(_), None) => {
            return Err(McpError::invalid_request(
                "find provided without a matching replace".to_string(),
                None,
            ));
        }
        (None, Some(_)) => {
            return Err(McpError::invalid_request(
                "replace provided without a matching find".to_string(),
                None,
            ));
        }
        (None, None) => {}
    }

    // Shape 3: the edits[] array, each entry carrying its own find/replace.
    if let Some(edits) = args.get("edits") {
        let items = edits.as_array().ok_or_else(|| {
            McpError::invalid_request("edits must be an array of edit objects".to_string(), None)
        })?;
        for (idx, item) in items.iter().enumerate() {
            let obj = item.as_object().ok_or_else(|| {
                McpError::invalid_request(
                    format!("edits[{idx}] must be an object with find/replace"),
                    None,
                )
            })?;
            let finds =
                collect_strings(first_present(obj, "find", FIND_ALIASES))?.ok_or_else(|| {
                    McpError::invalid_request(format!("edits[{idx}] is missing find"), None)
                })?;
            let replaces = collect_strings(first_present(obj, "replace", REPLACE_ALIASES))?
                .ok_or_else(|| {
                    McpError::invalid_request(format!("edits[{idx}] is missing replace"), None)
                })?;
            pairs.extend(pair_finds_replaces(
                finds,
                replaces,
                read_replace_all(obj),
                read_occurrence(obj),
            )?);
        }
    }

    if pairs.is_empty() {
        return Err(McpError::invalid_request(
            "no edits provided: supply find/replace (or aliases), or an edits array".to_string(),
            None,
        ));
    }

    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // normalize_edit_args — pure argument shaping (no IO)
    // =========================================================================

    /// Build a JSON arg map from a serde_json::json! object literal.
    fn args(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().expect("object literal").clone()
    }

    fn pair(find: &str, replace: &str, replace_all: bool) -> EditPair {
        EditPair {
            find: find.to_string(),
            replace: replace.to_string(),
            replace_all,
            occurrence: None,
        }
    }

    #[test]
    fn normalize_canonical_scalar_find_replace() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "file_path": "/x", "find": "a", "replace": "b"
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false)]);
    }

    #[test]
    fn normalize_legacy_old_new_string_resolves_same_as_find_replace() {
        let canonical = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let legacy = normalize_edit_args(&args(serde_json::json!({
            "old_string": "a", "new_string": "b"
        })))
        .unwrap();
        assert_eq!(legacy, canonical);
    }

    #[test]
    fn normalize_legacy_oldtext_newtext_resolves_same_as_find_replace() {
        let canonical = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let legacy = normalize_edit_args(&args(serde_json::json!({
            "oldText": "a", "newText": "b"
        })))
        .unwrap();
        assert_eq!(legacy, canonical);
    }

    #[test]
    fn normalize_search_with_alias_pair() {
        // edits[] entries using {search, with} aliases.
        let got = normalize_edit_args(&args(serde_json::json!({
            "edits": [{ "search": "a", "with": "b" }, { "search": "c", "with": "d" }]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_scalar_array_and_edits_yield_same_pairs() {
        let scalar = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b"
        })))
        .unwrap();
        let arrays = normalize_edit_args(&args(serde_json::json!({
            "find": ["a"], "replace": ["b"]
        })))
        .unwrap();
        let edits = normalize_edit_args(&args(serde_json::json!({
            "edits": [{ "find": "a", "replace": "b" }]
        })))
        .unwrap();
        assert_eq!(scalar, vec![pair("a", "b", false)]);
        assert_eq!(arrays, scalar);
        assert_eq!(edits, scalar);
    }

    #[test]
    fn normalize_parallel_arrays_zip() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "c"], "replace": ["b", "d"]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_broadcast_single_replace_to_many_finds() {
        // Delete-many: many finds + one empty replace.
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b", "c"], "replace": [""]
        })))
        .unwrap();
        assert_eq!(
            got,
            vec![
                pair("a", "", false),
                pair("b", "", false),
                pair("c", "", false)
            ]
        );
    }

    #[test]
    fn normalize_broadcast_scalar_replace_to_array_finds() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b"], "replace": "X"
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "X", false), pair("b", "X", false)]);
    }

    #[test]
    fn normalize_toplevel_and_edits_concatenate() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b",
            "edits": [{ "find": "c", "replace": "d" }]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", false), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_replace_all_scalar_applies_to_toplevel_pair() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "find": "a", "replace": "b", "replace_all": true
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", true)]);
    }

    #[test]
    fn normalize_replace_all_per_edit_entry() {
        let got = normalize_edit_args(&args(serde_json::json!({
            "edits": [
                { "find": "a", "replace": "b", "replace_all": true },
                { "find": "c", "replace": "d" }
            ]
        })))
        .unwrap();
        assert_eq!(got, vec![pair("a", "b", true), pair("c", "d", false)]);
    }

    #[test]
    fn normalize_mismatched_array_lengths_errors_with_remainder() {
        // 3 finds, 2 replaces (not a broadcast): zip the first 2, surface the
        // unpaired remainder in the error — never silently drop.
        let err = normalize_edit_args(&args(serde_json::json!({
            "find": ["a", "b", "c"], "replace": ["x", "y"]
        })))
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains('c'),
            "error must name the unpaired find: {msg}"
        );
    }

    #[test]
    fn normalize_one_find_many_replaces_errors_with_remainder() {
        let err = normalize_edit_args(&args(serde_json::json!({
            "find": ["a"], "replace": ["x", "y"]
        })))
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains('y'),
            "error must name the unpaired replace: {msg}"
        );
    }

    #[test]
    fn normalize_no_find_or_replace_or_edits_errors() {
        // A path alone is not an edit: the file is named but nothing says what
        // to change, so the error must say the edits are missing rather than
        // complain about the path.
        let err = normalize_edit_args(&args(serde_json::json!({ "file_path": "/x" }))).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("no edits provided"),
            "a call carrying only a path must report the missing edits: {msg}"
        );
    }

    // =====================================================================
    // Pure-function argument normalization error arms
    // =====================================================================

    /// `collect_strings` rejects a non-string array element, naming the offender.
    #[test]
    fn test_collect_strings_rejects_non_string_array_element() {
        let value = serde_json::json!(["ok", 42]);
        let err = collect_strings(Some(&value)).unwrap_err();
        assert!(format!("{err:?}").contains("array entries must be strings"));
    }

    /// `collect_strings` rejects a value that is neither string nor array.
    #[test]
    fn test_collect_strings_rejects_non_string_non_array() {
        let value = serde_json::json!({ "not": "a string" });
        let err = collect_strings(Some(&value)).unwrap_err();
        assert!(format!("{err:?}").contains("string or array of strings"));
    }

    /// `collect_strings` returns `None` for absent input and a one-element vec for
    /// a scalar string.
    #[test]
    fn test_collect_strings_absent_and_scalar() {
        assert!(collect_strings(None).unwrap().is_none());
        let scalar = serde_json::json!("hello");
        assert_eq!(
            collect_strings(Some(&scalar)).unwrap().unwrap(),
            vec!["hello".to_string()]
        );
    }

    /// A top-level `replace` with no matching `find` is rejected.
    #[test]
    fn test_normalize_replace_without_find() {
        let mut args = serde_json::Map::new();
        args.insert("replace".to_string(), serde_json::json!("x"));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("replace provided without a matching find"));
    }

    /// A top-level `find` with no matching `replace` is rejected.
    #[test]
    fn test_normalize_find_without_replace() {
        let mut args = serde_json::Map::new();
        args.insert("find".to_string(), serde_json::json!("x"));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("find provided without a matching replace"));
    }

    /// `edits` that is not an array is rejected.
    #[test]
    fn test_normalize_edits_not_an_array() {
        let mut args = serde_json::Map::new();
        args.insert("edits".to_string(), serde_json::json!("not an array"));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("edits must be an array"));
    }

    /// An `edits[]` entry that is not an object is rejected, naming the index.
    #[test]
    fn test_normalize_edits_entry_not_an_object() {
        let mut args = serde_json::Map::new();
        args.insert("edits".to_string(), serde_json::json!(["scalar"]));
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("edits[0] must be an object"));
    }

    /// An `edits[]` entry missing `find` (or `replace`) is rejected, naming it.
    #[test]
    fn test_normalize_edits_entry_missing_find_and_replace() {
        let mut missing_find = serde_json::Map::new();
        missing_find.insert("edits".to_string(), serde_json::json!([{ "replace": "x" }]));
        let err = normalize_edit_args(&missing_find).unwrap_err();
        assert!(format!("{err:?}").contains("edits[0] is missing find"));

        let mut missing_replace = serde_json::Map::new();
        missing_replace.insert("edits".to_string(), serde_json::json!([{ "find": "x" }]));
        let err = normalize_edit_args(&missing_replace).unwrap_err();
        assert!(format!("{err:?}").contains("edits[0] is missing replace"));
    }

    /// A mismatched find/replace count (2 finds, 1 replace is broadcast, but
    /// 2 finds + 3 replaces cannot pair) surfaces the unpaired remainder.
    #[test]
    fn test_pair_finds_replaces_mismatch_reports_remainder() {
        let finds = vec!["a".to_string(), "b".to_string()];
        let replaces = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let err = pair_finds_replaces(finds, replaces, false, None).unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("mismatched find/replace counts"));
        assert!(msg.contains("unpaired replaces"));
    }

    /// An empty arg map (no find/replace/edits) reports "no edits provided".
    #[test]
    fn test_normalize_no_edits_provided() {
        let args = serde_json::Map::new();
        let err = normalize_edit_args(&args).unwrap_err();
        assert!(format!("{err:?}").contains("no edits provided"));
    }
}
