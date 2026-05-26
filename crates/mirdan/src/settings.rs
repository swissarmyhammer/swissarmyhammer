//! Generic JSON settings-file primitives shared by install components.
//!
//! These primitives are the building blocks for any install component that
//! reads, mutates, and writes a JSON configuration file (e.g. Claude Code's
//! `~/.claude/settings.json`, `~/.claude.json`, `.mcp.json`). They are
//! intentionally agent-agnostic: the caller supplies the JSON pointer/key
//! and the desired value, the primitive applies the change idempotently.
//!
//! All mutating primitives return `bool` — `true` if the in-memory value
//! changed, `false` if the operation was a no-op (already in the desired
//! state). Callers use this flag to decide whether to write the file back
//! and to emit reporter events.

use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::registry::RegistryError;

/// Read a JSON settings file, returning an empty object if the file does
/// not exist or is empty.
///
/// Returns a `RegistryError::Io` on I/O failure or `RegistryError::Json`
/// when the file exists but contains invalid JSON.
pub fn read_json(path: &Path) -> Result<Value, RegistryError> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let content = fs::read_to_string(path)?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(trimmed).map_err(|e| {
        RegistryError::Validation(format!("Invalid JSON in {}: {}", path.display(), e))
    })
}

/// Write a JSON value to a settings file with pretty-printing.
///
/// Creates parent directories if they do not exist. Output is terminated
/// with a trailing newline to match the established mirdan convention.
///
/// Returns a `RegistryError::Io` on I/O failure or `RegistryError::Validation`
/// when the value cannot be serialized.
pub fn write_json(path: &Path, value: &Value) -> Result<(), RegistryError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| RegistryError::Validation(format!("Failed to serialize JSON: {}", e)))?;
    fs::write(path, format!("{}\n", json))?;
    Ok(())
}

/// Ensure that the JSON array at `pointer` contains `value`, creating any
/// missing object parents along the way.
///
/// `pointer` follows RFC 6901 JSON pointer syntax (e.g. `"/permissions/deny"`).
/// Each segment that does not yet exist is created as an empty object,
/// except the final segment, which is created as an empty array if missing.
///
/// Returns `true` when the array was modified (either `value` was appended
/// or a parent object/array had to be created), `false` when `value` was
/// already present in the array at `pointer`.
///
/// Panics if `root` is not a JSON object or if `pointer` traverses a
/// non-object intermediate value. Callers should pass a fresh object from
/// [`read_json`] or a known-object subtree.
pub fn ensure_array_contains(root: &mut Value, pointer: &str, value: &Value) -> bool {
    let array = ensure_array_at(root, pointer);
    if array.iter().any(|v| v == value) {
        return false;
    }
    array.push(value.clone());
    true
}

/// Remove every occurrence of `value` from the array at `pointer`.
///
/// `pointer` follows RFC 6901 JSON pointer syntax. If the path does not
/// resolve to an array, the operation is a no-op.
///
/// Returns `true` when at least one element was removed, `false` otherwise.
pub fn remove_from_array(root: &mut Value, pointer: &str, value: &Value) -> bool {
    let array = match root.pointer_mut(pointer).and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => return false,
    };
    let before = array.len();
    array.retain(|v| v != value);
    array.len() != before
}

/// Set `root[key]` to `value`, returning `true` if it differs from the
/// current value at that key.
///
/// `root` must be a JSON object. If `root[key]` already equals `value`,
/// the operation is a no-op and returns `false`. Otherwise the key is
/// inserted or overwritten and `true` is returned.
///
/// Panics if `root` is not a JSON object.
pub fn set_object(root: &mut Value, key: &str, value: Value) -> bool {
    let obj = root
        .as_object_mut()
        .expect("set_object requires root to be a JSON object");
    if obj.get(key) == Some(&value) {
        return false;
    }
    obj.insert(key.to_string(), value);
    true
}

/// Remove `key` from `root`, returning `true` if it was present.
///
/// If `root` is not an object or `key` is absent, the operation is a no-op
/// and returns `false`.
pub fn remove_key(root: &mut Value, key: &str) -> bool {
    root.as_object_mut()
        .map(|obj| obj.remove(key).is_some())
        .unwrap_or(false)
}

