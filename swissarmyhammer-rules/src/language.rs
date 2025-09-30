//! Language detection from file paths and content.
//!
//! This module provides functionality to detect programming languages
//! based on file extensions, which is used by the rule checker to provide
//! language context to rules.

use std::path::Path;

use crate::Result;

/// Detect programming language from file path and content.
///
/// Uses file extension mapping to determine the language. Returns "unknown"
/// for unrecognized extensions.
///
/// # Arguments
///
/// * `path` - The file path to detect language for
/// * `_content` - The file content (currently unused, reserved for future content-based detection)
///
/// # Returns
///
/// Returns the detected language name (e.g., "rust", "python", "javascript")
/// or "unknown" if the language cannot be determined.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use swissarmyhammer_rules::detect_language;
///
/// let language = detect_language(Path::new("main.rs"), "").unwrap();
/// assert_eq!(language, "rust");
///
/// let language = detect_language(Path::new("script.py"), "").unwrap();
/// assert_eq!(language, "python");
///
/// let language = detect_language(Path::new("data.xyz"), "").unwrap();
/// assert_eq!(language, "unknown");
/// ```
pub fn detect_language(path: &Path, _content: &str) -> Result<String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown");

    // Map extensions to language names
    // Supports all languages that tree-sitter knows about
    let language = match extension {
        // Programming languages
        "rs" => "rust",
        "py" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescript",
        "jsx" => "javascript",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" => "cpp",
        "c" | "h" => "c",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "dart" => "dart",
        "scala" => "scala",
        "r" | "R" => "r",
        "lua" => "lua",
        "pl" | "pm" => "perl",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "clj" | "cljs" | "cljc" => "clojure",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "fs" | "fsi" | "fsx" => "fsharp",
        "v" => "verilog",
        "vhd" | "vhdl" => "vhdl",
        "zig" => "zig",
        "nim" => "nim",
        "cr" => "crystal",
        "d" => "d",
        "pas" => "pascal",
        "ada" | "adb" | "ads" => "ada",
        "f" | "f90" | "f95" | "f03" => "fortran",
        "cob" | "cbl" => "cobol",

        // Scripting and shell
        "sh" | "bash" | "zsh" => "shell",
        "fish" => "fish",
        "ps1" => "powershell",
        "bat" | "cmd" => "batch",

        // Markup and data formats
        "html" | "htm" => "html",
        "xml" => "xml",
        "css" => "css",
        "scss" | "sass" => "scss",
        "less" => "less",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "jsonc" => "json",
        "md" | "markdown" => "markdown",
        "rst" => "restructuredtext",
        "tex" => "latex",
        "sql" => "sql",
        "graphql" | "gql" => "graphql",
        "proto" => "protobuf",

        // Configuration
        "ini" | "cfg" => "ini",
        "conf" => "config",
        "properties" => "properties",
        "env" => "env",

        // Other
        "vim" => "vim",
        "diff" | "patch" => "diff",
        "dockerfile" => "dockerfile",
        "makefile" => "make",

        // Default
        _ => "unknown",
    };

    Ok(language.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_rust_detection() {
        assert_eq!(detect_language(Path::new("main.rs"), "").unwrap(), "rust");
        assert_eq!(
            detect_language(Path::new("src/lib.rs"), "").unwrap(),
            "rust"
        );
    }

    #[test]
    fn test_python_detection() {
        assert_eq!(
            detect_language(Path::new("script.py"), "").unwrap(),
            "python"
        );
        assert_eq!(
            detect_language(Path::new("app/main.py"), "").unwrap(),
            "python"
        );
    }

    #[test]
    fn test_javascript_detection() {
        assert_eq!(
            detect_language(Path::new("app.js"), "").unwrap(),
            "javascript"
        );
        assert_eq!(
            detect_language(Path::new("app.mjs"), "").unwrap(),
            "javascript"
        );
        assert_eq!(
            detect_language(Path::new("app.cjs"), "").unwrap(),
            "javascript"
        );
        assert_eq!(
            detect_language(Path::new("App.jsx"), "").unwrap(),
            "javascript"
        );
    }

    #[test]
    fn test_typescript_detection() {
        assert_eq!(
            detect_language(Path::new("app.ts"), "").unwrap(),
            "typescript"
        );
        assert_eq!(
            detect_language(Path::new("app.mts"), "").unwrap(),
            "typescript"
        );
        assert_eq!(
            detect_language(Path::new("app.cts"), "").unwrap(),
            "typescript"
        );
        assert_eq!(
            detect_language(Path::new("App.tsx"), "").unwrap(),
            "typescript"
        );
    }

    #[test]
    fn test_go_detection() {
        assert_eq!(detect_language(Path::new("main.go"), "").unwrap(), "go");
    }

    #[test]
    fn test_java_detection() {
        assert_eq!(detect_language(Path::new("Main.java"), "").unwrap(), "java");
    }

    #[test]
    fn test_cpp_detection() {
        assert_eq!(detect_language(Path::new("main.cpp"), "").unwrap(), "cpp");
        assert_eq!(detect_language(Path::new("main.cc"), "").unwrap(), "cpp");
        assert_eq!(detect_language(Path::new("main.cxx"), "").unwrap(), "cpp");
        assert_eq!(detect_language(Path::new("main.hpp"), "").unwrap(), "cpp");
    }

    #[test]
    fn test_c_detection() {
        assert_eq!(detect_language(Path::new("main.c"), "").unwrap(), "c");
        assert_eq!(detect_language(Path::new("main.h"), "").unwrap(), "c");
    }

    #[test]
    fn test_dart_detection() {
        assert_eq!(detect_language(Path::new("main.dart"), "").unwrap(), "dart");
    }

    #[test]
    fn test_ruby_detection() {
        assert_eq!(detect_language(Path::new("app.rb"), "").unwrap(), "ruby");
    }

    #[test]
    fn test_shell_detection() {
        assert_eq!(
            detect_language(Path::new("script.sh"), "").unwrap(),
            "shell"
        );
        assert_eq!(
            detect_language(Path::new("script.bash"), "").unwrap(),
            "shell"
        );
        assert_eq!(
            detect_language(Path::new("script.zsh"), "").unwrap(),
            "shell"
        );
    }

    #[test]
    fn test_markup_detection() {
        assert_eq!(detect_language(Path::new("page.html"), "").unwrap(), "html");
        assert_eq!(detect_language(Path::new("data.xml"), "").unwrap(), "xml");
        assert_eq!(detect_language(Path::new("style.css"), "").unwrap(), "css");
        assert_eq!(
            detect_language(Path::new("style.scss"), "").unwrap(),
            "scss"
        );
    }

    #[test]
    fn test_data_format_detection() {
        assert_eq!(
            detect_language(Path::new("config.toml"), "").unwrap(),
            "toml"
        );
        assert_eq!(
            detect_language(Path::new("config.yaml"), "").unwrap(),
            "yaml"
        );
        assert_eq!(
            detect_language(Path::new("config.yml"), "").unwrap(),
            "yaml"
        );
        assert_eq!(detect_language(Path::new("data.json"), "").unwrap(), "json");
        assert_eq!(
            detect_language(Path::new("README.md"), "").unwrap(),
            "markdown"
        );
    }

    #[test]
    fn test_unknown_extension() {
        assert_eq!(
            detect_language(Path::new("file.xyz"), "").unwrap(),
            "unknown"
        );
        assert_eq!(
            detect_language(Path::new("file.unknown"), "").unwrap(),
            "unknown"
        );
    }

    #[test]
    fn test_no_extension() {
        assert_eq!(
            detect_language(Path::new("Makefile"), "").unwrap(),
            "unknown"
        );
        assert_eq!(detect_language(Path::new("README"), "").unwrap(), "unknown");
    }

    #[test]
    fn test_more_languages() {
        assert_eq!(detect_language(Path::new("app.kt"), "").unwrap(), "kotlin");
        assert_eq!(detect_language(Path::new("app.cs"), "").unwrap(), "csharp");
        assert_eq!(detect_language(Path::new("app.php"), "").unwrap(), "php");
        assert_eq!(
            detect_language(Path::new("app.swift"), "").unwrap(),
            "swift"
        );
        assert_eq!(
            detect_language(Path::new("app.scala"), "").unwrap(),
            "scala"
        );
        assert_eq!(detect_language(Path::new("app.lua"), "").unwrap(), "lua");
        assert_eq!(detect_language(Path::new("app.hs"), "").unwrap(), "haskell");
        assert_eq!(detect_language(Path::new("app.zig"), "").unwrap(), "zig");
        assert_eq!(detect_language(Path::new("query.sql"), "").unwrap(), "sql");
    }
}
