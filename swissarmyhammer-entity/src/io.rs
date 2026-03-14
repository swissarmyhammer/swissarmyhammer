//! Generic entity I/O — reads and writes entities as YAML or frontmatter+body.
//!
//! The format is determined by the EntityDef:
//! - If `body_field` is `Some(field_name)`, the entity is stored as a `.md` file
//!   with YAML frontmatter and the body field as markdown content.
//! - If `body_field` is `None`, the entity is stored as a `.yaml` file.
//!
//! The entity's `id` comes from the filename, not the file contents.
//! Writes are atomic (temp file + rename).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use swissarmyhammer_fields::EntityDef;
use tokio::fs;
use tracing::warn;
use ulid::Ulid;

use crate::entity::Entity;
use crate::error::{EntityError, Result};

/// Get the file extension for an entity type.
pub fn entity_extension(entity_def: &EntityDef) -> &'static str {
    if entity_def.body_field.is_some() {
        "md"
    } else {
        "yaml"
    }
}

/// Get the file path for an entity.
///
/// The id is sanitized to prevent path traversal — slashes, backslashes,
/// null bytes, and `..` components are rejected.
pub fn entity_file_path(dir: &Path, id: impl AsRef<str>, entity_def: &EntityDef) -> PathBuf {
    let safe_id = sanitize_id(id.as_ref());
    dir.join(format!("{}.{}", safe_id, entity_extension(entity_def)))
}

/// Sanitize an entity ID to prevent path traversal.
///
/// Strips path separators and null bytes; rejects `..` entirely.
fn sanitize_id(id: &str) -> String {
    if id == ".." || id == "." {
        return String::from("_invalid_");
    }
    id.chars()
        .filter(|c| *c != '/' && *c != '\\' && *c != '\0')
        .collect()
}

/// Read a single entity from a file.
///
/// The entity_type and id are provided externally (from directory and filename).
/// The EntityDef determines the file format.
pub async fn read_entity(
    path: &Path,
    entity_type: impl AsRef<str>,
    id: impl AsRef<str>,
    entity_def: &EntityDef,
) -> Result<Entity> {
    let entity_type = entity_type.as_ref();
    let id = id.as_ref();
    let content = fs::read_to_string(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            EntityError::NotFound {
                entity_type: entity_type.to_string(),
                id: id.to_string(),
            }
        } else {
            EntityError::Io(e)
        }
    })?;

    if let Some(ref body_field) = entity_def.body_field {
        parse_frontmatter_body(&content, entity_type, id, body_field, path)
    } else {
        parse_plain_yaml(&content, entity_type, id, path)
    }
}

/// Write an entity to a file.
///
/// Uses atomic write (ULID-named temp file + rename) for safety.
/// Each write gets a unique temp filename, so concurrent writes
/// to the same entity won't collide. The temp file is cleaned up
/// if the rename step fails.
pub async fn write_entity(path: &Path, entity: &Entity, entity_def: &EntityDef) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let content = if let Some(ref body_field) = entity_def.body_field {
        format_frontmatter_body(entity, body_field)?
    } else {
        format_plain_yaml(entity)?
    };

    // Use a ULID-based temp filename to avoid collisions with concurrent writers.
    // The temp file lives in the same directory as the target for atomic rename.
    let temp_path = path
        .parent()
        .expect("entity path must have a parent directory")
        .join(format!(".tmp_{}", Ulid::new()));
    fs::write(&temp_path, content.as_bytes()).await?;

    // If rename fails, clean up the temp file before propagating the error.
    if let Err(e) = fs::rename(&temp_path, path).await {
        let _ = fs::remove_file(&temp_path).await;
        return Err(e.into());
    }

    Ok(())
}

