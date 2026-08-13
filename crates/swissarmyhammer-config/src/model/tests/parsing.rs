//! The description and the agent configuration a model file carries.
//!
//! YAML front matter and comment form, which one wins, and the malformed
//! openings the parser must refuse.

use super::*;

#[test]
fn test_parse_model_description_yaml_frontmatter() {
    let content = r#"---
description: "This is a test agent"
other_field: value
---
type: llama-embedding
config:
  source: !HuggingFace
    repo: test/embed"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("This is a test agent".to_string()));
}

#[test]
fn test_parse_model_description_comment_format() {
    let content = r#"# Description: This is a comment-based description
type: llama-embedding
config:
  source: !HuggingFace
    repo: test/embed"#;

    let description = parse_model_description(content);
    assert_eq!(
        description,
        Some("This is a comment-based description".to_string())
    );
}

#[test]
fn test_parse_model_description_no_description() {
    let content = r#"executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, None);
}

#[test]
fn test_parse_model_description_empty_yaml_description() {
    let content = r#"---
description: ""
other_field: value
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("".to_string()));
}

#[test]
fn test_parse_model_description_empty_comment_description() {
    let content = r#"# Description:
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, None); // Empty descriptions are treated as None
}

#[test]
fn test_parse_model_description_yaml_precedence() {
    let content = r#"---
description: "YAML description"
---
# Description: Comment description
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("YAML description".to_string()));
}

#[test]
fn test_parse_model_description_malformed_yaml() {
    let content = r#"---
invalid: yaml: content: [unclosed
---
# Description: Fallback comment description
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(
        description,
        Some("Fallback comment description".to_string())
    );
}

#[test]
fn test_parse_model_description_whitespace_handling() {
    let content = r#"---
description: "  Padded description  "
---"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("Padded description".to_string()));

    let comment_content = r#"# Description:   Padded comment   "#;
    let description = parse_model_description(comment_content);
    assert_eq!(description, Some("Padded comment".to_string()));
}

#[test]
fn test_parse_model_description_multiline_comment() {
    let content = r#"# Description: First line
# This is additional content
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(description, Some("First line".to_string()));
}

#[test]
fn test_parse_agent_config_frontmatter() {
    let content = r#"---
description: "Test agent"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let config = parse_model_config(content);
    assert!(config.is_ok(), "Should parse frontmatter agent config");
    let config = config.unwrap();
    assert!(!config.quiet);
}

#[test]
fn test_parse_agent_config_comment_format() {
    let content = r#"# Description: Test agent 2
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
      filename: test.gguf
    normalize: true
quiet: false"#;

    let config = parse_model_config(content);
    assert!(config.is_ok(), "Should parse comment format agent config");
    let config = config.unwrap();
    assert!(!config.quiet);
}

#[test]
fn test_parse_agent_config_pure_yaml() {
    let content = r#"executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: true"#;

    let config = parse_model_config(content);
    assert!(config.is_ok(), "Should parse pure YAML agent config");
    let config = config.unwrap();
    assert!(config.quiet);
}

#[test]
fn test_parse_model_description_survives_triple_hyphen_in_value() {
    // A `---` run embedded in a quoted scalar value must not be read as
    // the closing frontmatter delimiter. Only a line that is exactly
    // three hyphens delimits, so every frontmatter key -- including this
    // one -- must survive.
    let content = r#"---
description: "Claude Code --- installed separately"
other_field: value
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let description = parse_model_description(content);
    assert_eq!(
        description,
        Some("Claude Code --- installed separately".to_string())
    );
}

#[test]
fn test_parse_agent_config_survives_triple_hyphen_in_frontmatter_value() {
    // Same content as above: the frontmatter description holds a `---`
    // run. The model configuration must still load -- the frontmatter
    // split must not cut in the middle of the quoted scalar.
    let content = r#"---
description: "Claude Code --- installed separately"
other_field: value
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

    let config = parse_model_config(content);
    assert!(
        config.is_ok(),
        "model config must still load when frontmatter holds a `---` run: {:?}",
        config.err()
    );
    assert!(!config.unwrap().quiet);
}

#[test]
fn test_parse_model_description_rejects_four_hyphen_opening_line() {
    // A first line of four hyphens is not a valid opening delimiter --
    // only a line that is exactly three hyphens opens frontmatter.
    let content = "----\ndescription: not real frontmatter\n----\nquiet: true";
    assert_eq!(parse_model_description(content), None);
}

#[test]
fn test_parse_model_description_rejects_hyphen_with_trailing_text_opening_line() {
    // A first line of `---x` is not a valid opening delimiter either.
    let content = "---x\ndescription: not real frontmatter\n---\nquiet: true";
    assert_eq!(parse_model_description(content), None);
}

#[test]
fn test_parse_model_description_rejects_missing_closing_delimiter() {
    // No closing `---` line at all: this must not be read as delimited
    // frontmatter. With no `# Description:` comment either, the overall
    // result is `None`.
    let content = "---\ndescription: no closer\nother: field\n";
    assert_eq!(parse_model_description(content), None);
}
