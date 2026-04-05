use crate::model::entity::{build_entity_id, SemanticEntity};
use crate::parser::plugin::SemanticParserPlugin;
use crate::utils::hash::content_hash;

pub struct FallbackParserPlugin;

const CHUNK_SIZE: usize = 20;

impl SemanticParserPlugin for FallbackParserPlugin {
    fn id(&self) -> &str {
        "fallback"
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }

    fn extract_entities(&self, content: &str, file_path: &str) -> Vec<SemanticEntity> {
        let lines: Vec<&str> = content.lines().collect();
        let mut entities = Vec::new();

        let mut i = 0;
        while i < lines.len() {
            let end = (i + CHUNK_SIZE).min(lines.len());
            let chunk: Vec<&str> = lines[i..end].to_vec();
            let chunk_content = chunk.join("\n");
            let start_line = i + 1;
            let end_line = end;
            let name = format!("lines {start_line}-{end_line}");

            entities.push(SemanticEntity {
                id: build_entity_id(file_path, "chunk", &name, None),
                file_path: file_path.to_string(),
                entity_type: "chunk".to_string(),
                name,
                parent_id: None,
                content_hash: content_hash(&chunk_content),
                structural_hash: None,
                content: chunk_content,
                start_line,
                end_line,
                metadata: None,
            });

            i += CHUNK_SIZE;
        }

        entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin() -> FallbackParserPlugin {
        FallbackParserPlugin
    }

    #[test]
    fn test_empty_content_returns_no_entities() {
        let entities = plugin().extract_entities("", "file.txt");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_single_chunk_under_20_lines() {
        let content = (1..=10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let entities = plugin().extract_entities(&content, "file.txt");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].entity_type, "chunk");
        assert_eq!(entities[0].start_line, 1);
        assert_eq!(entities[0].end_line, 10);
        assert_eq!(entities[0].name, "lines 1-10");
    }

    #[test]
    fn test_exactly_20_lines_is_one_chunk() {
        let content = (1..=20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let entities = plugin().extract_entities(&content, "file.txt");
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].start_line, 1);
        assert_eq!(entities[0].end_line, 20);
    }

    #[test]
    fn test_21_lines_creates_two_chunks() {
        let content = (1..=21)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let entities = plugin().extract_entities(&content, "file.txt");
        assert_eq!(entities.len(), 2);
        // First chunk: lines 1-20
        assert_eq!(entities[0].start_line, 1);
        assert_eq!(entities[0].end_line, 20);
        assert_eq!(entities[0].name, "lines 1-20");
        // Second chunk: line 21
        assert_eq!(entities[1].start_line, 21);
        assert_eq!(entities[1].end_line, 21);
        assert_eq!(entities[1].name, "lines 21-21");
    }

    #[test]
    fn test_40_lines_creates_two_equal_chunks() {
        let content = (1..=40)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let entities = plugin().extract_entities(&content, "file.txt");
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].start_line, 1);
        assert_eq!(entities[0].end_line, 20);
        assert_eq!(entities[1].start_line, 21);
        assert_eq!(entities[1].end_line, 40);
    }

    #[test]
    fn test_chunk_content_contains_lines() {
        let content = "alpha\nbeta\ngamma\n";
        let entities = plugin().extract_entities(content, "file.txt");
        assert_eq!(entities.len(), 1);
        assert!(entities[0].content.contains("alpha"));
        assert!(entities[0].content.contains("beta"));
        assert!(entities[0].content.contains("gamma"));
    }

    #[test]
    fn test_file_path_in_entity() {
        let content = "hello\n";
        let entities = plugin().extract_entities(content, "my/path/file.log");
        assert_eq!(entities[0].file_path, "my/path/file.log");
    }

    #[test]
    fn test_chunk_ids_include_file_path() {
        let content = "hello\n";
        let entities = plugin().extract_entities(content, "src/notes.txt");
        assert!(entities[0].id.contains("src/notes.txt"));
    }
}
