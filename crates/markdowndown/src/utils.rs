//! Utility functions shared across the codebase.

/// Known URL schemes that are not file paths.
const KNOWN_NON_FILE_SCHEMES: &[&str] = &[
    "data:",
    "javascript:",
    "mailto:",
    "ftp:",
    "tel:",
    "sms:",
    "http:",
    "https:",
];

/// Minimum length for Windows absolute paths (e.g., `C:\`).
const MIN_WINDOWS_PATH_LEN: usize = 3;

/// Index of the colon in Windows paths (e.g., `C:`).
const WINDOWS_DRIVE_COLON_INDEX: usize = 1;

/// Index of the path separator in Windows paths (e.g., `C:\` or `C:/`).
const WINDOWS_PATH_SEPARATOR_INDEX: usize = 2;

/// Number of parts in simple domains (e.g., `example.com`).
const SIMPLE_DOMAIN_PARTS: usize = 2;

/// Maximum number of dots allowed in filenames to avoid false positives with domain names.
/// Increased to 4 to handle multi-level subdomains like "api.staging.example.com"
const MAX_FILENAME_DOT_COUNT: usize = 4;

/// Checks if a string starts with a known non-file URL scheme.
fn has_url_scheme(input: &str) -> bool {
    KNOWN_NON_FILE_SCHEMES
        .iter()
        .any(|&scheme| input.starts_with(scheme))
}

/// Checks if a string contains URL-like patterns.
fn contains_url_patterns(input: &str) -> bool {
    input.contains("://")
        || input.contains("www.")
        || input.starts_with("//")
        || has_url_scheme(input)
}

