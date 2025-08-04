//! Integration tests for outline functionality

#[cfg(test)]
mod tests {
    use super::super::{FileDiscovery, FileDiscoveryConfig};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_end_to_end_file_discovery() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create a realistic project structure
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir)?;

        // Create Rust files
        fs::write(
            src_dir.join("main.rs"),
            r#"
            fn main() {
                println!("Hello, world!");
            }
        "#,
        )?;

        fs::write(
            src_dir.join("lib.rs"),
            r#"
            pub mod utils;
            
            pub struct Calculator {
                value: f64,
            }
            
            impl Calculator {
                pub fn new() -> Self {
                    Self { value: 0.0 }
                }
                
                pub fn add(&mut self, value: f64) -> &mut Self {
                    self.value += value;
                    self
                }
            }
        "#,
        )?;

        fs::write(
            src_dir.join("utils.rs"),
            r#"
            pub fn format_number(n: f64) -> String {
                format!("{:.2}", n)
            }
            
            #[derive(Debug)]
            pub enum Operation {
                Add,
                Subtract,
                Multiply,
                Divide,
            }
        "#,
        )?;

        // Create Python files
        fs::write(
            temp_dir.path().join("script.py"),
            r#"
            def hello_world():
                """Print hello world message."""
                print("Hello, world!")
            
            class Calculator:
                def __init__(self):
                    self.value = 0
                
                def add(self, value):
                    self.value += value
                    return self
        "#,
        )?;

        // Create files to be ignored
        fs::write(temp_dir.path().join("README.md"), "# Test Project")?;
        fs::write(temp_dir.path().join("config.json"), "{}")?;

        // Test multi-language discovery
        let patterns = vec![
            format!("{}/**/*.rs", temp_dir.path().display()),
            format!("{}/**/*.py", temp_dir.path().display()),
        ];

        let discovery = FileDiscovery::new(patterns)?;
        let (files, report) = discovery.discover_files()?;

        // Verify results
        assert!(files.len() >= 4, "Should discover at least 4 files");
        assert!(
            report.supported_files >= 4,
            "Should have at least 4 supported files"
        );
        assert_eq!(report.errors.len(), 0, "Should have no errors");

        // Check that we found both Rust and Python files
        let rust_files: Vec<_> = files
            .iter()
            .filter(|f| f.extension() == Some("rs"))
            .collect();
        let python_files: Vec<_> = files
            .iter()
            .filter(|f| f.extension() == Some("py"))
            .collect();

        assert_eq!(rust_files.len(), 3, "Should find 3 Rust files");
        assert_eq!(python_files.len(), 1, "Should find 1 Python file");

        // Check specific files
        assert!(files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "main.rs"));
        assert!(files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "lib.rs"));
        assert!(files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "utils.rs"));
        assert!(files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "script.py"));

        // Should not find non-code files
        assert!(!files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "README.md"));
        assert!(!files
            .iter()
            .any(|f| f.path.file_name().unwrap() == "config.json"));

        println!("Discovery report: {}", report.summary());

        Ok(())
    }

    #[test]
    fn test_gitignore_respect() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Initialize as git repo
        fs::create_dir_all(temp_dir.path().join(".git"))?;

        // Create .gitignore
        fs::write(
            temp_dir.path().join(".gitignore"),
            r#"
            target/
            *.tmp
            .env
        "#,
        )?;

        // Create files
        fs::write(temp_dir.path().join("main.rs"), "fn main() {}")?;
        fs::write(temp_dir.path().join("temp.tmp"), "temporary")?;
        fs::write(temp_dir.path().join(".env"), "SECRET=value")?;

        // Create target directory
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&target_dir)?;
        fs::write(target_dir.join("debug.rs"), "// generated")?;

        let patterns = vec![format!("{}/**/*", temp_dir.path().display())];
        let config = FileDiscoveryConfig::new(); // Respects gitignore by default
        let discovery = FileDiscovery::with_config(patterns, config)?;

        let (files, report) = discovery.discover_files()?;

        // Should only find main.rs
        let code_files: Vec<_> = files.iter().filter(|f| f.is_supported()).collect();
        assert_eq!(code_files.len(), 1);
        assert_eq!(code_files[0].path.file_name().unwrap(), "main.rs");

        // Check that ignored files were properly skipped
        assert!(
            report.files_skipped_ignored > 0,
            "Should have skipped ignored files"
        );

        Ok(())
    }

    #[test]
    fn test_large_file_filtering() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;

        // Create normal file
        fs::write(temp_dir.path().join("small.rs"), "fn main() {}")?;

        // Create large file (exceed 1KB limit)
        let large_content = "// ".repeat(600); // ~1.2KB
        fs::write(temp_dir.path().join("large.rs"), large_content)?;

        let patterns = vec![format!("{}/*.rs", temp_dir.path().display())];
        let config = FileDiscoveryConfig::new().with_max_file_size(1024); // 1KB limit
        let discovery = FileDiscovery::with_config(patterns, config)?;

        let (files, report) = discovery.discover_files()?;

        // Should only find the small file
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path.file_name().unwrap(), "small.rs");

        // Large file should be recorded as skipped
        assert_eq!(report.files_skipped_size, 1);

        Ok(())
    }
}
