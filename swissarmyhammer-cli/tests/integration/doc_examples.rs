use std::fs;
use std::path::Path;
use walkdir::WalkDir;

// Note: This test uses WalkDir instead of PromptResolver because it's specifically
// validating documentation example files, not actual runtime prompts. Documentation
// examples are stored outside the standard prompt locations (builtin/user/local) and
// need direct file system access for validation purposes.
#[test]
fn test_all_doc_example_prompts_are_valid() {
    let doc_examples_dir = std::env::var("SWISSARMYHAMMER_DOC_EXAMPLES_PATH")
        .unwrap_or_else(|_| "../doc/examples/prompts".to_string());
    let doc_examples_dir = Path::new(&doc_examples_dir);

    // Skip if examples directory doesn't exist (e.g., in CI without full checkout)
    if !doc_examples_dir.exists() {
        eprintln!("Skipping doc examples test - examples directory not found");
        return;
    }

    let mut tested_files = 0;
    let mut failed_files = vec![];

    // Pre-compile regex patterns outside the loop
    let unclosed_tags_regex =
        regex::Regex::new(r"\{%\s*(?:if|for|unless|case)\s+[^%]*%\}").unwrap();
    let closing_tags_regex =
        regex::Regex::new(r"\{%\s*(?:endif|endfor|endunless|endcase)\s*%\}").unwrap();
    let malformed_vars_regex = regex::Regex::new(r"\{\{[^}]*\{").unwrap();

    // Walk through all .md files in the prompts examples directory
    // Using WalkDir is appropriate here as we're validating documentation files, not runtime prompts
    for entry in WalkDir::new(doc_examples_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process .md files
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        tested_files += 1;

        // Read the file content
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => {
                failed_files.push(format!("{}: Failed to read - {}", path.display(), e));
                continue;
            }
        };

        // Basic validation - check for YAML front matter and basic structure
        if !content.starts_with("---") {
            failed_files.push(format!("{}: Missing YAML front matter", path.display()));
            continue;
        }

        // Find the end of front matter
        let lines: Vec<&str> = content.lines().collect();
        let mut end_line = None;
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.trim() == "---" {
                end_line = Some(i);
                break;
            }
        }

        if end_line.is_none() {
            failed_files.push(format!(
                "{}: Missing closing YAML delimiter",
                path.display()
            ));
            continue;
        }

        // Extract and parse YAML
        let yaml_content = lines[1..end_line.unwrap()].join("\n");
        match serde_yaml::from_str::<serde_yaml::Value>(&yaml_content) {
            Ok(yaml) => {
                // Check for required fields
                let yaml_map = yaml.as_mapping();
                if let Some(map) = yaml_map {
                    // Must have either 'name' or 'title'
                    if !map.contains_key("name") && !map.contains_key("title") {
                        failed_files.push(format!(
                            "{}: Missing 'name' or 'title' field",
                            path.display()
                        ));
                    }
                    // Must have 'description'
                    if !map.contains_key("description") {
                        failed_files
                            .push(format!("{}: Missing 'description' field", path.display()));
                    }
                } else {
                    failed_files.push(format!(
                        "{}: YAML front matter is not a mapping",
                        path.display()
                    ));
                }
            }
            Err(e) => {
                failed_files.push(format!("{}: YAML parsing error - {}", path.display(), e));
            }
        }

        // Extract template content
        let template_content = lines[end_line.unwrap() + 1..].join("\n");

        // Basic Liquid syntax check using regex
        // Check for common syntax errors
        let open_count = unclosed_tags_regex.find_iter(&template_content).count();
        let close_count = closing_tags_regex.find_iter(&template_content).count();

        if open_count != close_count {
            failed_files.push(format!(
                "{}: Mismatched Liquid tags - {} opening tags, {} closing tags",
                path.display(),
                open_count,
                close_count
            ));
        }

        // Check for malformed variable syntax
        if malformed_vars_regex.is_match(&template_content) {
            failed_files.push(format!(
                "{}: Malformed variable syntax detected",
                path.display()
            ));
        }
    }

    // Report results
    if !failed_files.is_empty() {
        panic!(
            "Documentation example validation failed!\n\n\
            Tested {} files, {} failed:\n\n{}\n\n\
            Fix these validation errors in the documentation examples.",
            tested_files,
            failed_files.len(),
            failed_files.join("\n\n")
        );
    }

    println!("Successfully validated {tested_files} documentation example prompts");
}

