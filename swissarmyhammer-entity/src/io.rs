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
pub fn entity_file_path(dir: &Path, id: &str, entity_def: &EntityDef) -> PathBuf {
    let safe_id = sanitize_id(id);
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
    entity_type: &str,
    id: &str,
    entity_def: &EntityDef,
) -> Result<Entity> {
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
/// Uses atomic write (temp file + rename) for safety.
pub async fn write_entity(
    path: &Path,
    entity: &Entity,
    entity_def: &EntityDef,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let content = if let Some(ref body_field) = entity_def.body_field {
        format_frontmatter_body(entity, body_field)?
    } else {
        format_plain_yaml(entity)?
    };

    // Use PID in temp extension to avoid collisions with concurrent writers
    let temp_ext = format!("tmp.{}", std::process::id());
    let temp_path = path.with_extension(temp_ext);
    fs::write(&temp_path, content.as_bytes()).await?;
    fs::rename(&temp_path, path).await?;

    Ok(())
}

/// Read all entities from a directory.
///
/// Scans for files matching the expected extension and parses each one.
pub async fn read_entity_dir(
    dir: &Path,
    entity_type: &str,
    entity_def: &EntityDef,
) -> Result<Vec<Entity>> {
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
            Err(e) => {
                warn!(path = %path.display(), error = %e, "skipping unparseable entity file");
                continue;
            }
        }
    }

    Ok(entities)
}

/// Delete an entity's data file and optional log file.
///
/// Silently succeeds if files don't exist (no TOCTOU race).
pub async fn delete_entity_files(path: &Path) -> Result<()> {
    // Attempt removal, ignore NotFound
    if let Err(e) = fs::remove_file(path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(EntityError::Io(e));
        }
    }

    let log_path = path.with_extension("jsonl");
    if let Err(e) = fs::remove_file(&log_path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(EntityError::Io(e));
        }
    }

    Ok(())
}

// --- Internal helpers ---

/// Parse a frontmatter+body file into an Entity.
fn parse_frontmatter_body(
    content: &str,
    entity_type: &str,
    id: &str,
    body_field: &str,
    path: &Path,
) -> Result<Entity> {
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
        serde_yaml::from_str(frontmatter).map_err(|e| EntityError::Yaml {
            path: path.to_path_buf(),
            source: e,
        })?;

    let mut entity = Entity::new(entity_type, id);
    for (k, v) in yaml_map {
        entity.set(k, v);
    }
    // Body field comes from the markdown body, not the frontmatter
    entity.set(body_field, Value::String(body.to_string()));

    Ok(entity)
}

/// Parse a plain YAML file into an Entity.
fn parse_plain_yaml(
    content: &str,
    entity_type: &str,
    id: &str,
    path: &Path,
) -> Result<Entity> {
    let yaml_map: HashMap<String, Value> =
        serde_yaml::from_str(content).map_err(|e| EntityError::Yaml {
            path: path.to_path_buf(),
            source: e,
        })?;

    let mut entity = Entity::new(entity_type, id);
    for (k, v) in yaml_map {
        entity.set(k, v);
    }

    Ok(entity)
}

/// Format an entity as frontmatter + markdown body.
fn format_frontmatter_body(entity: &Entity, body_field: &str) -> Result<String> {
    let body = entity
        .get_str(body_field)
        .unwrap_or("")
        .to_string();

    // Build frontmatter from all fields except the body field
    let mut frontmatter_map = serde_json::Map::new();
    for (k, v) in &entity.fields {
        if k != body_field {
            frontmatter_map.insert(k.clone(), v.clone());
        }
    }

    let frontmatter_value = Value::Object(frontmatter_map);
    let frontmatter_yaml = serde_yaml::to_string(&frontmatter_value).map_err(|e| {
        EntityError::Yaml {
            path: PathBuf::from("<serialization>"),
            source: e,
        }
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
    serde_yaml::to_string(&map_value).map_err(|e| EntityError::Yaml {
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
        }
    }

    fn tag_entity_def() -> EntityDef {
        EntityDef {
            name: "tag".into(),
            body_field: None,
            fields: vec!["tag_name".into(), "color".into()],
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
        entity.set("body", Value::String("This is the body.\nWith multiple lines.".into()));

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

        let parsed =
            parse_plain_yaml(&content, "tag", "bug", Path::new("test.yaml")).unwrap();

        assert_eq!(parsed.entity_type, "tag");
        assert_eq!(parsed.id, "bug");
        assert_eq!(parsed.get_str("tag_name"), Some("bug"));
        assert_eq!(parsed.get_str("color"), Some("ff0000"));
    }

    #[test]
    fn parse_frontmatter_missing_delimiters() {
        let content = "just some text without frontmatter";
        let result =
            parse_frontmatter_body(content, "task", "01ABC", "body", Path::new("test.md"));
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
        entity.set("body", Value::String("Task body content.\n\nWith paragraphs.".into()));

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

        let loaded = read_entity(&path, "tag", "bug", &entity_def)
            .await
            .unwrap();

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
        fs::write(dir.path().join("stray.md"), "# Not a tag").await.unwrap();

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
    async fn delete_entity_files_removes_data_and_log() {
        let dir = tempfile::tempdir().unwrap();
        let entity_def = tag_entity_def();
        let path = entity_file_path(dir.path(), "bug", &entity_def);
        let log_path = path.with_extension("jsonl");

        // Create data and log files
        let mut entity = Entity::new("tag", "bug");
        entity.set("tag_name", Value::String("bug".into()));
        write_entity(&path, &entity, &entity_def).await.unwrap();
        fs::write(&log_path, "{}\n").await.unwrap();

        assert!(path.exists());
        assert!(log_path.exists());

        delete_entity_files(&path).await.unwrap();

        assert!(!path.exists());
        assert!(!log_path.exists());
    }

    #[tokio::test]
    async fn delete_entity_files_nonexistent_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.yaml");
        // Should not error
        delete_entity_files(&path).await.unwrap();
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
        assert_eq!(loaded.get_string_list("assignees"), vec!["actor1", "actor2"]);
        assert_eq!(loaded.get_string_list("depends_on"), vec!["task1"]);
        assert_eq!(loaded.get_str("body"), Some("Body with #tags"));
    }
}
