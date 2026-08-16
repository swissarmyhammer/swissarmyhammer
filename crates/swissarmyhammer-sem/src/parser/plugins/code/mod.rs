//! Tree-sitter code plugins: entity extraction, function definitions, the test
//! census, duplication, commented code and public surface.
//!
//! # No shared tree-sitter helper module: the decision, not an omission
//!
//! Each plugin here keeps its own small tree-sitter helpers. Several of them
//! carry the SAME NAME in more than one file. A `similar` probe scores those
//! pairs high, because the probe measures shape. The shape is alike and the
//! contract is not. The `reuse` rule carves out exactly this case: "A `similar`
//! candidate that only *looks* alike (same shape, different domain or contract)
//! is not a reuse miss."
//!
//! Read the contract before you report one of these pairs.
//!
//! `node_text` has four copies and four different contracts:
//!
//! | site | signature | answer when the text is absent |
//! |---|---|---|
//! | `definitions::node_text` | `(Node, &str) -> Option<&str>` | `None` |
//! | `duplication::node_text` | `(Node, &str) -> &str` | `""` |
//! | `entity_extractor::node_text` | `(Node, &[u8]) -> &str` | `""`, through `utf8_text` |
//! | `swissarmyhammer_treesitter::ParsedFile::node_text` | a method, `-> Option<&str>` | `None` |
//!
//! The `Option` and the `""` are not two spellings of one answer. `definitions`
//! compares the text against test markers, so `""` there would read as "this is
//! not a test" and would hide a test from the census. `duplication`
//! must still hash and compare a chunk whose slice it cannot read, so `""` is
//! the answer it needs. `entity_extractor` takes bytes and validates UTF-8;
//! the two `&str` copies slice text that is already valid UTF-8 and can miss on
//! a codepoint boundary instead. One contract cannot serve all four sites.
//!
//! `spec_for_language` has four copies. Each reads a DIFFERENT static table of a
//! DIFFERENT type: `DEFINITION_SPECS`/`DefinitionSpec`, `COMMENT_SPECS`/`CommentSpec`,
//! `SURFACE_SPECS`/`SurfaceSpec`, and `LANGUAGE_SPECS`/`LanguageSpec`. Two
//! tables hold references and two hold values. Each body is one
//! `.iter().find()` line. A shared version needs a trait, four impls of it, and
//! a generic function, to replace four lines. That moves code and adds to it.
//! The `duplication/rust` rule states the test this fails: "Do not flag this
//! unless a further shared abstraction would strictly reduce the code (not just
//! relocate it)".
//!
//! `is_test_definition` has two copies that share a name and nothing else. The
//! `duplication` copy takes a `TestSpec` and ORs four `marked_by_*` helpers. The
//! `definitions` copy takes a `DefinitionSpec` and reads a name, then a defining
//! call's target, then attributes.
//!
//! Three more named counterparts sit in OTHER crates —
//! `swissarmyhammer-treesitter`, `swissarmyhammer-templating` — and one is a
//! TEST file in `swissarmyhammer-tools`. A helper for these plugins cannot live
//! in any of them.
//!
//! Recorded on 2026-08-14 for ^4dyewvd, against the seven pairs a `review file`
//! run raised on `complexity.rs`, whose surviving half is now `definitions.rs`.

mod commented_code;
mod definitions;
mod duplication;
mod entity_extractor;
mod languages;
mod public_surface;
mod test_census;

/// All file extensions the code parser handles, in the canonical
/// dotted-lowercase form (e.g. `".rs"`) — the single extension list other
/// crates reuse instead of keeping their own.
pub use languages::get_all_code_extensions;
/// Whether a path has a code extension per [`get_all_code_extensions`] — the
/// predicate that owns the dotted-lowercase matching convention.
pub use languages::is_code_file;

/// Where a file hides code inside its comments, so a reviewer reads blocks
/// instead of skimming for them, computed by the `commented_code` module.
pub use commented_code::{commented_code_blocks, commented_code_extensions, CommentedCodeBlock};
/// The tokens and the exemptions one file contributes to the verbatim
/// duplicate gate, so the detector pairs code and never pairs test code. See
/// [`duplication`].
pub use duplication::{
    duplication_source, DuplicationDefinition, DuplicationSource, DUPLICATION_ALLOW_MARKER,
};
/// What a change did to a file's public surface — declarations added, removed,
/// re-spelled, or given a different visibility — so a reviewer reads rows
/// instead of comparing declarations by eye, computed by the `public_surface`
/// module.
pub use public_surface::{
    PublicSurface, SurfaceChange, SurfaceChangeKind, SurfaceSymbol, Visibility,
};
/// What each test function in a file actually measures — zero assertions, a
/// skip marker, an empty body — so a reviewer reads rows instead of counting
/// assertion calls by eye, computed by the `test_census` module.
pub use test_census::{test_census, TestCensus, TestDefect};

