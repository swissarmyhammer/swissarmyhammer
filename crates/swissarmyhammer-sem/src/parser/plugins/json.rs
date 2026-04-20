use crate::model::entity::{build_entity_id, SemanticEntity};
use crate::parser::plugin::SemanticParserPlugin;
use crate::utils::hash::content_hash;

/// JSON parser plugin that extracts top-level properties from JSON objects as semantic entities.
pub struct JsonParserPlugin;

impl SemanticParserPlugin for JsonParserPlugin {
    fn id(&self) -> &str {
        "json"
    }

    fn extensions(&self) -> &[&str] {
        &[".json"]
    }

    fn extract_entities(&self, content: &str, file_path: &str) -> Vec<SemanticEntity> {
        // Only extract top-level properties from JSON objects.
        // We scan the source text directly to get accurate line positions,
        // which weave needs for entity-level merge reconstruction.
        let trimmed = content.trim();
        if !trimmed.starts_with('{') {
            return Vec::new();
        }

        let lines: Vec<&str> = content.lines().collect();
        let entries = find_top_level_entries(content);

        let mut entities = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let end_line = if i + 1 < entries.len() {
                // End just before the next entry starts (minus trailing blank/comma lines)
                let next_start = entries[i + 1].start_line;
                trim_trailing_blanks(&lines, entry.start_line, next_start)
            } else {
                // Last entry: end before the closing brace
                let closing = find_closing_brace_line(&lines);
                trim_trailing_blanks(&lines, entry.start_line, closing)
            };

            let entity_content = lines[entry.start_line - 1..end_line].join("\n");

            // Compute a structural_hash over just the value (excluding the key name)
            // so that rename detection works: "timeout": 30 → "request_timeout": 30
            let value_content = extract_value_content(&entity_content);
            let structural_hash = Some(content_hash(value_content));

            entities.push(SemanticEntity {
                id: build_entity_id(file_path, &entry.entity_type, &entry.pointer, None),
                file_path: file_path.to_string(),
                entity_type: entry.entity_type.clone(),
                name: entry.key.clone(),
                parent_id: None,
                content_hash: content_hash(&entity_content),
                structural_hash,
                content: entity_content,
                start_line: entry.start_line,
                end_line,
                metadata: None,
            });
        }

        entities
    }
}

struct JsonEntry {
    key: String,
    pointer: String,
    entity_type: String,
    start_line: usize, // 1-based
}

/// Scan the source text to find each top-level key in the root JSON object.
/// Returns entries with accurate start_line positions.
fn find_top_level_entries(content: &str) -> Vec<JsonEntry> {
    let mut entries = Vec::new();
    let mut state = ParseState::new();

    for ch in content.chars() {
        state.process_char(ch, &mut entries);
    }

    // Handle last entry (no trailing comma)
    if let Some(entry) = entries.last_mut() {
        if entry.entity_type.is_empty() {
            entry.entity_type = "property".to_string();
        }
    }

    entries
}

struct ParseState {
    depth: usize,
    in_string: bool,
    escape_next: bool,
    line_num: usize,
    current_key: Option<String>,
    key_start: bool,
    key_buf: String,
    reading_key: bool,
}

impl ParseState {
    fn new() -> Self {
        Self {
            depth: 0,
            in_string: false,
            escape_next: false,
            line_num: 1,
            current_key: None,
            key_start: false,
            key_buf: String::new(),
            reading_key: false,
        }
    }

    fn process_char(&mut self, ch: char, entries: &mut Vec<JsonEntry>) {
        if ch == '\n' {
            self.line_num += 1;
            return;
        }

        if self.escape_next {
            if self.reading_key {
                self.key_buf.push(ch);
            }
            self.escape_next = false;
            return;
        }

        if ch == '\\' && self.in_string {
            if self.reading_key {
                self.key_buf.push(ch);
            }
            self.escape_next = true;
            return;
        }

        if self.in_string {
            if ch == '"' {
                self.in_string = false;
                if self.reading_key {
                    self.reading_key = false;
                    self.current_key = Some(self.key_buf.clone());
                    self.key_buf.clear();
                }
            } else if self.reading_key {
                self.key_buf.push(ch);
            }
            return;
        }

        match ch {
            '"' => {
                self.in_string = true;
                if self.depth == 1 && self.current_key.is_none() && !self.key_start {
                    self.reading_key = true;
                    self.key_buf.clear();
                }
            }
            ':' if self.depth == 1 => {
                if let Some(ref key) = self.current_key {
                    let escaped_key = key.replace('~', "~0").replace('/', "~1");
                    let pointer = format!("/{escaped_key}");
                    entries.push(JsonEntry {
                        key: key.clone(),
                        pointer,
                        entity_type: String::new(),
                        start_line: self.line_num,
                    });
                    self.key_start = true;
                }
            }
            '{' | '[' => {
                self.depth += 1;
                if self.depth == 2 && self.key_start {
                    if let Some(entry) = entries.last_mut() {
                        entry.entity_type = "object".to_string();
                    }
                }
            }
            '}' | ']' => {
                self.depth = self.depth.saturating_sub(1);
            }
            ',' if self.depth == 1 => {
                if let Some(entry) = entries.last_mut() {
                    if entry.entity_type.is_empty() {
                        entry.entity_type = "property".to_string();
                    }
                }
                self.current_key = None;
                self.key_start = false;
            }
            _ => {}
        }
    }
}