#[test]
fn test_doc_examples_directory_structure() {
    let doc_examples_dir = std::env::var("SWISSARMYHAMMER_DOC_EXAMPLES_PATH")
        .map(|p| p.replace("/prompts", ""))
        .unwrap_or_else(|_| "../doc/examples".to_string());
    let doc_examples_dir = Path::new(&doc_examples_dir);

    // Skip if examples directory doesn't exist
    if !doc_examples_dir.exists() {
        eprintln!("Skipping doc examples structure test - examples directory not found");
        return;
    }

    // Check expected subdirectories exist
    let expected_dirs = vec!["prompts", "workflows", "scripts", "configs"];

    for dir_name in expected_dirs {
        let dir_path = doc_examples_dir.join(dir_name);
        assert!(
            dir_path.exists() && dir_path.is_dir(),
            "Expected examples subdirectory '{dir_name}' not found"
        );
    }
}

#[test]
fn test_doc_markdown_includes_valid_paths() {
    let doc_src_dir = Path::new("../doc/src");

    // Skip if doc directory doesn't exist
    if !doc_src_dir.exists() {
        eprintln!("Skipping markdown includes test - doc directory not found");
        return;
    }

    let mut invalid_includes = vec![];

    // Pre-compile regex pattern outside the loop
    let include_regex = regex::Regex::new(r"\{\{#include\s+([^\}]+)\}\}").unwrap();

    // Walk through all .md files in doc/src
    // WalkDir is used here to check documentation includes, not runtime files
    for entry in WalkDir::new(doc_src_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Only process .md files
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        // Read file content
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        // Find all {{#include ...}} directives
        for captures in include_regex.captures_iter(&content) {
            if let Some(include_path) = captures.get(1) {
                let include_path_str = include_path.as_str().trim();

                // Resolve the include path relative to the markdown file
                let resolved_path = path.parent().unwrap().join(include_path_str);

                // Check if the included file exists
                if !resolved_path.exists() {
                    invalid_includes.push(format!(
                        "File: {}\n  Invalid include: {}\n  Resolved to: {}",
                        path.display(),
                        include_path_str,
                        resolved_path.display()
                    ));
                }
            }
        }
    }

    if !invalid_includes.is_empty() {
        panic!(
            "Found {} invalid mdbook include paths:\n\n{}",
            invalid_includes.len(),
            invalid_includes.join("\n\n")
        );
    }
}

#[test]
fn test_example_prompts_have_required_fields() {
    let doc_examples_dir = std::env::var("SWISSARMYHAMMER_DOC_EXAMPLES_PATH")
        .unwrap_or_else(|_| "../doc/examples/prompts".to_string());
    let doc_examples_dir = Path::new(&doc_examples_dir);

    // Skip if examples directory doesn't exist
    if !doc_examples_dir.exists() {
        eprintln!("Skipping prompt fields test - examples directory not found");
        return;
    }

    let mut missing_fields = vec![];

    // Direct file system traversal is needed for documentation validation
    for entry in WalkDir::new(doc_examples_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        // Check for required fields in YAML front matter
        let has_name_or_title = content.contains("\nname:") || content.contains("\ntitle:");
        let has_description = content.contains("\ndescription:");

        if !has_name_or_title {
            missing_fields.push(format!(
                "{}: Missing 'name' or 'title' field",
                path.display()
            ));
        }
        if !has_description {
            missing_fields.push(format!("{}: Missing 'description' field", path.display()));
        }
    }

    if !missing_fields.is_empty() {
        panic!(
            "Example prompts missing required fields:\n\n{}",
            missing_fields.join("\n")
        );
    }
}