use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::model::entity::SemanticEntity;
use crate::parser::plugin::SemanticParserPlugin;
use entity_extractor::extract_entities;
use languages::{dotted_lowercase_extension, get_language_config, LanguageConfig};

/// Semantic parser plugin that extracts entities (functions, classes, traits,
/// modules, ...) from source code via tree-sitter, covering every language
/// registered in `languages`. Implements [`SemanticParserPlugin`] for the
/// extensions reported by [`get_all_code_extensions`].
pub struct CodeParserPlugin;

// Thread-local parser cache: one Parser per language per thread.
// Avoids creating a new Parser for every file during parallel graph builds.
thread_local! {
    static PARSER_CACHE: RefCell<HashMap<&'static str, tree_sitter::Parser>> = RefCell::new(HashMap::new());
}

impl SemanticParserPlugin for CodeParserPlugin {
    fn id(&self) -> &str {
        "code"
    }

    fn extensions(&self) -> &[&str] {
        get_all_code_extensions()
    }

    fn extract_entities(&self, content: &str, file_path: &str) -> Vec<SemanticEntity> {
        match parse_code(file_path, content) {
            Some(parsed) => parsed.entities(file_path, content),
            None => Vec::new(),
        }
    }
}

/// One source file parsed by the code plugin's tree-sitter grammars.
///
/// Carries the parse tree together with the language config the roster
/// routed the file to, so everything computed from a parse — entities,
/// definitions, review probes — reads the SAME grammar table rather than
/// keeping a roster of its own.
///
/// Cloning is cheap: a `tree_sitter::Tree` is reference counted.
#[derive(Debug, Clone)]
pub struct ParsedCode {
    config: &'static LanguageConfig,
    tree: tree_sitter::Tree,
}

impl ParsedCode {
    /// The language id the roster routed the file to (e.g. `"rust"`).
    pub fn language(&self) -> &'static str {
        self.config.id
    }

    /// The tree-sitter parse tree.
    pub fn tree(&self) -> &tree_sitter::Tree {
        &self.tree
    }

    /// The semantic entities (functions, methods, types, modules, ...) this
    /// parse defines.
    ///
    /// The same extraction [`CodeParserPlugin`] performs, run against a parse
    /// the caller already holds — so a caller that needs both a tree and its
    /// entities parses the file once, not twice.
    ///
    /// `file_path` and `source` must be the ones the parse was made from; they
    /// name the entity ids and supply the text the node ranges index into.
    pub fn entities(&self, file_path: &str, source: &str) -> Vec<SemanticEntity> {
        extract_entities(&self.tree, file_path, self.config, source)
    }

    /// Every declaration this parse holds, read for what it takes to tell an
    /// API change from a body edit: the declaration's signature and whether it
    /// reaches outside the file.
    ///
    /// Returns `None` — meaning **not measured** — when the parse's language has
    /// no visibility mapping. A caller must report "not computed" for `None` and
    /// never substitute an empty surface, which would read as "this file
    /// declares nothing public".
    ///
    /// `file_path` and `source` must be the ones the parse was made from; they
    /// name the entity ids and supply the text the node ranges index into.
    pub fn public_surface(&self, file_path: &str, source: &str) -> Option<PublicSurface> {
        public_surface::read(&self.tree, file_path, self.config, source)
    }
}

