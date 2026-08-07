use tree_sitter::Language;

/// One programming language's row in the grammar roster: which files it claims,
/// which tree-sitter grammar parses them, and which node kinds carry meaning.
///
/// The roster is the single table every consumer of a parse reads — the entity
/// extractor, the complexity scorer, and the review probes all route a file to
/// its grammar through [`get_language_config`] rather than keeping a list of
/// their own. Adding a language is adding one config and one entry in the
/// roster it is looked up from.
#[allow(dead_code)]
#[derive(Debug)]
pub struct LanguageConfig {
    /// The language identifier (e.g. `"rust"`), reported as
    /// [`ParsedCode::language`](super::ParsedCode::language) and used as the
    /// per-thread parser cache key.
    pub id: &'static str,
    /// The file extensions this language claims, in the dotted-lowercase form
    /// [`dotted_lowercase_extension`] produces (e.g. `".rs"`).
    pub extensions: &'static [&'static str],
    /// The tree-sitter node kinds that are themselves entities — functions,
    /// types, modules — so the extractor reports one entity for each.
    pub entity_node_types: &'static [&'static str],
    /// The tree-sitter node kinds that hold entities without being one, such as
    /// a class or declaration body. The extractor descends through them to
    /// build an entity's parent path.
    pub container_node_types: &'static [&'static str],
    /// The call-target names that DEFINE an entity in languages where a
    /// definition is spelled as a call rather than as its own node kind (e.g.
    /// Elixir's `defmodule` and `def`). Empty for languages whose grammars give
    /// definitions a node kind of their own.
    pub call_entity_identifiers: &'static [&'static str],
    /// Loads the tree-sitter grammar for this language, `None` when it is
    /// unavailable.
    pub get_language: fn() -> Option<Language>,
}

fn get_typescript() -> Option<Language> {
    Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
}

fn get_tsx() -> Option<Language> {
    Some(tree_sitter_typescript::LANGUAGE_TSX.into())
}

fn get_javascript() -> Option<Language> {
    Some(tree_sitter_javascript::LANGUAGE.into())
}

fn get_python() -> Option<Language> {
    Some(tree_sitter_python::LANGUAGE.into())
}

fn get_go() -> Option<Language> {
    Some(tree_sitter_go::LANGUAGE.into())
}

fn get_rust() -> Option<Language> {
    Some(tree_sitter_rust::LANGUAGE.into())
}

fn get_java() -> Option<Language> {
    Some(tree_sitter_java::LANGUAGE.into())
}

fn get_c() -> Option<Language> {
    Some(tree_sitter_c::LANGUAGE.into())
}

fn get_cpp() -> Option<Language> {
    Some(tree_sitter_cpp::LANGUAGE.into())
}

fn get_ruby() -> Option<Language> {
    Some(tree_sitter_ruby::LANGUAGE.into())
}

fn get_csharp() -> Option<Language> {
    Some(tree_sitter_c_sharp::LANGUAGE.into())
}

fn get_php() -> Option<Language> {
    Some(tree_sitter_php::LANGUAGE_PHP.into())
}

fn get_fortran() -> Option<Language> {
    Some(tree_sitter_fortran::LANGUAGE.into())
}

fn get_swift() -> Option<Language> {
    Some(tree_sitter_swift::LANGUAGE.into())
}

fn get_elixir() -> Option<Language> {
    Some(tree_sitter_elixir::LANGUAGE.into())
}

fn get_bash() -> Option<Language> {
    Some(tree_sitter_bash::LANGUAGE.into())
}

/// The config of one TypeScript-family language.
///
/// TypeScript and TSX are the same language with one extra syntax, so the TSX
/// grammar reports the same node kinds under the same names. Both configs are
/// built here rather than written twice, so the two cannot drift: a node kind
/// added for TypeScript reaches TSX in the same edit.
const fn typescript_family_config(
    id: &'static str,
    extensions: &'static [&'static str],
    get_language: fn() -> Option<Language>,
) -> LanguageConfig {
    LanguageConfig {
        id,
        extensions,
        entity_node_types: &[
            "function_declaration",
            "class_declaration",
            "interface_declaration",
            "type_alias_declaration",
            "enum_declaration",
            "export_statement",
            "lexical_declaration",
            "variable_declaration",
            "method_definition",
            "public_field_definition",
        ],
        container_node_types: &["class_body", "interface_body", "enum_body"],
        call_entity_identifiers: &[],
        get_language,
    }
}

static TYPESCRIPT_CONFIG: LanguageConfig =
    typescript_family_config("typescript", &[".ts"], get_typescript);

static TSX_CONFIG: LanguageConfig = typescript_family_config("tsx", &[".tsx"], get_tsx);

