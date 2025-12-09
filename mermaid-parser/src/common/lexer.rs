//! Common lexing utilities

/// Remove metadata comments from input (lines starting with //)
pub fn strip_metadata_comments(input: &str) -> String {
    input
        .lines()
        .filter(|line| !line.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_metadata_comments() {
        let input = r#"// Metadata comment
timeline
// Another metadata
title My Timeline
Normal content"#;

        let result = strip_metadata_comments(input);
        let expected = "timeline\ntitle My Timeline\nNormal content";
        assert_eq!(result, expected);
    }
}