/// Parse `source` with the tree-sitter grammar the code plugin routes `path` to.
///
/// This is the one entry point into the grammar roster: every consumer that
/// needs a parse calls it instead of building a `tree_sitter::Parser` and
/// picking a language itself.
///
/// Returns `None` — meaning **not parsed** — when the path carries no extension
/// the roster maps, when the grammar fails to load, or when tree-sitter
/// produces no tree. A caller must report "not computed" for `None` and never
/// substitute an empty result, which would silently read as "nothing found".
///
/// The parser comes from a per-thread cache, so repeated calls for one language
/// build the parser once.
///
/// # Examples
///
/// ```
/// use swissarmyhammer_sem::parser::plugins::code::parse_code;
///
/// let parsed = parse_code("src/lib.rs", "fn one() {}\n").ok_or("rust is mapped")?;
/// assert_eq!(parsed.language(), "rust");
/// assert!(parse_code("notes.txt", "plain text\n").is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_code(path: &str, source: &str) -> Option<ParsedCode> {
    let extension = dotted_lowercase_extension(path)?;
    let config = get_language_config(&extension)?;
    let language = (config.get_language)()?;

    PARSER_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let parser = match cache.entry(config.id) {
            Entry::Occupied(occupied) => occupied.into_mut(),
            Entry::Vacant(vacant) => {
                let mut parser = tree_sitter::Parser::new();
                // Only cache a successfully configured parser: caching a
                // language-less parser after a `set_language` failure would
                // permanently pin it for this thread and every later file
                // of this language would silently parse to nothing.
                if let Err(error) = parser.set_language(&language) {
                    tracing::warn!(
                        language = config.id,
                        error = %error,
                        "failed to set tree-sitter language; skipping parse"
                    );
                    return None;
                }
                vacant.insert(parser)
            }
        };

        let tree = parser.parse(source.as_bytes(), None)?;
        Some(ParsedCode { config, tree })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_code_routes_a_mapped_extension_to_its_grammar() {
        let parsed = parse_code("src/lib.rs", "fn one() {}\n").expect("rust is in the roster");
        assert_eq!(parsed.language(), "rust");
        assert_eq!(parsed.tree().root_node().kind(), "source_file");
    }

    /// The roster is keyed by the dotted-LOWERCASE extension, and a repository
    /// carries files spelled `.RS` or `.PY`. `parse_code` normalizes before it
    /// looks up, so an uppercase path must reach the same grammar; without the
    /// normalization the lookup misses and the file reads as "not parsed".
    #[test]
    fn parse_code_routes_an_uppercase_extension_to_the_same_grammar() {
        let parsed = parse_code("src/lib.RS", "fn one() {}\n").expect("rust is in the roster");
        assert_eq!(parsed.language(), "rust");

        let mixed =
            parse_code("app/Main.Py", "def one():\n    pass\n").expect("python is in the roster");
        assert_eq!(mixed.language(), "python");
    }

    #[test]
    fn parse_code_returns_none_for_an_extension_the_roster_does_not_map() {
        assert!(parse_code("notes.txt", "plain text\n").is_none());
        assert!(parse_code("Makefile", "all:\n").is_none());
    }

    #[test]
    fn parsed_code_entities_match_the_plugin_extraction_of_the_same_source() {
        let source = "fn one() {}\nstruct Two;\nfn three() {}\n";
        let parsed = parse_code("src/lib.rs", source).expect("rust is in the roster");

        let from_parse: Vec<String> = parsed
            .entities("src/lib.rs", source)
            .into_iter()
            .map(|entity| entity.name)
            .collect();
        let from_plugin: Vec<String> = CodeParserPlugin
            .extract_entities(source, "src/lib.rs")
            .into_iter()
            .map(|entity| entity.name)
            .collect();

        assert_eq!(from_parse, from_plugin);
        assert_eq!(from_parse, vec!["one", "Two", "three"]);
    }

    #[test]
    fn test_java_entity_extraction() {
        let code = r#"
package com.example;

import java.util.List;

public class UserService {
    private String name;

    public UserService(String name) {
        this.name = name;
    }

    public List<User> getUsers() {
        return db.findAll();
    }

    public void createUser(User user) {
        db.save(user);
    }
}

interface Repository<T> {
    T findById(String id);
    List<T> findAll();
}

enum Status {
    ACTIVE,
    INACTIVE,
    DELETED
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "UserService.java");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Java entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"UserService"),
            "Should find class UserService, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Repository"),
            "Should find interface Repository, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Status"),
            "Should find enum Status, got: {:?}",
            names
        );
    }

    #[test]
    fn test_java_nested_methods() {
        let code = r#"
public class Calculator {
    public int add(int a, int b) {
        return a + b;
    }

    public int subtract(int a, int b) {
        return a - b;
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "Calculator.java");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Java nested: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Calculator"),
            "Should find Calculator class"
        );
        assert!(
            names.contains(&"add"),
            "Should find add method, got: {:?}",
            names
        );
        assert!(
            names.contains(&"subtract"),
            "Should find subtract method, got: {:?}",
            names
        );

        // Methods should have Calculator as parent
        let add = entities.iter().find(|e| e.name == "add").unwrap();
        assert!(add.parent_id.is_some(), "add should have parent_id");
    }

    #[test]
    fn test_c_entity_extraction() {
        let code = r#"
#include <stdio.h>

struct Point {
    int x;
    int y;
};

enum Color {
    RED,
    GREEN,
    BLUE
};

typedef struct {
    char name[50];
    int age;
} Person;

void greet(const char* name) {
    printf("Hello, %s!\n", name);
}

int add(int a, int b) {
    return a + b;
}

int main() {
    greet("world");
    return 0;
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "main.c");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "C entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"greet"),
            "Should find greet function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"add"),
            "Should find add function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"main"),
            "Should find main function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Point"),
            "Should find Point struct, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Color"),
            "Should find Color enum, got: {:?}",
            names
        );
    }

    #[test]
    fn test_cpp_entity_extraction() {
        let code = "namespace math {\nclass Vector3 {\npublic:\n    float length() const { return 0; }\n};\n}\nvoid greet() {}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "main.cpp");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"math"), "got: {:?}", names);
        assert!(names.contains(&"Vector3"), "got: {:?}", names);
        assert!(names.contains(&"greet"), "got: {:?}", names);
    }

    #[test]
    fn test_ruby_entity_extraction() {
        let code = "module Auth\n  class User\n    def greet\n      \"hi\"\n    end\n  end\nend\ndef helper(x)\n  x * 2\nend\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "auth.rb");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Auth"), "got: {:?}", names);
        assert!(names.contains(&"User"), "got: {:?}", names);
        assert!(names.contains(&"helper"), "got: {:?}", names);
    }

    #[test]
    fn test_csharp_entity_extraction() {
        let code = "namespace MyApp {\npublic class User {\n    public string GetName() { return \"\"; }\n}\npublic enum Role { Admin, User }\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "Models.cs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"MyApp"), "got: {:?}", names);
        assert!(names.contains(&"User"), "got: {:?}", names);
        assert!(names.contains(&"Role"), "got: {:?}", names);
    }

    #[test]
    fn test_swift_entity_extraction() {
        let code = r#"
import Foundation

class UserService {
    var name: String

    init(name: String) {
        self.name = name
    }

    func getUsers() -> [User] {
        return db.findAll()
    }
}

struct Point {
    var x: Double
    var y: Double
}

enum Status {
    case active
    case inactive
    case deleted
}

protocol Repository {
    associatedtype Item
    func findById(id: String) -> Item?
    func findAll() -> [Item]
}

func helper(x: Int) -> Int {
    return x * 2
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "UserService.swift");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Swift entities: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"UserService"),
            "Should find class UserService, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Point"),
            "Should find struct Point, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Status"),
            "Should find enum Status, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Repository"),
            "Should find protocol Repository, got: {:?}",
            names
        );
        assert!(
            names.contains(&"helper"),
            "Should find function helper, got: {:?}",
            names
        );
    }

    #[test]
    fn test_elixir_entity_extraction() {
        let code = r#"
defmodule MyApp.Accounts do
  def create_user(attrs) do
    %User{}
    |> User.changeset(attrs)
    |> Repo.insert()
  end

  defp validate(attrs) do
    # private helper
    :ok
  end

  defmacro is_admin(user) do
    quote do
      unquote(user).role == :admin
    end
  end

  defguard is_positive(x) when is_integer(x) and x > 0
end

defprotocol Printable do
  def to_string(data)
end

defimpl Printable, for: Integer do
  def to_string(i), do: Integer.to_string(i)
end
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "accounts.ex");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Elixir entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"MyApp.Accounts"),
            "Should find module, got: {:?}",
            names
        );
        assert!(
            names.contains(&"create_user"),
            "Should find def, got: {:?}",
            names
        );
        assert!(
            names.contains(&"validate"),
            "Should find defp, got: {:?}",
            names
        );
        assert!(
            names.contains(&"is_admin"),
            "Should find defmacro, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Printable"),
            "Should find defprotocol, got: {:?}",
            names
        );

        // Verify nesting: create_user should have MyApp.Accounts as parent
        let create_user = entities.iter().find(|e| e.name == "create_user").unwrap();
        assert!(
            create_user.parent_id.is_some(),
            "create_user should be nested under module"
        );
    }

    #[test]
    fn test_bash_entity_extraction() {
        let code = r#"#!/bin/bash

greet() {
    echo "Hello, $1!"
}

function deploy {
    echo "deploying..."
}

# not a function
echo "main script"
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "deploy.sh");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Bash entities: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"greet"),
            "Should find greet(), got: {:?}",
            names
        );
        assert!(
            names.contains(&"deploy"),
            "Should find function deploy, got: {:?}",
            names
        );
        assert_eq!(
            entities.len(),
            2,
            "Should only find functions, got: {:?}",
            names
        );
    }

    #[test]
    fn test_typescript_entity_extraction() {
        // Existing language should still work
        let code = r#"
export function hello(): string {
    return "hello";
}

export class Greeter {
    greet(name: string): string {
        return `Hello, ${name}!`;
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "test.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello"), "Should find hello function");
        assert!(names.contains(&"Greeter"), "Should find Greeter class");
    }

    #[test]
    fn test_typescript_class_with_methods() {
        // Tests class/method extraction and nested parent_id assignment
        let code = r#"
class Animal {
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    speak(): string {
        return `${this.name} makes a sound.`;
    }

    static create(name: string): Animal {
        return new Animal(name);
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "animal.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "TS class+methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Animal"),
            "Should find Animal class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"speak"),
            "Should find speak method, got: {:?}",
            names
        );

        // speak should have Animal as parent
        let speak = entities.iter().find(|e| e.name == "speak").unwrap();
        assert!(
            speak.parent_id.is_some(),
            "speak method should have a parent_id"
        );
    }

    #[test]
    fn test_typescript_interface_extraction() {
        // Tests interface declaration and its body members
        let code = r#"
interface Shape {
    area(): number;
    perimeter(): number;
    color: string;
}

interface Drawable extends Shape {
    draw(): void;
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "shapes.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "TS interface: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Shape"),
            "Should find Shape interface, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Drawable"),
            "Should find Drawable interface, got: {:?}",
            names
        );

        let shape = entities.iter().find(|e| e.name == "Shape").unwrap();
        assert_eq!(
            shape.entity_type, "interface",
            "Shape should be an interface"
        );
    }

    #[test]
    fn test_rust_struct_and_trait_extraction() {
        // Tests Rust struct_item, trait_item, impl_item, and nested function_item
        let code = r#"
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
    fn name(&self) -> &str {
        "shape"
    }
}