static JAVASCRIPT_CONFIG: LanguageConfig = LanguageConfig {
    id: "javascript",
    extensions: &[".js", ".jsx", ".mjs", ".cjs"],
    entity_node_types: &[
        "function_declaration",
        "class_declaration",
        "export_statement",
        "lexical_declaration",
        "variable_declaration",
        "method_definition",
        "field_definition",
    ],
    container_node_types: &["class_body"],
    call_entity_identifiers: &[],
    get_language: get_javascript,
};

static PYTHON_CONFIG: LanguageConfig = LanguageConfig {
    id: "python",
    extensions: &[".py"],
    entity_node_types: &[
        "function_definition",
        "class_definition",
        "decorated_definition",
    ],
    container_node_types: &["block"],
    call_entity_identifiers: &[],
    get_language: get_python,
};

static GO_CONFIG: LanguageConfig = LanguageConfig {
    id: "go",
    extensions: &[".go"],
    entity_node_types: &[
        "function_declaration",
        "method_declaration",
        "type_declaration",
        "var_declaration",
        "const_declaration",
    ],
    container_node_types: &[],
    call_entity_identifiers: &[],
    get_language: get_go,
};

static RUST_CONFIG: LanguageConfig = LanguageConfig {
    id: "rust",
    extensions: &[".rs"],
    entity_node_types: &[
        "function_item",
        "struct_item",
        "enum_item",
        "impl_item",
        "trait_item",
        "mod_item",
        "const_item",
        "static_item",
        "type_item",
    ],
    container_node_types: &["declaration_list"],
    call_entity_identifiers: &[],
    get_language: get_rust,
};

static JAVA_CONFIG: LanguageConfig = LanguageConfig {
    id: "java",
    extensions: &[".java"],
    entity_node_types: &[
        "class_declaration",
        "method_declaration",
        "interface_declaration",
        "enum_declaration",
        "field_declaration",
        "constructor_declaration",
        "annotation_type_declaration",
    ],
    container_node_types: &["class_body", "interface_body", "enum_body"],
    call_entity_identifiers: &[],
    get_language: get_java,
};

static C_CONFIG: LanguageConfig = LanguageConfig {
    id: "c",
    extensions: &[".c", ".h"],
    entity_node_types: &[
        "function_definition",
        "struct_specifier",
        "enum_specifier",
        "union_specifier",
        "type_definition",
        "declaration",
    ],
    container_node_types: &[],
    call_entity_identifiers: &[],
    get_language: get_c,
};

static CPP_CONFIG: LanguageConfig = LanguageConfig {
    id: "cpp",
    extensions: &[".cpp", ".cc", ".cxx", ".hpp", ".hh", ".hxx"],
    entity_node_types: &[
        "function_definition",
        "class_specifier",
        "struct_specifier",
        "enum_specifier",
        "namespace_definition",
        "template_declaration",
        "declaration",
        "type_definition",
    ],
    container_node_types: &["field_declaration_list", "declaration_list"],
    call_entity_identifiers: &[],
    get_language: get_cpp,
};

static RUBY_CONFIG: LanguageConfig = LanguageConfig {
    id: "ruby",
    extensions: &[".rb"],
    entity_node_types: &["method", "singleton_method", "class", "module"],
    container_node_types: &["body_statement"],
    call_entity_identifiers: &[],
    get_language: get_ruby,
};

static CSHARP_CONFIG: LanguageConfig = LanguageConfig {
    id: "csharp",
    extensions: &[".cs"],
    entity_node_types: &[
        "method_declaration",
        "class_declaration",
        "interface_declaration",
        "enum_declaration",
        "struct_declaration",
        "namespace_declaration",
        "property_declaration",
        "constructor_declaration",
        "field_declaration",
    ],
    container_node_types: &["declaration_list"],
    call_entity_identifiers: &[],
    get_language: get_csharp,
};

static PHP_CONFIG: LanguageConfig = LanguageConfig {
    id: "php",
    extensions: &[".php"],
    entity_node_types: &[
        "function_definition",
        "class_declaration",
        "method_declaration",
        "interface_declaration",
        "trait_declaration",
        "enum_declaration",
        "namespace_definition",
    ],
    container_node_types: &["declaration_list", "enum_declaration_list"],
    call_entity_identifiers: &[],
    get_language: get_php,
};

static FORTRAN_CONFIG: LanguageConfig = LanguageConfig {
    id: "fortran",
    extensions: &[".f90", ".f95", ".f03", ".f08", ".f", ".for"],
    entity_node_types: &[
        "function",
        "subroutine",
        "module",
        "program",
        "interface",
        "type_declaration",
    ],
    container_node_types: &[],
    call_entity_identifiers: &[],
    get_language: get_fortran,
};

static SWIFT_CONFIG: LanguageConfig = LanguageConfig {
    id: "swift",
    extensions: &[".swift"],
    entity_node_types: &[
        "function_declaration",
        "class_declaration",
        "protocol_declaration",
        "init_declaration",
        "deinit_declaration",
        "subscript_declaration",
        "typealias_declaration",
        "property_declaration",
        "operator_declaration",
        "associatedtype_declaration",
    ],
    container_node_types: &["class_body", "protocol_body", "enum_class_body"],
    call_entity_identifiers: &[],
    get_language: get_swift,
};