/// Checks if a string is a Windows absolute path (e.g., C:\path or D:/path).
fn is_windows_absolute_path(s: &str) -> bool {
    s.len() >= MIN_WINDOWS_PATH_LEN
        && s.chars().nth(WINDOWS_DRIVE_COLON_INDEX) == Some(':')
        && matches!(
            s.chars().nth(WINDOWS_PATH_SEPARATOR_INDEX),
            Some('\\') | Some('/')
        )
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// Checks if a string looks like a valid TLD using heuristic rules.
///
/// Valid TLDs according to ICANN rules:
/// - Are 2-63 characters long
/// - Contain only ASCII letters, digits, and hyphens
/// - Do not start or end with a hyphen
/// - Are typically lowercase or can be case-normalized
///
/// This heuristic approach avoids maintaining a hard-coded list while
/// catching the vast majority of real TLDs and rejecting obvious non-TLDs.
fn looks_like_valid_tld(s: &str) -> bool {
    // TLDs must be between 2 and 63 characters (per RFC 1035)
    if s.len() < 2 || s.len() > 63 {
        return false;
    }

    // TLDs can only contain ASCII letters, digits, and hyphens
    // They cannot start or end with a hyphen
    let mut chars = s.chars();
    if matches!(chars.next(), Some('-')) || matches!(s.chars().last(), Some('-')) {
        return false;
    }

    // All characters must be alphanumeric or hyphens
    // At least one character should be a letter (pure numbers aren't valid TLDs)
    let has_letter = s.chars().any(|c| c.is_ascii_alphabetic());
    let all_valid = s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');

    all_valid && has_letter
}

/// Checks if a string ends with what looks like a valid top-level domain.
fn ends_with_valid_tld(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == SIMPLE_DOMAIN_PARTS && parts.last().is_some_and(|&tld| looks_like_valid_tld(tld))
}

/// Checks if a string is a well-known file without an extension.
/// Uses pattern matching for common file naming conventions.
fn is_well_known_file(s: &str) -> bool {
    // Early returns for basic requirements
    if s.is_empty() || s.contains('.') || s.contains(' ') {
        return false;
    }

    // Check for common patterns in file names:
    // 1. All caps names (common for documentation/configuration files)
    let all_caps_patterns = [
        "README",
        "LICENSE",
        "CHANGELOG",
        "CONTRIBUTING",
        "AUTHORS",
        "CODEOWNERS",
        "COPYING",
        "INSTALL",
        "NOTICE",
        "PATENTS",
        "SECURITY",
        "SUPPORT",
        "TODO",
        "VERSION",
    ];

    if all_caps_patterns
        .iter()
        .any(|&pattern| s.eq_ignore_ascii_case(pattern))
    {
        return true;
    }

    // 2. Files ending with "file" (case-insensitive)
    if s.len() >= 4 && s[s.len() - 4..].eq_ignore_ascii_case("file") {
        return true;
    }

    // 3. Files ending with "rc" (configuration files)
    if s.len() >= 2 && s[s.len() - 2..].eq_ignore_ascii_case("rc") {
        return true;
    }

    // 4. Common build/config file names (mixed case)
    let build_config_files = [
        "jenkinsfile",
        "rakefile",
        "gemfile",
        "podfile",
        "brewfile",
        "fastfile",
        "snapfile",
        "matchfile",
        "scanfile",
        "gymfile",
    ];

    build_config_files
        .iter()
        .any(|&pattern| s.eq_ignore_ascii_case(pattern))
}

/// Checks if a string looks like a domain name.
fn looks_like_domain(input: &str, parts: &[&str]) -> bool {
    // Reject obvious non-domains
    if input.contains("..") {
        return false;
    }

    // Domains typically have 2-3 parts (e.g., "example.com" or "docs.google.com")
    // More than 3 parts suggests a file path or subdomain structure that's too complex
    let dot_count = input.matches('.').count();
    if !(1..=MAX_FILENAME_DOT_COUNT).contains(&dot_count) {
        return false;
    }

    parts.last().is_some_and(|&last| looks_like_valid_tld(last))
}

/// Common file extensions that are clearly not TLDs.
const KNOWN_FILE_EXTENSIONS: &[&str] = &[
    "txt", "md", "json", "xml", "html", "htm", "css", "js", "ts", "jsx", "tsx", "py", "rs", "go",
    "java", "c", "cpp", "h", "hpp", "cs", "php", "rb", "swift", "kt", "scala", "sh", "bash", "zsh",
    "fish", "ps1", "bat", "cmd", "yml", "yaml", "toml", "ini", "cfg", "conf", "config", "log",
    "out", "err", "tmp", "temp", "bak", "backup", "zip", "tar", "gz", "bz2", "xz", "rar", "7z",
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "jpg", "jpeg", "png", "gif", "svg", "bmp",
    "ico", "webp", "mp3", "mp4", "wav", "avi", "mov", "mkv", "flac", "sql", "db", "sqlite", "mdb",
];

/// Checks if a string looks like a simple filename (without path separators).
fn looks_like_simple_filename(input: &str) -> bool {
    // Early return if it has URL-like patterns
    if contains_url_patterns(input) {
        return false;
    }

    // Check for well-known files without extensions
    if is_well_known_file(input) {
        return true;
    }

    // Must contain a dot and no spaces to be considered further
    if !input.contains('.') || input.contains(' ') {
        return false;
    }

    let parts: Vec<&str> = input.split('.').collect();

    // Check if the last part is a known file extension
    if let Some(&last_part) = parts.last() {
        // If it's a known file extension, it's definitely a file
        // This handles both simple cases like "test.md" and complex cases like "archive.com.txt"
        if KNOWN_FILE_EXTENSIONS
            .iter()
            .any(|&ext| last_part.eq_ignore_ascii_case(ext))
        {
            return true;
        }
    }

    // If it ends with a valid TLD and is a simple two-part name, it's probably a domain
    if ends_with_valid_tld(input) && parts.len() == SIMPLE_DOMAIN_PARTS {
        return false;
    }

    // Additional checks to avoid false positives for domain names
    !looks_like_domain(input, &parts)
}

/// Checks if a string is a path with separators but no protocol.
fn is_path_with_separators(trimmed: &str) -> bool {
    // Must not have a protocol and must have path separators
    if contains_url_patterns(trimmed) || !(trimmed.contains('/') || trimmed.contains('\\')) {
        return false;
    }

    // Return true if it looks like a path (not a domain pattern)
    trimmed.starts_with('.') || trimmed.contains('/') || trimmed.contains('\\')
}

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

    // Check for Windows-style absolute paths
    if is_windows_absolute_path(trimmed) {
        return true;
    }

    // Check for paths with separators but no protocol
    if is_path_with_separators(trimmed) {
        return true;
    }

    // Check for simple relative filenames
    looks_like_simple_filename(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to assert that all provided paths are recognized as local file paths.
    fn assert_paths_are_local(paths: &[&str]) {
        for path in paths {
            assert!(
                is_local_file_path(path),
                "Expected {} to be local path",
                path
            );
        }
    }

    #[test]
    fn test_unix_absolute_paths() {
        assert_paths_are_local(&["/path/to/file", "/usr/local/bin", "/file.txt"]);
    }

    #[test]
    fn test_relative_paths() {
        assert_paths_are_local(&["./file.txt", "../parent/file.txt", "./", "../"]);
    }

    #[test]
    fn test_windows_paths() {
        assert_paths_are_local(&["C:\\path\\to\\file", "D:/path/to/file", "Z:\\file.txt"]);
    }

    #[test]
    fn test_relative_file_paths() {
        assert_paths_are_local(&["relative/path.txt", "docs/README.md"]);
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
        assert_paths_are_local(&[
            "file:///path/to/file",
            "file://./relative.md",
            "file://../parent.md",
            "file:///Users/user/doc.md",
        ]);
    }

    #[test]
    fn test_simple_relative_filenames() {
        // Should recognize common file extensions
        assert_paths_are_local(&[
            "test.md",
            "document.txt",
            "README.md",
            "config.json",
            "script.py",
        ]);

        // Should recognize well-known files without extensions
        assert_paths_are_local(&["Makefile", "README", "LICENSE", "Dockerfile"]);
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
        assert_paths_are_local(&[
            "archive.com.txt", // .txt extension makes it clear it's a file
            "backup.org.json", // .json extension makes it clear it's a file
        ]);
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