pub struct Circle {
    pub center: Point,
    pub radius: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }

    fn perimeter(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius
    }
}

impl Circle {
    pub fn new(x: f64, y: f64, radius: f64) -> Self {
        Circle {
            center: Point { x, y },
            radius,
        }
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "geometry.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Rust struct+trait: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Point"),
            "Should find Point struct, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Shape"),
            "Should find Shape trait, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Circle"),
            "Should find Circle struct, got: {:?}",
            names
        );

        // Verify entity types
        let point = entities.iter().find(|e| e.name == "Point").unwrap();
        assert_eq!(point.entity_type, "struct", "Point should be a struct");

        let shape = entities.iter().find(|e| e.name == "Shape").unwrap();
        assert_eq!(shape.entity_type, "trait", "Shape should be a trait");
    }

    #[test]
    fn test_rust_impl_nested_methods() {
        // Tests that methods inside impl blocks have parent_id set
        let code = r#"
pub struct Counter {
    count: u32,
}

impl Counter {
    pub fn new() -> Self {
        Counter { count: 0 }
    }

    pub fn increment(&mut self) {
        self.count += 1;
    }

    pub fn value(&self) -> u32 {
        self.count
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "counter.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Rust impl methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Counter"),
            "Should find Counter struct, got: {:?}",
            names
        );
        assert!(
            names.contains(&"new"),
            "Should find new function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"increment"),
            "Should find increment function, got: {:?}",
            names
        );
        assert!(
            names.contains(&"value"),
            "Should find value function, got: {:?}",
            names
        );

        // Methods inside impl should have parent_id
        let new_fn = entities.iter().find(|e| e.name == "new").unwrap();
        assert!(
            new_fn.parent_id.is_some(),
            "new function should have parent_id (impl block)"
        );
    }

    #[test]
    fn test_python_class_with_methods() {
        // Tests Python class_definition containing method_definition (function_definition in block)
        let code = r#"
class Animal:
    def __init__(self, name: str):
        self.name = name

    def speak(self) -> str:
        return f"{self.name} makes a sound"

    def __repr__(self) -> str:
        return f"Animal({self.name!r})"


class Dog(Animal):
    def speak(self) -> str:
        return f"{self.name} barks"

    @staticmethod
    def species() -> str:
        return "Canis lupus familiaris"


def standalone_function(x: int) -> int:
    return x * 2
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "animals.py");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Python class+methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Animal"),
            "Should find Animal class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Dog"),
            "Should find Dog class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"standalone_function"),
            "Should find standalone_function, got: {:?}",
            names
        );

        // Verify class type
        let animal = entities.iter().find(|e| e.name == "Animal").unwrap();
        assert_eq!(animal.entity_type, "class", "Animal should be a class");

        // Methods should be nested
        let speak_methods: Vec<_> = entities.iter().filter(|e| e.name == "speak").collect();
        assert!(
            !speak_methods.is_empty(),
            "Should find speak methods, got: {:?}",
            names
        );

        // At least one speak should have a parent
        let has_parent = speak_methods.iter().any(|e| e.parent_id.is_some());
        assert!(
            has_parent,
            "speak methods should have parent_id (the class)"
        );
    }

    #[test]
    fn test_python_decorated_class() {
        // Tests decorated_definition for class (map_decorated_type returns "class")
        let code = r#"
import dataclasses

@dataclasses.dataclass
class Config:
    host: str
    port: int
    debug: bool = False

@staticmethod
def helper():
    pass
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "config.py");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Python decorated: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Config"),
            "Should find Config class, got: {:?}",
            names
        );

        let config = entities.iter().find(|e| e.name == "Config").unwrap();
        // decorated_definition with class_definition inside → should map to "class"
        assert_eq!(
            config.entity_type, "class",
            "Decorated class should have entity_type 'class'"
        );
    }

    #[test]
    fn test_go_method_and_type_extraction() {
        // Tests Go method_declaration and function_declaration extraction.
        // Note: Go tree-sitter represents `type Rectangle struct { ... }` as a
        // type_declaration containing a type_spec, so the name is not at the
        // type_declaration level directly. Methods (func with receiver) use
        // method_declaration which does have a name field.
        let code = r#"
package main

import "fmt"

type Rectangle struct {
    Width  float64
    Height float64
}

type Circle struct {
    Radius float64
}

func (r Rectangle) Area() float64 {
    return r.Width * r.Height
}

func (r Rectangle) Perimeter() float64 {
    return 2 * (r.Width + r.Height)
}

func (c Circle) Area() float64 {
    return 3.14159 * c.Radius * c.Radius
}

func main() {
    r := Rectangle{Width: 3, Height: 4}
    fmt.Println(r.Area())
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "shapes.go");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let _types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Go method+type: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type))
                .collect::<Vec<_>>()
        );

        // Go methods (with receiver) should be found
        let area_methods: Vec<_> = entities.iter().filter(|e| e.name == "Area").collect();
        assert!(
            !area_methods.is_empty(),
            "Should find Area method declarations, got: {:?}",
            names
        );

        // Verify method entity type
        let area = area_methods[0];
        assert_eq!(area.entity_type, "method", "Area should be a method");

        // Regular function should be found
        assert!(
            names.contains(&"main"),
            "Should find main function, got: {:?}",
            names
        );

        let main_fn = entities.iter().find(|e| e.name == "main").unwrap();
        assert_eq!(main_fn.entity_type, "function", "main should be a function");
    }

    #[test]
    fn test_php_class_and_trait_extraction() {
        // Tests PHP class_declaration, trait_declaration, method_declaration, interface_declaration
        let code = r#"<?php

namespace App\Models;

interface Printable {
    public function toString(): string;
}

trait Timestampable {
    private \DateTime $createdAt;

    public function getCreatedAt(): \DateTime {
        return $this->createdAt;
    }

    public function setCreatedAt(\DateTime $dt): void {
        $this->createdAt = $dt;
    }
}

class User implements Printable {
    use Timestampable;

    private string $name;

    public function __construct(string $name) {
        $this->name = $name;
    }

    public function toString(): string {
        return $this->name;
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "User.php");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "PHP class+trait: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Printable"),
            "Should find Printable interface, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Timestampable"),
            "Should find Timestampable trait, got: {:?}",
            names
        );
        assert!(
            names.contains(&"User"),
            "Should find User class, got: {:?}",
            names
        );

        // Verify entity types
        let printable = entities.iter().find(|e| e.name == "Printable").unwrap();
        assert_eq!(
            printable.entity_type, "interface",
            "Printable should be an interface"
        );

        let timestampable = entities.iter().find(|e| e.name == "Timestampable").unwrap();
        assert_eq!(
            timestampable.entity_type, "trait",
            "Timestampable should be a trait"
        );
    }

    #[test]
    fn test_javascript_class_with_methods() {
        // Tests JS class with method_definition inside class_body
        let code = r#"
class EventEmitter {
    #listeners = new Map();

    on(event, listener) {
        if (!this.#listeners.has(event)) {
            this.#listeners.set(event, []);
        }
        this.#listeners.get(event).push(listener);
        return this;
    }

    emit(event, ...args) {
        const listeners = this.#listeners.get(event) || [];
        listeners.forEach(fn => fn(...args));
        return this;
    }

    off(event, listener) {
        const arr = this.#listeners.get(event);
        if (arr) {
            this.#listeners.set(event, arr.filter(l => l !== listener));
        }
        return this;
    }
}

function createEmitter() {
    return new EventEmitter();
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "events.js");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "JS class+methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"EventEmitter"),
            "Should find EventEmitter class, got: {:?}",
            names
        );
        assert!(
            names.contains(&"on"),
            "Should find on method, got: {:?}",
            names
        );
        assert!(
            names.contains(&"emit"),
            "Should find emit method, got: {:?}",
            names
        );
        assert!(
            names.contains(&"createEmitter"),
            "Should find createEmitter function, got: {:?}",
            names
        );

        // Methods should have EventEmitter as parent
        let on_method = entities.iter().find(|e| e.name == "on").unwrap();
        assert!(
            on_method.parent_id.is_some(),
            "on method should have parent_id"
        );
    }

    #[test]
    fn test_rust_trait_with_default_methods() {
        // Tests trait_item with methods inside declaration_list
        let code = r#"
pub trait Greet {
    fn name(&self) -> &str;

    fn greeting(&self) -> String {
        format!("Hello, {}!", self.name())
    }

    fn farewell(&self) -> String {
        format!("Goodbye, {}!", self.name())
    }
}

pub struct Person {
    pub name: String,
}

impl Greet for Person {
    fn name(&self) -> &str {
        &self.name
    }
}
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "greet.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        eprintln!(
            "Rust trait methods: {:?}",
            entities
                .iter()
                .map(|e| (&e.name, &e.entity_type, &e.parent_id))
                .collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Greet"),
            "Should find Greet trait, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Person"),
            "Should find Person struct, got: {:?}",
            names
        );

        let greet = entities.iter().find(|e| e.name == "Greet").unwrap();
        assert_eq!(greet.entity_type, "trait", "Greet should be a trait");

        // Methods inside trait should have parent_id
        let greeting = entities.iter().find(|e| e.name == "greeting");
        if let Some(g) = greeting {
            assert!(
                g.parent_id.is_some(),
                "greeting should have parent_id (trait)"
            );
        }
    }

    // ---------------------------------------------------------------
    // entity_extractor coverage: extract_name edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_typescript_const_variable_declaration() {
        // Tests lexical_declaration → variable_declarator → name extraction path
        let code = "export const API_URL = \"https://example.com\";\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "config.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"API_URL"),
            "Should find const API_URL, got: {:?}",
            names
        );
    }

    #[test]
    fn test_typescript_let_variable_declaration() {
        // Tests variable_declaration path
        let code = "let counter = 0;\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "test.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"counter"),
            "Should find let counter, got: {:?}",
            names
        );
    }

    #[test]
    fn test_c_typedef_struct_extraction() {
        // Tests type_definition → declarator name extraction (typedef struct)
        let code = "typedef struct {\n    int x;\n    int y;\n} Point;\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "types.h");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"Point"),
            "Should find typedef struct Point, got: {:?}",
            names
        );
    }

    #[test]
    fn test_c_union_extraction() {
        // Tests union_specifier → name extraction
        let code = "union Data {\n    int i;\n    float f;\n};\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "data.c");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"Data"),
            "Should find union Data, got: {:?}",
            names
        );
    }

    #[test]
    fn test_c_function_pointer_declarator() {
        // Tests pointer_declarator path in extract_declarator_name
        let code = "int *get_pointer() {\n    return 0;\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "ptr.c");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"get_pointer"),
            "Should find function get_pointer, got: {:?}",
            names
        );
    }

    #[test]
    fn test_cpp_template_class() {
        // Tests template_declaration → inner class name extraction
        let code = "template<typename T>\nclass Container {\npublic:\n    T value;\n};\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "container.hpp");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"Container"),
            "Should find template class Container, got: {:?}",
            names
        );
    }

    #[test]
    fn test_cpp_template_function() {
        // Tests template_declaration → inner function with declarator name extraction
        let code = "template<typename T>\nT maximum(T a, T b) {\n    return a > b ? a : b;\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "util.cpp");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"maximum"),
            "Should find template function maximum, got: {:?}",
            names
        );
    }

    #[test]
    fn test_csharp_struct_extraction() {
        // Tests C# struct_declaration → name extraction
        let code = "public struct Point {\n    public int X;\n    public int Y;\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "Point.cs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"Point"),
            "Should find struct Point, got: {:?}",
            names
        );
    }

    #[test]
    fn test_csharp_property_extraction() {
        // Tests C# property_declaration → name extraction
        let code = "public class Person {\n    public string Name { get; set; }\n    public int Age { get; set; }\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "Person.cs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"Person"),
            "Should find class Person, got: {:?}",
            names
        );
    }

    #[test]
    fn test_csharp_namespace_extraction() {
        // Tests C# namespace_declaration → name extraction
        let code = "namespace MyApp.Models {\n    public class User { }\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "User.cs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"MyApp.Models"),
            "Should find namespace MyApp.Models, got: {:?}",
            names
        );
    }

    #[test]
    fn test_rust_const_and_static_extraction() {
        // Tests const_item and static_item entity extraction
        let code = "pub const MAX_SIZE: usize = 100;\npub static GLOBAL_COUNT: i32 = 0;\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "consts.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        assert!(
            names.contains(&"MAX_SIZE"),
            "Should find const MAX_SIZE, got: {:?}",
            names
        );
        assert!(
            names.contains(&"GLOBAL_COUNT"),
            "Should find static GLOBAL_COUNT, got: {:?}",
            names
        );
        let max_idx = names.iter().position(|n| *n == "MAX_SIZE").unwrap();
        assert_eq!(types[max_idx], "constant");
        let global_idx = names.iter().position(|n| *n == "GLOBAL_COUNT").unwrap();
        assert_eq!(types[global_idx], "static");
    }

    #[test]
    fn test_rust_type_alias_extraction() {
        // Tests type_item extraction
        let code = "pub type Result<T> = std::result::Result<T, Error>;\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "types.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"Result"),
            "Should find type alias Result, got: {:?}",
            names
        );
    }

    #[test]
    fn test_rust_mod_extraction() {
        // Tests mod_item extraction
        let code = "pub mod helpers {\n    pub fn do_thing() {}\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "lib.rs");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"helpers"),
            "Should find mod helpers, got: {:?}",
            names
        );
    }

    #[test]
    fn test_elixir_defstruct_extraction() {
        // Tests defstruct entity extraction
        let code = "defmodule User do\n  defstruct [:name, :age]\nend\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "user.ex");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        assert!(
            names.contains(&"User"),
            "Should find module User, got: {:?}",
            names
        );
        assert!(
            names.contains(&"__struct__"),
            "Should find defstruct as __struct__, got: {:?}",
            names
        );
        let struct_idx = names.iter().position(|n| *n == "__struct__").unwrap();
        assert_eq!(types[struct_idx], "struct");
    }

    #[test]
    fn test_elixir_defexception_extraction() {
        // Tests defexception entity extraction
        let code = "defmodule MyError do\n  defexception message: \"error\"\nend\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "error.ex");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"__exception__"),
            "Should find defexception as __exception__, got: {:?}",
            names
        );
    }

    #[test]
    fn test_elixir_defimpl_with_for() {
        // Tests defimpl with for: keyword
        let code = "defimpl String.Chars, for: Integer do\n  def to_string(i), do: Integer.to_string(i)\nend\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "impl.ex");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "Elixir defimpl: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );
        // Should find a defimpl entity
        assert!(
            types.contains(&"impl"),
            "Should find defimpl entity, got types: {:?}",
            types
        );
    }

    #[test]
    fn test_elixir_defguard_extraction() {
        // Tests defguard with when clause (binary_operator path)
        let code = "defmodule Guards do\n  defguard is_even(x) when rem(x, 2) == 0\nend\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "guards.ex");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"is_even"),
            "Should find defguard is_even, got: {:?}",
            names
        );
    }

    #[test]
    fn test_python_decorated_function() {
        // Tests decorated_definition wrapping a function_definition
        let code = "@app.route('/hello')\ndef hello():\n    return 'Hello, World!'\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "app.py");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        assert!(
            names.contains(&"hello"),
            "Should find decorated function hello, got: {:?}",
            names
        );
        let idx = names.iter().position(|n| *n == "hello").unwrap();
        assert_eq!(
            types[idx], "function",
            "decorated function_definition should map to 'function'"
        );
    }

    #[test]
    fn test_map_node_type_coverage() {
        // Exercise map_node_type through real code extraction for various entity types
        // Java constructor
        let code = "public class Foo {\n    public Foo() {}\n}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "Foo.java");
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        assert!(
            types.contains(&"class"),
            "Should map class_declaration to 'class', got: {:?}",
            types
        );
    }

    #[test]
    fn test_javascript_export_statement() {
        // Tests export_statement → declaration visit path
        let code = "export function greet() { return 'hi'; }\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "greet.js");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"greet"),
            "Should find exported function greet, got: {:?}",
            names
        );
    }

    #[test]
    fn test_empty_source_returns_no_entities() {
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities("", "empty.ts");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_entity_has_structural_hash() {
        // Code parser should produce structural_hash via tree-sitter
        let code = "function hello() { return 1; }\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "test.ts");
        assert!(!entities.is_empty());
        assert!(
            entities[0].structural_hash.is_some(),
            "Code entities should have structural_hash"
        );
    }

    #[test]
    fn test_entity_line_numbers() {
        let code = "\nfunction first() {}\n\nfunction second() {}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "test.ts");
        let first = entities.iter().find(|e| e.name == "first");
        let second = entities.iter().find(|e| e.name == "second");
        assert!(first.is_some(), "Should find first function");
        assert!(second.is_some(), "Should find second function");
        // second should start after first
        if let (Some(f), Some(s)) = (first, second) {
            assert!(
                s.start_line > f.start_line,
                "second should start after first"
            );
        }
    }

    #[test]
    fn test_c_global_declaration() {
        // Tests C declaration (not function_definition) → declarator name extraction
        let code = "int global_counter;\nvoid func() {}\n";
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "globals.c");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"global_counter"),
            "Should find global declaration, got: {:?}",
            names
        );
    }

    #[test]
    fn test_unknown_extension_returns_empty() {
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities("some content", "file.unknown_ext");
        assert!(entities.is_empty());
    }

    #[test]
    fn test_typescript_enum_extraction() {
        // Tests TypeScript enum_declaration
        let code = r#"
enum Direction {
    Up = "UP",
    Down = "DOWN",
    Left = "LEFT",
    Right = "RIGHT",
}

const enum Color {
    Red,
    Green,
    Blue,
}

type Point = {
    x: number;
    y: number;
};
"#;
        let plugin = CodeParserPlugin;
        let entities = plugin.extract_entities(code, "types.ts");
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        let types: Vec<&str> = entities.iter().map(|e| e.entity_type.as_str()).collect();
        eprintln!(
            "TS enum+type: {:?}",
            names.iter().zip(types.iter()).collect::<Vec<_>>()
        );

        assert!(
            names.contains(&"Direction"),
            "Should find Direction enum, got: {:?}",
            names
        );
        assert!(
            names.contains(&"Point"),
            "Should find Point type alias, got: {:?}",
            names
        );

        let direction = entities.iter().find(|e| e.name == "Direction").unwrap();
        assert_eq!(direction.entity_type, "enum", "Direction should be an enum");

        let point = entities.iter().find(|e| e.name == "Point").unwrap();
        assert_eq!(point.entity_type, "type", "Point should be a type alias");
    }
}