/// Extract just the value portion of a `"key": value` entity content string,
/// stripping the key name so that renamed keys with identical values share the
/// same structural_hash and are detected as renames rather than delete + add.
fn extract_value_content(content: &str) -> &str {
    let mut in_string = false;
    let mut escape_next = false;
    for (i, ch) in content.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
        }
        if ch == ':' && !in_string {
            let rest = content[i + 1..].trim();
            return rest.trim_end_matches(',').trim();
        }
    }
    content
}

/// Find the line number (1-based) of the closing `}` of the root object.
fn find_closing_brace_line(lines: &[&str]) -> usize {
    for (i, line) in lines.iter().enumerate().rev() {
        if line.trim() == "}" {
            return i + 1;
        }
    }
    lines.len()
}

/// Walk backwards from next_start to skip trailing blank lines and commas,
/// returning the end_line (1-based, inclusive) for the current entry.
fn trim_trailing_blanks(lines: &[&str], start: usize, next_start: usize) -> usize {
    let mut end = next_start - 1;
    while end > start {
        let trimmed = lines[end - 1].trim();
        if trimmed.is_empty() || trimmed == "," {
            end -= 1;
        } else {
            break;
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::change::ChangeType;
    use crate::model::identity::match_entities;

    #[test]
    fn test_json_line_positions() {
        let content = r#"{
  "name": "my-app",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc",
    "test": "jest"
  },
  "description": "a test app"
}
"#;
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "package.json");

        assert_eq!(entities.len(), 4);

        assert_eq!(entities[0].name, "name");
        assert_eq!(entities[0].start_line, 2);
        assert_eq!(entities[0].end_line, 2);

        assert_eq!(entities[1].name, "version");
        assert_eq!(entities[1].start_line, 3);
        assert_eq!(entities[1].end_line, 3);

        assert_eq!(entities[2].name, "scripts");
        assert_eq!(entities[2].entity_type, "object");
        assert_eq!(entities[2].start_line, 4);
        assert_eq!(entities[2].end_line, 7);

        assert_eq!(entities[3].name, "description");
        assert_eq!(entities[3].start_line, 8);
        assert_eq!(entities[3].end_line, 8);
    }

    #[test]
    fn test_rename_detected_end_to_end() {
        let before_content = "{\n  \"timeout\": 30\n}\n";
        let after_content = "{\n  \"request_timeout\": 30\n}\n";
        let plugin = JsonParserPlugin;
        let before = plugin.extract_entities(before_content, "config.json");
        let after = plugin.extract_entities(after_content, "config.json");
        let result = match_entities(&before, &after, "config.json", None, None, None);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::Renamed);
        assert_eq!(result.changes[0].entity_name, "request_timeout");
    }

    #[test]
    fn test_renamed_scalar_property_shares_structural_hash() {
        let before_content = "{\n  \"timeout\": 30\n}\n";
        let after_content = "{\n  \"request_timeout\": 30\n}\n";
        let plugin = JsonParserPlugin;
        let before = plugin.extract_entities(before_content, "config.json");
        let after = plugin.extract_entities(after_content, "config.json");
        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), 1);
        // content_hash differs (key name is part of content)
        assert_ne!(before[0].content_hash, after[0].content_hash);
        // structural_hash matches (same value)
        assert_eq!(before[0].structural_hash, after[0].structural_hash);
    }

    #[test]
    fn test_non_object_json_returns_empty() {
        // JSON arrays at top level should return no entities
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities("[1, 2, 3]", "data.json");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_empty_json_returns_empty() {
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities("", "empty.json");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_whitespace_only_returns_empty() {
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities("   \n  \n  ", "blank.json");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_single_property() {
        let content = "{\n  \"key\": \"value\"\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "test.json");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "key");
        assert_eq!(entities[0].entity_type, "property");
    }

    #[test]
    fn test_nested_array_value() {
        let content = "{\n  \"items\": [\n    1,\n    2,\n    3\n  ]\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "arr.json");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "items");
        assert_eq!(entities[0].entity_type, "object");
    }

    #[test]
    fn test_boolean_and_null_values() {
        let content = "{\n  \"enabled\": true,\n  \"debug\": false,\n  \"extra\": null\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "flags.json");
        assert_eq!(entities.len(), 3);
        assert!(entities.iter().all(|e| e.entity_type == "property"));
    }

    #[test]
    fn test_numeric_value() {
        let content = "{\n  \"port\": 8080\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "config.json");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_type, "property");
    }

    #[test]
    fn test_json_pointer_with_special_chars() {
        // Key with / and ~ should be escaped in the JSON pointer
        let content = "{\n  \"a/b\": 1,\n  \"c~d\": 2\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "special.json");
        assert_eq!(entities.len(), 2);
        // Check pointer escaping: ~ → ~0, / → ~1
        assert_eq!(entities[0].id, "special.json::property::/a~1b");
        assert_eq!(entities[1].id, "special.json::property::/c~0d");
    }

    #[test]
    fn test_string_value_with_colon() {
        // Colon inside a string value should not confuse the parser
        let content = "{\n  \"url\": \"http://example.com:8080\"\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "urls.json");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "url");
    }

    #[test]
    fn test_escaped_quote_in_key() {
        // Key with escaped quotes inside
        let content = "{\n  \"say\\\"hi\\\"\": \"value\"\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "escaped.json");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "say\\\"hi\\\"");
    }

    #[test]
    fn test_many_properties() {
        let content = "{\n  \"a\": 1,\n  \"b\": 2,\n  \"c\": 3,\n  \"d\": 4,\n  \"e\": 5\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "many.json");
        assert_eq!(entities.len(), 5);
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn test_extract_value_content_scalar() {
        let result = extract_value_content("\"key\": 42");
        assert_eq!(result, "42");
    }

    #[test]
    fn test_extract_value_content_string_value() {
        let result = extract_value_content("\"key\": \"hello\"");
        assert_eq!(result, "\"hello\"");
    }

    #[test]
    fn test_extract_value_content_with_trailing_comma() {
        let result = extract_value_content("\"key\": 42,");
        assert_eq!(result, "42");
    }

    #[test]
    fn test_extract_value_content_with_colon_in_string_key() {
        // Colon inside the key string should be skipped; only the bare colon matters
        let result = extract_value_content("\"url:port\": 8080");
        assert_eq!(result, "8080");
    }

    #[test]
    fn test_extract_value_content_no_colon() {
        // If there's no colon, returns the original content
        let result = extract_value_content("just some text");
        assert_eq!(result, "just some text");
    }

    #[test]
    fn test_find_closing_brace_line() {
        let lines = vec!["  {", "    \"a\": 1", "  }"];
        assert_eq!(find_closing_brace_line(&lines), 3);
    }

    #[test]
    fn test_find_closing_brace_line_no_brace() {
        let lines = vec!["no", "closing", "brace"];
        assert_eq!(find_closing_brace_line(&lines), 3); // falls back to lines.len()
    }

    #[test]
    fn test_trim_trailing_blanks() {
        let lines = vec!["{", "  \"a\": 1,", "", "  \"b\": 2", "}"];
        // From start=2 (line 2, "  \"a\": 1,"), next_start=4 (line 4, "  \"b\": 2")
        // Should trim the blank line 3 and return end_line 2
        let end = trim_trailing_blanks(&lines, 2, 4);
        assert_eq!(end, 2);
    }

    #[test]
    fn test_json_plugin_id_and_extensions() {
        let plugin = JsonParserPlugin;
        assert_eq!(plugin.id(), "json");
        assert_eq!(plugin.extensions(), &[".json"]);
    }

    #[test]
    fn test_entity_file_path() {
        let content = "{\n  \"key\": \"value\"\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "path/to/config.json");
        assert_eq!(entities[0].file_path, "path/to/config.json");
    }

    #[test]
    fn test_deeply_nested_object_is_single_entity() {
        // Only top-level keys become entities
        let content =
            "{\n  \"config\": {\n    \"db\": {\n      \"host\": \"localhost\"\n    }\n  }\n}\n";
        let plugin = JsonParserPlugin;
        let entities = plugin.extract_entities(content, "deep.json");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "config");
        assert_eq!(entities[0].entity_type, "object");
    }

    #[test]
    fn test_renamed_object_property_shares_structural_hash() {
        let before_content = "{\n  \"config\": {\n    \"port\": 8080\n  }\n}\n";
        let after_content = "{\n  \"settings\": {\n    \"port\": 8080\n  }\n}\n";
        let plugin = JsonParserPlugin;
        let before = plugin.extract_entities(before_content, "config.json");
        let after = plugin.extract_entities(after_content, "config.json");
        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), 1);
        assert_ne!(before[0].content_hash, after[0].content_hash);
        assert_eq!(before[0].structural_hash, after[0].structural_hash);
    }
}