static ELIXIR_CONFIG: LanguageConfig = LanguageConfig {
    id: "elixir",
    extensions: &[".ex", ".exs"],
    entity_node_types: &[],
    container_node_types: &["do_block"],
    call_entity_identifiers: &[
        "defmodule",
        "def",
        "defp",
        "defmacro",
        "defmacrop",
        "defguard",
        "defguardp",
        "defprotocol",
        "defimpl",
        "defstruct",
        "defexception",
        "defdelegate",
    ],
    get_language: get_elixir,
};

static BASH_CONFIG: LanguageConfig = LanguageConfig {
    id: "bash",
    extensions: &[".sh"],
    entity_node_types: &["function_definition"],
    container_node_types: &[],
    call_entity_identifiers: &[],
    get_language: get_bash,
};

static ALL_CONFIGS: &[&LanguageConfig] = &[
    &TYPESCRIPT_CONFIG,
    &TSX_CONFIG,
    &JAVASCRIPT_CONFIG,
    &PYTHON_CONFIG,
    &GO_CONFIG,
    &RUST_CONFIG,
    &JAVA_CONFIG,
    &C_CONFIG,
    &CPP_CONFIG,
    &RUBY_CONFIG,
    &CSHARP_CONFIG,
    &PHP_CONFIG,
    &FORTRAN_CONFIG,
    &SWIFT_CONFIG,
    &ELIXIR_CONFIG,
    &BASH_CONFIG,
];

/// The roster entry that claims `extension`, `None` when no language does.
///
/// `extension` must be in the dotted-lowercase form
/// [`dotted_lowercase_extension`] produces (e.g. `".rs"`); an extension spelled
/// any other way matches nothing. `None` means **the roster has no grammar for
/// this file**, so a caller reports "not computed" rather than an empty result.
pub fn get_language_config(extension: &str) -> Option<&'static LanguageConfig> {
    ALL_CONFIGS
        .iter()
        .find(|c| c.extensions.contains(&extension))
        .copied()
}

/// All file extensions the code parser plugin handles, in the canonical
/// dotted-lowercase form (e.g. `".rs"`). The single source of truth other
/// crates reuse instead of keeping their own extension lists.
pub fn get_all_code_extensions() -> &'static [&'static str] {
    // All unique extensions across all language configs
    static EXTENSIONS: &[&str] = &[
        ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".py", ".go", ".rs", ".java", ".c", ".h",
        ".cpp", ".cc", ".cxx", ".hpp", ".hh", ".hxx", ".rb", ".cs", ".php", ".f90", ".f95", ".f03",
        ".f08", ".f", ".for", ".swift", ".ex", ".exs", ".sh",
    ];
    EXTENSIONS
}

/// The dotted, lowercased extension of `path` (e.g. `".RS"` → `".rs"`), `None`
/// when the path carries no UTF-8 extension.
///
/// This is the normalization convention the extension lists in this module are
/// keyed by — every lookup against [`get_all_code_extensions`] or
/// [`get_language_config`] must go through it, so the convention has exactly
/// one owner.
pub fn dotted_lowercase_extension(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path).extension()?.to_str()?;
    Some(format!(".{}", ext.to_lowercase()))
}

/// Whether `path` has a code extension per [`get_all_code_extensions`].
///
/// Lives next to the list it interprets so callers (e.g. review scoping in
/// other crates) never re-encode the dotted-lowercase convention themselves.
pub fn is_code_file(path: &str) -> bool {
    dotted_lowercase_extension(path)
        .is_some_and(|ext| get_all_code_extensions().contains(&ext.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_code_file_accepts_known_code_extensions() {
        assert!(is_code_file("src/new.rs"));
        assert!(is_code_file("app/main.py"));
        assert!(is_code_file("deep/nested/dir/events.js"));
    }

    #[test]
    fn is_code_file_normalizes_case_to_the_dotted_lowercase_convention() {
        assert!(is_code_file("SRC/MAIN.RS"));
        assert!(is_code_file("Lib.Swift"));
    }

    #[test]
    fn is_code_file_rejects_non_code_and_extensionless_paths() {
        assert!(!is_code_file("logs/run.log"));
        assert!(!is_code_file("data.jsonl"));
        assert!(!is_code_file("Makefile"));
        assert!(!is_code_file("notes.txt"));
    }

    #[test]
    fn dotted_lowercase_extension_matches_the_list_entry_format() {
        assert_eq!(dotted_lowercase_extension("a/b.RS").as_deref(), Some(".rs"));
        assert_eq!(dotted_lowercase_extension("Makefile"), None);
    }
}