/// Read all entities from a directory.
///
/// Scans for files matching the expected extension and parses each one.
/// Parse errors (invalid YAML, bad frontmatter) are logged and skipped.
/// I/O errors (permission denied, disk failure) are propagated.
pub async fn read_entity_dir(
    dir: &Path,
    entity_type: impl AsRef<str>,
    entity_def: &EntityDef,
) -> Result<Vec<Entity>> {
    let entity_type = entity_type.as_ref();
    let ext = entity_extension(entity_def);
    let mut entities = Vec::new();

    let mut entries = match fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(EntityError::Io(e)),
    };

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            continue;
        }
        let id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };
        match read_entity(&path, entity_type, &id, entity_def).await {
            Ok(entity) => entities.push(entity),
            // File deleted between readdir and read — benign race condition
            Err(EntityError::NotFound { .. }) => continue,
            // Parse errors — warn and skip
            Err(e @ (EntityError::InvalidFrontmatter { .. } | EntityError::Yaml { .. })) => {
                warn!(path = %path.display(), error = %e, "skipping unparseable entity file");
                continue;
            }
            // I/O and other errors — propagate
            Err(e) => return Err(e),
        }
    }

    Ok(entities)
}

/// Move an entity's data file and changelog to a trash directory.
///
/// Moves both the data file (.yaml/.md) and the changelog (.jsonl)
/// to the corresponding trash directory, preserving the full history.
/// Creates the trash directory if it doesn't exist.
/// Silently succeeds if source files don't exist.
pub async fn trash_entity_files(path: &Path, trash_dir: &Path) -> Result<()> {
    fs::create_dir_all(trash_dir).await?;

    // Move data file (try-rename, ignore NotFound to avoid TOCTOU race)
    {
        let filename = path.file_name().expect("entity path must have a filename");
        let dest = trash_dir.join(filename);
        match fs::rename(path, &dest).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    // Move changelog (try-rename, ignore NotFound to avoid TOCTOU race)
    let log_path = path.with_extension("jsonl");
    {
        let log_filename = log_path
            .file_name()
            .expect("changelog path must have a filename");
        let log_dest = trash_dir.join(log_filename);
        match fs::rename(&log_path, &log_dest).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

/// Restore an entity's data file and changelog from a trash directory back to live storage.
///
/// Inverse of [`trash_entity_files`]. Moves both the data file (.yaml/.md) and
/// the changelog (.jsonl) from the trash directory back to the original location.
/// Creates the destination directory if it doesn't exist.
/// Returns an error if the source files are not found in trash.
pub async fn restore_entity_files(path: &Path, trash_dir: &Path) -> Result<()> {
    // Ensure the destination directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Move data file back from trash — error if missing (nothing to restore)
    {
        let filename = path.file_name().expect("entity path must have a filename");
        let src = trash_dir.join(filename);
        match fs::rename(&src, path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(EntityError::RestoreFromTrashFailed { path: src });
            }
            Err(e) => return Err(e.into()),
        }
    }

    // Move changelog back from trash
    let log_path = path.with_extension("jsonl");
    {
        let log_filename = log_path
            .file_name()
            .expect("changelog path must have a filename");
        let log_src = trash_dir.join(log_filename);
        match fs::rename(&log_src, &log_path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

// --- Internal helpers ---

/// Parse a frontmatter+body file into an Entity.
fn parse_frontmatter_body(
    content: &str,
    entity_type: impl AsRef<str>,
    id: impl AsRef<str>,
    body_field: impl AsRef<str>,
    path: &Path,
) -> Result<Entity> {
    let entity_type = entity_type.as_ref();
    let id = id.as_ref();
    let body_field = body_field.as_ref();
    // Split on --- delimiters: ["", frontmatter, body]
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err(EntityError::InvalidFrontmatter {
            path: path.to_path_buf(),
        });
    }

    let frontmatter = parts[1].trim();
    let body = parts[2].strip_prefix('\n').unwrap_or(parts[2]);

    let yaml_map: HashMap<String, Value> =
        serde_yaml_ng::from_str(frontmatter).map_err(|e| EntityError::Yaml {
            path: path.to_path_buf(),
            source: e,
        })?;

    let mut entity = Entity::new(entity_type, id);
    for (k, v) in yaml_map {
        flatten_into(&mut entity, &k, v);
    }
    // Body field comes from the markdown body, not the frontmatter
    entity.set(body_field, Value::String(body.to_string()));

    Ok(entity)
}

/// Flatten nested objects into underscore-separated keys.
///
/// If `key` maps to a JSON object, each sub-key is expanded to `key_subkey`.
/// Non-object values are inserted as-is. Only one level of nesting is flattened.
fn flatten_into(entity: &mut Entity, key: &str, value: Value) {
    if let Value::Object(map) = &value {
        for (sub_key, sub_value) in map {
            let flat_key = format!("{}_{}", key, sub_key);
            entity.set(flat_key, sub_value.clone());
        }
    } else {
        entity.set(key, value);
    }
}

/// Parse a plain YAML file into an Entity.
fn parse_plain_yaml(
    content: &str,
    entity_type: impl AsRef<str>,
    id: impl AsRef<str>,
    path: &Path,
) -> Result<Entity> {
    let entity_type = entity_type.as_ref();
    let id = id.as_ref();
    let yaml_map: HashMap<String, Value> =
        serde_yaml_ng::from_str(content).map_err(|e| EntityError::Yaml {
            path: path.to_path_buf(),
            source: e,
        })?;

    let mut entity = Entity::new(entity_type, id);
    for (k, v) in yaml_map {
        flatten_into(&mut entity, &k, v);
    }

    Ok(entity)
}

/// Format an entity as frontmatter + markdown body.
fn format_frontmatter_body(entity: &Entity, body_field: impl AsRef<str>) -> Result<String> {
    let body_field = body_field.as_ref();
    let body = entity.get_str(body_field).unwrap_or("").to_string();

    // Build frontmatter from all fields except the body field
    let mut frontmatter_map = serde_json::Map::new();
    for (k, v) in &entity.fields {
        if k != body_field {
            frontmatter_map.insert(k.clone(), v.clone());
        }
    }

    let frontmatter_value = Value::Object(frontmatter_map);
    let frontmatter_yaml =
        serde_yaml_ng::to_string(&frontmatter_value).map_err(|e| EntityError::Yaml {
            path: PathBuf::from("<serialization>"),
            source: e,
        })?;

    Ok(format!("---\n{}---\n{}", frontmatter_yaml, body))
}

/// Format an entity as plain YAML.
fn format_plain_yaml(entity: &Entity) -> Result<String> {
    let map_value = Value::Object(
        entity
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    );
    serde_yaml_ng::to_string(&map_value).map_err(|e| EntityError::Yaml {
        path: PathBuf::from("<serialization>"),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_entity_def() -> EntityDef {
        EntityDef {
            name: "task".into(),
            body_field: Some("body".into()),
            fields: vec!["title".into(), "body".into()],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            search_display_field: None,
        }
    }

    fn tag_entity_def() -> EntityDef {
        EntityDef {
            name: "tag".into(),
            body_field: None,
            fields: vec!["tag_name".into(), "color".into()],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            search_display_field: None,
        }
    }

    #[test]
    fn entity_extension_md_for_body_field() {
        assert_eq!(entity_extension(&task_entity_def()), "md");
    }

    #[test]
    fn entity_extension_yaml_for_no_body() {
        assert_eq!(entity_extension(&tag_entity_def()), "yaml");
    }

    #[test]
    fn entity_file_path_builds_correctly() {
        let dir = Path::new("/tmp/tasks");
        let path = entity_file_path(dir, "01ABC", &task_entity_def());
        assert_eq!(path, PathBuf::from("/tmp/tasks/01ABC.md"));

        let path = entity_file_path(dir, "01DEF", &tag_entity_def());
        assert_eq!(path, PathBuf::from("/tmp/tasks/01DEF.yaml"));
    }

    #[test]
    fn entity_file_path_sanitizes_traversal() {
        let dir = Path::new("/tmp/tasks");
        // Slashes stripped, dots remain
        let path = entity_file_path(dir, "../../etc/passwd", &tag_entity_def());
        assert_eq!(path, PathBuf::from("/tmp/tasks/....etcpasswd.yaml"));

        // Backslashes stripped
        let path = entity_file_path(dir, "..\\..\\etc", &tag_entity_def());
        assert_eq!(path, PathBuf::from("/tmp/tasks/....etc.yaml"));

        // Null bytes stripped
        let path = entity_file_path(dir, "test\0id", &tag_entity_def());
        assert_eq!(path, PathBuf::from("/tmp/tasks/testid.yaml"));

        // Bare .. becomes _invalid_
        let path = entity_file_path(dir, "..", &tag_entity_def());
        assert_eq!(path, PathBuf::from("/tmp/tasks/_invalid_.yaml"));

        // Bare . becomes _invalid_
        let path = entity_file_path(dir, ".", &tag_entity_def());
        assert_eq!(path, PathBuf::from("/tmp/tasks/_invalid_.yaml"));
    }

    #[test]
    fn parse_frontmatter_body_round_trip() {
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("My Task".into()));
        entity.set(
            "body",
            Value::String("This is the body.\nWith multiple lines.".into()),
        );

        let content = format_frontmatter_body(&entity, "body").unwrap();

        let parsed =
            parse_frontmatter_body(&content, "task", "01ABC", "body", Path::new("test.md"))
                .unwrap();

        assert_eq!(parsed.entity_type, "task");
        assert_eq!(parsed.id, "01ABC");
        assert_eq!(parsed.get_str("title"), Some("My Task"));
        assert_eq!(
            parsed.get_str("body"),
            Some("This is the body.\nWith multiple lines.")
        );
    }

    #[test]
    fn parse_plain_yaml_round_trip() {
        let mut entity = Entity::new("tag", "bug");
        entity.set("tag_name", Value::String("bug".into()));
        entity.set("color", Value::String("ff0000".into()));

        let content = format_plain_yaml(&entity).unwrap();

        let parsed = parse_plain_yaml(&content, "tag", "bug", Path::new("test.yaml")).unwrap();

        assert_eq!(parsed.entity_type, "tag");
        assert_eq!(parsed.id, "bug");
        assert_eq!(parsed.get_str("tag_name"), Some("bug"));
        assert_eq!(parsed.get_str("color"), Some("ff0000"));
    }

    #[test]
    fn parse_frontmatter_missing_delimiters() {
        let content = "just some text without frontmatter";
        let result = parse_frontmatter_body(content, "task", "01ABC", "body", Path::new("test.md"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("frontmatter"));
    }

    #[test]
    fn format_frontmatter_empty_body() {
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("No body".into()));

        let content = format_frontmatter_body(&entity, "body").unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("title:"));
        // Body should be empty but format should be valid
        let parsed =
            parse_frontmatter_body(&content, "task", "01ABC", "body", Path::new("test.md"))
                .unwrap();
        assert_eq!(parsed.get_str("title"), Some("No body"));
        assert_eq!(parsed.get_str("body"), Some(""));
    }

    #[tokio::test]
    async fn read_write_entity_with_body_field() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = task_entity_def();
        let path = entity_file_path(dir.path(), "01ABC", &entity_def);

        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Test Task".into()));
        entity.set(
            "body",
            Value::String("Task body content.\n\nWith paragraphs.".into()),
        );

        write_entity(&path, &entity, &entity_def).await.unwrap();

        let loaded = read_entity(&path, "task", "01ABC", &entity_def)
            .await
            .unwrap();

        assert_eq!(loaded.entity_type, "task");
        assert_eq!(loaded.id, "01ABC");
        assert_eq!(loaded.get_str("title"), Some("Test Task"));
        assert_eq!(
            loaded.get_str("body"),
            Some("Task body content.\n\nWith paragraphs.")
        );
    }

    #[tokio::test]
    async fn read_write_entity_plain_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();
        let path = entity_file_path(dir.path(), "bug", &entity_def);

        let mut entity = Entity::new("tag", "bug");
        entity.set("tag_name", Value::String("bug".into()));
        entity.set("color", Value::String("ff0000".into()));

        write_entity(&path, &entity, &entity_def).await.unwrap();

        let loaded = read_entity(&path, "tag", "bug", &entity_def).await.unwrap();

        assert_eq!(loaded.entity_type, "tag");
        assert_eq!(loaded.id, "bug");
        assert_eq!(loaded.get_str("tag_name"), Some("bug"));
        assert_eq!(loaded.get_str("color"), Some("ff0000"));
    }

    #[tokio::test]
    async fn read_entity_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();
        let path = entity_file_path(dir.path(), "nonexistent", &entity_def);

        let result = read_entity(&path, "tag", "nonexistent", &entity_def).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn read_entity_dir_reads_all() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();

        for (id, name) in [("bug", "bug"), ("feature", "feature"), ("docs", "docs")] {
            let path = entity_file_path(dir.path(), id, &entity_def);
            let mut entity = Entity::new("tag", id);
            entity.set("tag_name", Value::String(name.into()));
            write_entity(&path, &entity, &entity_def).await.unwrap();
        }

        let entities = read_entity_dir(dir.path(), "tag", &entity_def)
            .await
            .unwrap();

        assert_eq!(entities.len(), 3);
        let names: Vec<&str> = entities
            .iter()
            .filter_map(|e| e.get_str("tag_name"))
            .collect();
        assert!(names.contains(&"bug"));
        assert!(names.contains(&"feature"));
        assert!(names.contains(&"docs"));
    }

    #[tokio::test]
    async fn read_entity_dir_empty() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();

        let entities = read_entity_dir(dir.path(), "tag", &entity_def)
            .await
            .unwrap();
        assert!(entities.is_empty());
    }

    #[tokio::test]
    async fn read_entity_dir_nonexistent() {
        let entity_def = tag_entity_def();
        let entities = read_entity_dir(Path::new("/tmp/nonexistent_dir_12345"), "tag", &entity_def)
            .await
            .unwrap();
        assert!(entities.is_empty());
    }

    #[tokio::test]
    async fn read_entity_dir_skips_wrong_extension() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def(); // expects .yaml

        // Write a .md file (wrong extension for tags)
        fs::write(dir.path().join("stray.md"), "# Not a tag")
            .await
            .unwrap();

        // Write a valid .yaml file
        let path = entity_file_path(dir.path(), "bug", &entity_def);
        let mut entity = Entity::new("tag", "bug");
        entity.set("tag_name", Value::String("bug".into()));
        write_entity(&path, &entity, &entity_def).await.unwrap();

        let entities = read_entity_dir(dir.path(), "tag", &entity_def)
            .await
            .unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].id, "bug");
    }

    #[tokio::test]
    async fn read_entity_dir_skips_parse_errors() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def(); // expects .yaml

        // Write a valid .yaml file
        let path = entity_file_path(dir.path(), "bug", &entity_def);
        let mut entity = Entity::new("tag", "bug");
        entity.set("tag_name", Value::String("bug".into()));
        write_entity(&path, &entity, &entity_def).await.unwrap();

        // Write an unparseable .yaml file
        fs::write(dir.path().join("corrupt.yaml"), "{{{{not valid yaml")
            .await
            .unwrap();

        let entities = read_entity_dir(dir.path(), "tag", &entity_def)
            .await
            .unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].id, "bug");
    }

    #[tokio::test]
    async fn read_entity_dir_skips_bad_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = task_entity_def(); // expects .md with frontmatter

        // Write a valid .md file
        let path = entity_file_path(dir.path(), "01ABC", &entity_def);
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Good Task".into()));
        entity.set("body", Value::String("Body".into()));
        write_entity(&path, &entity, &entity_def).await.unwrap();

        // Write a .md file without frontmatter delimiters
        fs::write(dir.path().join("01DEF.md"), "just text, no frontmatter")
            .await
            .unwrap();

        let entities = read_entity_dir(dir.path(), "task", &entity_def)
            .await
            .unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].id, "01ABC");
    }

    #[tokio::test]
    async fn read_entity_dir_propagates_io_errors() {
        // Create a directory with a file that can't be read (permission denied).
        // On unix, we can remove read permission from a file.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let dir = tempfile::tempdir().unwrap();
            let entity_def = tag_entity_def();

            // Write a .yaml file, then remove read permission
            let path = dir.path().join("secret.yaml");
            fs::write(&path, "tag_name: secret\n").await.unwrap();
            let perms = std::fs::Permissions::from_mode(0o000);
            std::fs::set_permissions(&path, perms).unwrap();

            let result = read_entity_dir(dir.path(), "tag", &entity_def).await;
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), EntityError::Io(_)));

            // Restore permissions for cleanup
            let perms = std::fs::Permissions::from_mode(0o644);
            std::fs::set_permissions(&path, perms).unwrap();
        }
    }

    #[tokio::test]
    async fn trash_entity_files_moves_data_and_log() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();
        let path = entity_file_path(dir.path(), "bug", &entity_def);
        let log_path = path.with_extension("jsonl");
        let trash_dir = dir.path().join(".trash").join("tags");

        // Create data and log files
        let mut entity = Entity::new("tag", "bug");
        entity.set("tag_name", Value::String("bug".into()));
        write_entity(&path, &entity, &entity_def).await.unwrap();
        fs::write(&log_path, "{}\n").await.unwrap();

        assert!(path.exists());
        assert!(log_path.exists());

        trash_entity_files(&path, &trash_dir).await.unwrap();

        // Originals gone
        assert!(!path.exists());
        assert!(!log_path.exists());

        // Moved to trash
        assert!(trash_dir.join("bug.yaml").exists());
        assert!(trash_dir.join("bug.jsonl").exists());
    }

    #[tokio::test]
    async fn trash_entity_files_nonexistent_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.yaml");
        let trash_dir = dir.path().join(".trash").join("tags");
        // Should not error — creates trash dir but nothing to move
        trash_entity_files(&path, &trash_dir).await.unwrap();
        assert!(trash_dir.exists());
    }

    #[tokio::test]
    async fn write_entity_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();
        let path = dir.path().join("deep").join("nested").join("bug.yaml");

        let mut entity = Entity::new("tag", "bug");
        entity.set("tag_name", Value::String("bug".into()));
        write_entity(&path, &entity, &entity_def).await.unwrap();

        assert!(path.exists());
    }

    #[tokio::test]
    async fn body_containing_triple_dashes_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = task_entity_def();
        let path = entity_file_path(dir.path(), "01ABC", &entity_def);

        let body_with_dashes = "Some text\n---\nMore text after dashes\n---\nEven more";
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Dashes Test".into()));
        entity.set("body", Value::String(body_with_dashes.into()));

        write_entity(&path, &entity, &entity_def).await.unwrap();

        let loaded = read_entity(&path, "task", "01ABC", &entity_def)
            .await
            .unwrap();

        assert_eq!(loaded.get_str("title"), Some("Dashes Test"));
        assert_eq!(loaded.get_str("body"), Some(body_with_dashes));
    }

    #[tokio::test]
    async fn frontmatter_with_arrays_and_nested_values() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = task_entity_def();
        let path = entity_file_path(dir.path(), "01ABC", &entity_def);

        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Complex Task".into()));
        entity.set("assignees", serde_json::json!(["actor1", "actor2"]));
        entity.set("depends_on", serde_json::json!(["task1"]));
        entity.set("body", Value::String("Body with #tags".into()));

        write_entity(&path, &entity, &entity_def).await.unwrap();

        let loaded = read_entity(&path, "task", "01ABC", &entity_def)
            .await
            .unwrap();

        assert_eq!(loaded.get_str("title"), Some("Complex Task"));
        assert_eq!(
            loaded.get_string_list("assignees"),
            vec!["actor1", "actor2"]
        );
        assert_eq!(loaded.get_string_list("depends_on"), vec!["task1"]);
        assert_eq!(loaded.get_str("body"), Some("Body with #tags"));
    }

    #[test]
    fn parse_frontmatter_flattens_nested_objects() {
        // Legacy task files have nested position: {column, ordinal}
        // The parser should flatten them to position_column, position_ordinal
        let content = "---\ntitle: My Task\nposition:\n  column: todo\n  ordinal: a0\n  swimlane: feature\nassignees: []\n---\nBody text\n";

        let parsed =
            parse_frontmatter_body(content, "task", "01ABC", "body", Path::new("test.md")).unwrap();

        assert_eq!(parsed.get_str("title"), Some("My Task"));
        assert_eq!(parsed.get_str("position_column"), Some("todo"));
        assert_eq!(parsed.get_str("position_ordinal"), Some("a0"));
        assert_eq!(parsed.get_str("position_swimlane"), Some("feature"));
        // The nested "position" key should NOT exist as a field
        assert!(parsed.get("position").is_none());
        assert_eq!(parsed.get_str("body"), Some("Body text\n"));
    }

    #[test]
    fn parse_frontmatter_flat_fields_unchanged() {
        // New-format task files with flat position_column, position_ordinal
        let content =
            "---\ntitle: My Task\nposition_column: todo\nposition_ordinal: a0\n---\nBody\n";

        let parsed =
            parse_frontmatter_body(content, "task", "01ABC", "body", Path::new("test.md")).unwrap();

        assert_eq!(parsed.get_str("position_column"), Some("todo"));
        assert_eq!(parsed.get_str("position_ordinal"), Some("a0"));
    }

    #[test]
    fn parse_plain_yaml_flattens_nested_objects() {
        let content = "name: To Do\norder: 0\nmetadata:\n  color: red\n  icon: star\n";

        let parsed = parse_plain_yaml(content, "column", "todo", Path::new("test.yaml")).unwrap();

        assert_eq!(parsed.get_str("name"), Some("To Do"));
        assert_eq!(parsed.get_str("metadata_color"), Some("red"));
        assert_eq!(parsed.get_str("metadata_icon"), Some("star"));
    }

    #[tokio::test]
    async fn write_entity_concurrent_writes_do_not_collide() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();
        let path = entity_file_path(dir.path(), "shared", &entity_def);

        // Spawn 10 concurrent writes to the same entity path
        let mut handles = Vec::new();
        for i in 0..10 {
            let p = path.clone();
            let def = entity_def.clone();
            handles.push(tokio::spawn(async move {
                let mut entity = Entity::new("tag", "shared");
                entity.set("tag_name", Value::String(format!("variant_{i}")));
                entity.set("color", Value::String("ff0000".into()));
                write_entity(&p, &entity, &def).await.unwrap();
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // The file should exist and be valid (one of the writes won)
        let loaded = read_entity(&path, "tag", "shared", &entity_def)
            .await
            .unwrap();
        assert_eq!(loaded.entity_type, "tag");
        assert_eq!(loaded.id, "shared");
        // tag_name should be one of the variants, not corrupted
        let tag_name = loaded.get_str("tag_name").unwrap();
        assert!(tag_name.starts_with("variant_"), "tag_name was: {tag_name}");

        // No leftover temp files should remain
        let mut entries = fs::read_dir(dir.path()).await.unwrap();
        let mut count = 0;
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(!name.contains("tmp"), "leftover temp file found: {name}");
            count += 1;
        }
        assert_eq!(count, 1, "should have exactly one entity file");
    }

    #[tokio::test]
    async fn write_entity_cleans_up_temp_on_rename_failure() {
        // We test cleanup indirectly: write to a path where the parent dir exists
        // but the final target is a directory (rename will fail).
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();

        // Create a directory where the entity file should be — rename onto a dir fails
        let path = dir.path().join("blocker.yaml");
        fs::create_dir_all(&path).await.unwrap();

        let mut entity = Entity::new("tag", "blocker");
        entity.set("tag_name", Value::String("bug".into()));

        let result = write_entity(&path, &entity, &entity_def).await;
        assert!(
            result.is_err(),
            "write should fail when target is a directory"
        );

        // No temp files should be left behind
        let mut entries = fs::read_dir(dir.path()).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "blocker.yaml" {
                assert!(!name.contains("tmp"), "leftover temp file found: {name}");
            }
        }
    }

    #[tokio::test]
    async fn restore_entity_files_missing_data_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("live").join("bug.yaml");
        let trash_dir = dir.path().join(".trash").join("tags");
        // Create the trash directory but leave it empty — no data file to restore
        fs::create_dir_all(&trash_dir).await.unwrap();

        let result = restore_entity_files(&path, &trash_dir).await;
        assert!(
            result.is_err(),
            "restore should fail when data file is missing in trash"
        );
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("cannot restore from trash"),
            "error message should mention restore from trash, got: {msg}"
        );
        assert!(matches!(err, EntityError::RestoreFromTrashFailed { .. }));
    }

    #[tokio::test]
    async fn restore_entity_files_missing_changelog_ok() {
        let dir = tempfile::tempdir().unwrap();
        let live_dir = dir.path().join("live");
        let trash_dir = dir.path().join(".trash").join("tags");
        fs::create_dir_all(&trash_dir).await.unwrap();

        // Put only the data file in trash — no changelog
        let data_content = "tag_name: bug\ncolor: ff0000\n";
        fs::write(trash_dir.join("bug.yaml"), data_content)
            .await
            .unwrap();

        let path = live_dir.join("bug.yaml");
        let result = restore_entity_files(&path, &trash_dir).await;
        assert!(
            result.is_ok(),
            "restore should succeed when only changelog is missing"
        );

        // Data file should be back in the live dir
        assert!(path.exists());
        // Changelog should not exist (it was never there)
        assert!(!path.with_extension("jsonl").exists());
    }
}
