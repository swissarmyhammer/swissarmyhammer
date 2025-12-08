//! Utility functions shared across the codebase.

/// Checks if a string represents a local file path or file:// URL.
///
/// This function identifies various forms of local file paths:
/// - Absolute Unix paths: `/path/to/file`
/// - Relative paths: `./file`, `../file`
/// - Windows absolute paths: `C:\path`, `D:/path`  
/// - File URLs: `file:///path/to/file`, `file://./relative.md`
/// - Simple relative filenames: `file.md`, `document.txt`
///
/// # Arguments
///
/// * `input` - The string to check
///
/// # Returns
///
/// Returns `true` if the input appears to be a local file path, `false` otherwise.
pub fn is_local_file_path(input: &str) -> bool {
    let trimmed = input.trim();

    // Check for file:// URLs first
    if trimmed.starts_with("file://") {
        return true;
    }

    // Check for absolute paths (Unix-style), but not protocol-relative URLs
    if trimmed.starts_with('/') && !trimmed.starts_with("//") {
        return true;
    }

    // Check for relative paths
    if trimmed.starts_with("./") || trimmed.starts_with("../") {
        return true;
    }

    // Check for Windows-style absolute paths (C:\, D:\, etc.)
    if trimmed.len() >= 3
        && trimmed.chars().nth(1) == Some(':')
        && (trimmed.chars().nth(2) == Some('\\') || trimmed.chars().nth(2) == Some('/'))
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return true;
    }

    // Check for file paths that don't look like URLs (no protocol)
    if !trimmed.contains("://") && (trimmed.contains('/') || trimmed.contains('\\')) {
        // Don't treat protocol-relative URLs (starting with //) as local files
        if trimmed.starts_with("//") {
            return false;
        }

        // Don't treat URLs with known schemes as local files (excluding file: which we handled above)
        let known_schemes = [
            "data:",
            "javascript:",
            "mailto:",
            "ftp:",
            "tel:",
            "sms:",
            "http:",
            "https:",
        ];
        for scheme in &known_schemes {
            if trimmed.starts_with(scheme) {
                return false;
            }
        }

        // Don't treat domain-like patterns as local files unless they clearly look like paths
        return !trimmed.starts_with("www.")
            && !trimmed.contains("://")
            && (trimmed.starts_with('.') || trimmed.contains('/') || trimmed.contains('\\'));
    }

    // Check for simple relative filenames (no path separators but look like files)
    if !trimmed.contains("://") && !trimmed.contains("www.") && !trimmed.starts_with("//") {
        // Don't treat URLs with known schemes as local files
        let known_schemes = [
            "data:",
            "javascript:",
            "mailto:",
            "ftp:",
            "tel:",
            "sms:",
            "http:",
            "https:",
        ];
        for scheme in &known_schemes {
            if trimmed.starts_with(scheme) {
                return false;
            }
        }

        // Check if it looks like a filename with a clear file extension
        if trimmed.contains('.') && !trimmed.contains(' ') {
            // Don't treat common domain patterns as files
            let common_tlds = ["com", "org", "net", "edu", "gov", "mil", "int", "io", "co"];
            let parts: Vec<&str> = trimmed.split('.').collect();

            // If it's a simple two-part name ending in a common TLD, it's probably a domain
            if parts.len() == 2 && common_tlds.contains(&parts[1]) {
                return false;
            }

            // If it has a clear file extension (common file extensions), treat as file
            let file_extensions = [
                "md", "txt", "json", "xml", "yaml", "yml", "toml", "ini", "cfg", "conf", "py",
                "rs", "js", "ts", "html", "css", "java", "cpp", "c", "h", "pdf", "doc", "docx",
                "png", "jpg", "jpeg", "gif", "svg",
            ];
            if let Some(extension) = parts.last() {
                if file_extensions.contains(extension) {
                    return true;
                }
            }

            // Additional checks to avoid false positives for domain names
            // Must not contain multiple dots in a row and should have reasonable structure
            if !trimmed.contains("..") && trimmed.matches('.').count() <= 2 {
                // If it doesn't end with a common TLD and has dots, might be a file
                if let Some(last_part) = parts.last() {
                    if !common_tlds.contains(last_part) {
                        return true;
                    }
                }
            }
        }

        // Accept well-known files without extensions (common in Unix-like systems)
        if !trimmed.contains('.') && !trimmed.contains(' ') && !trimmed.is_empty() {
            let known_files = [
                "Makefile",
                "README",
                "LICENSE",
                "CHANGELOG",
                "CONTRIBUTING",
                "Dockerfile",
                "Vagrantfile",
                "Cargo",
                "package",
            ];
            if known_files.contains(&trimmed) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_absolute_paths() {
        assert!(is_local_file_path("/path/to/file"));
        assert!(is_local_file_path("/usr/local/bin"));
        assert!(is_local_file_path("/file.txt"));
    }

    #[test]
    fn test_relative_paths() {
        assert!(is_local_file_path("./file.txt"));
        assert!(is_local_file_path("../parent/file.txt"));
        assert!(is_local_file_path("./"));
        assert!(is_local_file_path("../"));
    }

    #[test]
    fn test_windows_paths() {
        assert!(is_local_file_path("C:\\path\\to\\file"));
        assert!(is_local_file_path("D:/path/to/file"));
        assert!(is_local_file_path("Z:\\file.txt"));
    }

    #[test]
    fn test_relative_file_paths() {
        assert!(is_local_file_path("relative/path.txt"));
        assert!(is_local_file_path("docs/README.md"));
    }

    #[test]
    fn test_not_local_paths() {
        assert!(!is_local_file_path("https://example.com"));
        assert!(!is_local_file_path("http://localhost"));
        assert!(!is_local_file_path("ftp://server.com"));
        assert!(!is_local_file_path("www.example.com"));
        assert!(!is_local_file_path("example.com"));
        assert!(!is_local_file_path("//protocol-relative"));
        assert!(!is_local_file_path("data:text/html,<h1>Test</h1>"));
        assert!(!is_local_file_path("javascript:alert('xss')"));
    }

    #[test]
    fn test_file_urls() {
        assert!(is_local_file_path("file:///path/to/file"));
        assert!(is_local_file_path("file://./relative.md"));
        assert!(is_local_file_path("file://../parent.md"));
        assert!(is_local_file_path("file:///Users/user/doc.md"));
    }

    #[test]
    fn test_simple_relative_filenames() {
        // Should recognize common file extensions
        assert!(is_local_file_path("test.md"));
        assert!(is_local_file_path("document.txt"));
        assert!(is_local_file_path("README.md"));
        assert!(is_local_file_path("config.json"));
        assert!(is_local_file_path("script.py"));

        // Should recognize well-known files without extensions
        assert!(is_local_file_path("Makefile"));
        assert!(is_local_file_path("README"));
        assert!(is_local_file_path("LICENSE"));
        assert!(is_local_file_path("Dockerfile"));
    }

    #[test]
    fn test_domain_vs_file_distinction() {
        // Should NOT recognize domain names as files
        assert!(!is_local_file_path("example.com"));
        assert!(!is_local_file_path("github.com"));
        assert!(!is_local_file_path("docs.google.com"));
        assert!(!is_local_file_path("site.org"));
        assert!(!is_local_file_path("university.edu"));

        // Should still recognize legitimate files with common TLD-like extensions
        assert!(is_local_file_path("archive.com.txt")); // .txt extension makes it clear it's a file
        assert!(is_local_file_path("backup.org.json")); // .json extension makes it clear it's a file
    }

    #[test]
    fn test_edge_cases() {
        assert!(!is_local_file_path(""));
        assert!(!is_local_file_path("   "));

        // Simple words without extensions that look like domains should be rejected
        assert!(!is_local_file_path("simple"));
        assert!(!is_local_file_path("word"));
    }
}