/// Resolve `pointer` against `root`, creating intermediate objects and a
/// trailing empty array as needed, and return a mutable reference to the
/// array.
///
/// The pointer must be in RFC 6901 syntax (each segment prefixed with `/`)
/// and the path must terminate at an array slot.
fn ensure_array_at<'a>(root: &'a mut Value, pointer: &str) -> &'a mut Vec<Value> {
    let segments: Vec<&str> = pointer
        .strip_prefix('/')
        .unwrap_or(pointer)
        .split('/')
        .collect();
    assert!(
        !segments.is_empty() && !segments.iter().any(|s| s.is_empty()),
        "ensure_array_contains pointer must be non-empty and have no empty segments: {:?}",
        pointer
    );

    let (last, parents) = segments.split_last().expect("non-empty segments");

    let mut current = root;
    for seg in parents {
        let obj = current
            .as_object_mut()
            .expect("ensure_array_contains traversed a non-object value");
        if !obj.contains_key(*seg) {
            obj.insert(seg.to_string(), json!({}));
        }
        current = obj.get_mut(*seg).expect("just inserted");
    }

    let obj = current
        .as_object_mut()
        .expect("ensure_array_contains parent must be an object");
    if !obj.contains_key(*last) {
        obj.insert(last.to_string(), json!([]));
    }
    obj.get_mut(*last)
        .expect("just ensured")
        .as_array_mut()
        .expect("ensure_array_contains terminal must be an array")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn read_json_returns_empty_object_when_file_missing() {
        let path = PathBuf::from("/tmp/sah-mirdan-settings-nonexistent.json");
        let value = read_json(&path).unwrap();
        assert_eq!(value, json!({}));
    }

    #[test]
    fn read_json_returns_empty_object_for_blank_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.json");
        fs::write(&path, "   \n").unwrap();
        let value = read_json(&path).unwrap();
        assert_eq!(value, json!({}));
    }

    #[test]
    fn read_json_roundtrips_existing_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"a": 1, "b": [2, 3]}"#).unwrap();
        let value = read_json(&path).unwrap();
        assert_eq!(value, json!({"a": 1, "b": [2, 3]}));
    }

    #[test]
    fn read_json_returns_validation_error_for_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        fs::write(&path, "not json").unwrap();
        let err = read_json(&path).unwrap_err();
        assert!(matches!(err, RegistryError::Validation(_)));
    }

    #[test]
    fn write_json_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/deeper/settings.json");
        write_json(&path, &json!({"k": "v"})).unwrap();
        assert!(path.exists());
        let back = read_json(&path).unwrap();
        assert_eq!(back, json!({"k": "v"}));
    }

    #[test]
    fn write_json_roundtrips_through_read_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = json!({
            "permissions": { "deny": ["Bash"] },
            "statusLine": { "type": "command", "command": "sah statusline" }
        });
        write_json(&path, &original).unwrap();
        assert_eq!(read_json(&path).unwrap(), original);
    }

    #[test]
    fn ensure_array_contains_creates_missing_parents_and_appends() {
        let mut root = json!({});
        let changed = ensure_array_contains(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(changed);
        assert_eq!(root, json!({"permissions": {"deny": ["Bash"]}}));
    }

    #[test]
    fn ensure_array_contains_is_idempotent() {
        let mut root = json!({"permissions": {"deny": ["Bash"]}});
        let changed = ensure_array_contains(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(!changed);
        assert_eq!(root, json!({"permissions": {"deny": ["Bash"]}}));
    }

    #[test]
    fn ensure_array_contains_preserves_existing_array_entries() {
        let mut root = json!({"permissions": {"deny": ["Read"]}});
        let changed = ensure_array_contains(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(changed);
        assert_eq!(root, json!({"permissions": {"deny": ["Read", "Bash"]}}));
    }

    #[test]
    fn ensure_array_contains_preserves_sibling_keys() {
        let mut root = json!({
            "permissions": { "allow": ["Read"] },
            "other": 42
        });
        let changed = ensure_array_contains(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(changed);
        assert_eq!(root["permissions"]["allow"], json!(["Read"]));
        assert_eq!(root["permissions"]["deny"], json!(["Bash"]));
        assert_eq!(root["other"], json!(42));
    }

    #[test]
    fn remove_from_array_returns_true_when_present() {
        let mut root = json!({"permissions": {"deny": ["Bash", "Read"]}});
        let removed = remove_from_array(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(removed);
        assert_eq!(root, json!({"permissions": {"deny": ["Read"]}}));
    }

    #[test]
    fn remove_from_array_returns_false_when_absent() {
        let mut root = json!({"permissions": {"deny": ["Read"]}});
        let removed = remove_from_array(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(!removed);
        assert_eq!(root, json!({"permissions": {"deny": ["Read"]}}));
    }

    #[test]
    fn remove_from_array_returns_false_when_path_missing() {
        let mut root = json!({});
        let removed = remove_from_array(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(!removed);
        assert_eq!(root, json!({}));
    }

    #[test]
    fn remove_from_array_returns_false_when_path_is_not_array() {
        let mut root = json!({"permissions": {"deny": "not-an-array"}});
        let removed = remove_from_array(&mut root, "/permissions/deny", &json!("Bash"));
        assert!(!removed);
    }

    #[test]
    fn set_object_inserts_missing_key() {
        let mut root = json!({});
        let changed = set_object(&mut root, "statusLine", json!({"type": "command"}));
        assert!(changed);
        assert_eq!(root, json!({"statusLine": {"type": "command"}}));
    }

    #[test]
    fn set_object_overwrites_differing_value() {
        let mut root = json!({"statusLine": {"type": "other"}});
        let changed = set_object(&mut root, "statusLine", json!({"type": "command"}));
        assert!(changed);
        assert_eq!(root, json!({"statusLine": {"type": "command"}}));
    }

    #[test]
    fn set_object_is_noop_for_equal_value() {
        let mut root = json!({"statusLine": {"type": "command"}});
        let changed = set_object(&mut root, "statusLine", json!({"type": "command"}));
        assert!(!changed);
    }

    #[test]
    fn set_object_preserves_sibling_keys() {
        let mut root = json!({"permissions": {"deny": ["Bash"]}});
        set_object(&mut root, "statusLine", json!({"type": "command"}));
        assert_eq!(root["permissions"]["deny"], json!(["Bash"]));
        assert_eq!(root["statusLine"], json!({"type": "command"}));
    }

    #[test]
    fn remove_key_returns_true_when_present() {
        let mut root = json!({"statusLine": {"type": "command"}, "keep": 1});
        let removed = remove_key(&mut root, "statusLine");
        assert!(removed);
        assert_eq!(root, json!({"keep": 1}));
    }

    #[test]
    fn remove_key_returns_false_when_absent() {
        let mut root = json!({"keep": 1});
        let removed = remove_key(&mut root, "statusLine");
        assert!(!removed);
        assert_eq!(root, json!({"keep": 1}));
    }

    #[test]
    fn remove_key_returns_false_when_root_is_not_object() {
        let mut root = json!([1, 2, 3]);
        let removed = remove_key(&mut root, "statusLine");
        assert!(!removed);
    }
}
