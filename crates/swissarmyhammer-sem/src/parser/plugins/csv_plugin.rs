use std::collections::HashMap;

use crate::model::entity::{build_entity_id, SemanticEntity};
use crate::parser::plugin::SemanticParserPlugin;
use crate::utils::hash::content_hash;

pub struct CsvParserPlugin;

impl SemanticParserPlugin for CsvParserPlugin {
    fn id(&self) -> &str {
        "csv"
    }

    fn extensions(&self) -> &[&str] {
        &[".csv", ".tsv"]
    }

    fn extract_entities(&self, content: &str, file_path: &str) -> Vec<SemanticEntity> {
        let mut entities = Vec::new();
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            return entities;
        }

        let is_tsv = file_path.ends_with(".tsv");
        let separator = if is_tsv { '\t' } else { ',' };

        let headers = parse_csv_line(lines[0], separator);

        for (i, &line) in lines.iter().enumerate().skip(1) {
            let cells = parse_csv_line(line, separator);
            let row_id = if cells.first().is_none_or(|c| c.is_empty()) {
                format!("row_{i}")
            } else {
                cells[0].clone()
            };
            let name = format!("row[{row_id}]");

            let mut metadata = HashMap::new();
            for (j, header) in headers.iter().enumerate() {
                metadata.insert(header.clone(), cells.get(j).cloned().unwrap_or_default());
            }

            entities.push(SemanticEntity {
                id: build_entity_id(file_path, "row", &name, None),
                file_path: file_path.to_string(),
                entity_type: "row".to_string(),
                name,
                parent_id: None,
                content_hash: content_hash(line),
                structural_hash: None,
                content: line.to_string(),
                start_line: i + 1,
                end_line: i + 1,
                metadata: Some(metadata),
            });
        }

        entities
    }
}

fn parse_csv_line(line: &str, separator: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if in_quotes {
            if ch == '"' && chars.get(i + 1) == Some(&'"') {
                current.push('"');
                i += 1;
            } else if ch == '"' {
                in_quotes = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == separator {
            cells.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(ch);
        }
        i += 1;
    }
    cells.push(current.trim().to_string());
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin() -> CsvParserPlugin {
        CsvParserPlugin
    }

    #[test]
    fn test_basic_csv_row_extraction() {
        let content = "name,age,city\nAlice,30,NYC\nBob,25,LA\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].entity_type, "row");
        assert_eq!(entities[1].entity_type, "row");
    }

    #[test]
    fn test_header_values_in_metadata() {
        let content = "name,age,city\nAlice,30,NYC\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert_eq!(entities.len(), 1);
        let meta = entities[0].metadata.as_ref().unwrap();
        assert_eq!(meta.get("name").map(|s| s.as_str()), Some("Alice"));
        assert_eq!(meta.get("age").map(|s| s.as_str()), Some("30"));
        assert_eq!(meta.get("city").map(|s| s.as_str()), Some("NYC"));
    }

    #[test]
    fn test_row_name_uses_first_column() {
        let content = "id,value\nABC123,hello\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "row[ABC123]");
    }

    #[test]
    fn test_row_name_fallback_when_first_cell_empty() {
        let content = "id,value\n,hello\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert_eq!(entities.len(), 1);
        // row index 1 (skip(1) gives i=1 for the second element)
        assert!(entities[0].name.starts_with("row[row_"));
    }

    #[test]
    fn test_tsv_separator_detection() {
        let content = "name\tage\tcolor\nAlice\t30\tblue\n";
        let entities = plugin().extract_entities(content, "data.tsv");
        assert_eq!(entities.len(), 1);
        let meta = entities[0].metadata.as_ref().unwrap();
        assert_eq!(meta.get("name").map(|s| s.as_str()), Some("Alice"));
        assert_eq!(meta.get("age").map(|s| s.as_str()), Some("30"));
        assert_eq!(meta.get("color").map(|s| s.as_str()), Some("blue"));
    }

    #[test]
    fn test_quoted_fields_with_comma_inside() {
        let content = "name,address\nAlice,\"123 Main St, Apt 4\"\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert_eq!(entities.len(), 1);
        let meta = entities[0].metadata.as_ref().unwrap();
        assert_eq!(
            meta.get("address").map(|s| s.as_str()),
            Some("123 Main St, Apt 4")
        );
    }

    #[test]
    fn test_quoted_field_with_escaped_quote() {
        let content = "name,bio\nAlice,\"She said \"\"hello\"\"\"\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert_eq!(entities.len(), 1);
        let meta = entities[0].metadata.as_ref().unwrap();
        assert_eq!(
            meta.get("bio").map(|s| s.as_str()),
            Some("She said \"hello\"")
        );
    }

    #[test]
    fn test_empty_content_returns_no_entities() {
        let entities = plugin().extract_entities("", "data.csv");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_header_only_returns_no_entities() {
        let content = "name,age,city\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_line_numbers_are_correct() {
        let content = "id,name\n1,Alice\n2,Bob\n";
        let entities = plugin().extract_entities(content, "data.csv");
        assert_eq!(entities.len(), 2);
        // First data row is line 2 (header is line 1)
        assert_eq!(entities[0].start_line, 2);
        assert_eq!(entities[0].end_line, 2);
        assert_eq!(entities[1].start_line, 3);
        assert_eq!(entities[1].end_line, 3);
    }

    #[test]
    fn test_file_path_in_entity() {
        let content = "id\n1\n";
        let entities = plugin().extract_entities(content, "path/to/records.csv");
        assert_eq!(entities[0].file_path, "path/to/records.csv");
    }
}
