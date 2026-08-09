//! The named definitions one file contributes to the duplicate gate, each one
//! normalized so that two definitions can be compared.
//!
//! The detector that pairs the definitions lives in
//! `swissarmyhammer-code-context`. This module owns the grammar half of the
//! question, and only that half: which nodes are a function, a method or a
//! type, how each one's tokens normalize, which definitions are test code, and
//! which definitions a marker comment exempts. Every one of them is read off
//! the tree-sitter parse. None of them reads the file's path.

use std::collections::HashMap;
use std::ops::Range;

use tree_sitter::Node;

use super::parse_code;

/// The marker comment that exempts the definition after it from the gate.
///
/// One form, in whatever comment syntax the language spells. The text is what
/// counts and the delimiter never does, so `// sah:allow duplication <reason>`,
/// `# sah:allow duplication <reason>` and
/// `/* sah:allow duplication <reason> */` all say the same thing.
pub const DUPLICATION_ALLOW_MARKER: &str = "sah:allow duplication";

/// One named definition of a file — a function, a method or a type — with the
/// token stream two definitions are compared over.
///
/// The stream is normalized, so it is not the definition's source text. What
/// the normalization drops is what the comparison is meant to see past: a
/// renamed variable, a substituted constant, a renamed field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DuplicationDefinition {
    /// The word this report calls the definition, in the language's own
    /// spelling (`fn`, `struct`, `def`, `func`).
    pub kind: &'static str,
    /// The name the definition declares.
    pub name: String,
    /// The one-based line the definition starts on.
    pub line: usize,
    /// The normalized token stream, in source order.
    pub shape: Vec<String>,
}

/// One file's contribution to the duplicate gate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DuplicationSource {
    /// The language id the grammar roster routed the file to.
    pub language: &'static str,
    /// Every reportable definition of the file, in source order. A test
    /// definition and a definition a marker comment exempts are already left
    /// out.
    pub definitions: Vec<DuplicationDefinition>,
}

/// Read the definitions of `source`, parsed as the language the grammar roster
/// routes `path` to.
///
/// Returns `None` — meaning **not parsed** — for a path the roster maps to no
/// grammar, the same silence [`parse_code`] keeps. A caller must report "not
/// measured" for `None` and never substitute an empty source, which would
/// read as "this file repeats nothing".
///
/// # Examples
///
/// ```
/// use swissarmyhammer_sem::parser::plugins::code::duplication_source;
///
/// let read = duplication_source("src/lib.rs", "fn one() { let a = 1; }\n")
///     .ok_or("rust is mapped")?;
/// assert_eq!(read.language, "rust");
/// assert_eq!(read.definitions[0].name, "one");
/// assert!(duplication_source("notes.txt", "fn one() {}\n").is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn duplication_source(path: &str, source: &str) -> Option<DuplicationSource> {
    let parsed = parse_code(path, source)?;
    let root = parsed.tree().root_node();
    let language = parsed.language();

    let tests = test_spec(language);
    let mut exempt = Vec::new();
    collect_under(
        root,
        &mut |node| exemption_of(node, tests, source),
        Descent::Past,
        &mut exempt,
    );

    let declares = definition_spec(language);
    let mut definitions = Vec::new();
    collect_under(
        root,
        &mut |node| definition_of(node, declares, source, &exempt),
        Descent::Through,
        &mut definitions,
    );

    Some(DuplicationSource {
        language,
        definitions,
    })
}

/// How one definition's tokens are normalized before comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Normalization {
    /// A statement stream, which is what a function or a method holds. The
    /// first distinct identifier becomes `v1`, the second `v2`, and every
    /// literal becomes a marker of its kind, so two bodies that differ only by
    /// their variable names or by one constant normalize the same.
    Positional,
    /// A member list, which is what a type holds. Every declared name drops
    /// out and every type stays, so two records whose members carry the same
    /// types in the same order normalize the same.
    Nameless,
}

/// One kind of definition a language declares.
struct DefinitionKind {
    /// The node kind the grammar names it with — or, for a language that
    /// spells a definition as a macro call, the name of the call target.
    node: &'static str,
    /// The word this report calls it.
    label: &'static str,
    /// How its tokens are normalized.
    normalization: Normalization,
}

/// A function or a method, whose body is a statement stream.
const fn callable(node: &'static str, label: &'static str) -> DefinitionKind {
    DefinitionKind {
        node,
        label,
        normalization: Normalization::Positional,
    }
}

/// A type, whose body is a member list.
const fn record(node: &'static str, label: &'static str) -> DefinitionKind {
    DefinitionKind {
        node,
        label,
        normalization: Normalization::Nameless,
    }
}

/// The definitions one language declares.
struct DefinitionSpec {
    /// The definitions the grammar gives a node kind of their own.
    kinds: &'static [DefinitionKind],
    /// The definitions the grammar spells as a call to a macro, keyed by the
    /// name of the call target.
    calls: &'static [DefinitionKind],
}

/// A language the roster parses and this module reads no definition from.
static NO_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[],
    calls: &[],
};

/// Rust: a free function or an `impl` method, and the three type declarations.
static RUST_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_item", "fn"),
        record("struct_item", "struct"),
        record("enum_item", "enum"),
        record("trait_item", "trait"),
    ],
    calls: &[],
};

/// TypeScript, TSX and JavaScript: a function, a class method, and the two
/// type declarations TypeScript adds.
static TYPESCRIPT_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_declaration", "function"),
        callable("method_definition", "method"),
        record("interface_declaration", "interface"),
        record("type_alias_declaration", "type"),
    ],
    calls: &[],
};

/// Python: a `def`, which is both the function and the method, and a `class`.
static PYTHON_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_definition", "def"),
        record("class_definition", "class"),
    ],
    calls: &[],
};

/// Go: a function, a method, and a `type`.
static GO_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_declaration", "func"),
        callable("method_declaration", "method"),
        record("type_declaration", "type"),
    ],
    calls: &[],
};

/// Swift: a function or a method, and the type declarations. One grammar node
/// carries `class`, `struct` and `enum`, so the three share one label.
static SWIFT_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_declaration", "func"),
        record("class_declaration", "type"),
        record("protocol_declaration", "protocol"),
    ],
    calls: &[],
};

/// Java: a method or a constructor, and the type declarations.
static JAVA_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("method_declaration", "method"),
        callable("constructor_declaration", "constructor"),
        record("class_declaration", "class"),
        record("interface_declaration", "interface"),
        record("enum_declaration", "enum"),
    ],
    calls: &[],
};

/// C#: a method or a constructor, and the type declarations.
static CSHARP_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("method_declaration", "method"),
        callable("constructor_declaration", "constructor"),
        record("class_declaration", "class"),
        record("interface_declaration", "interface"),
        record("struct_declaration", "struct"),
        record("enum_declaration", "enum"),
    ],
    calls: &[],
};

/// C: a function, and the three aggregate specifiers.
static C_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_definition", "function"),
        record("struct_specifier", "struct"),
        record("union_specifier", "union"),
        record("enum_specifier", "enum"),
    ],
    calls: &[],
};

/// C++: C's set, and a class.
static CPP_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_definition", "function"),
        record("class_specifier", "class"),
        record("struct_specifier", "struct"),
        record("union_specifier", "union"),
        record("enum_specifier", "enum"),
    ],
    calls: &[],
};

/// Ruby: a method, and the two definition bodies that hold methods.
static RUBY_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("method", "def"),
        callable("singleton_method", "def"),
        record("class", "class"),
        record("module", "module"),
    ],
    calls: &[],
};

/// PHP: a function or a method, and the type declarations.
static PHP_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function_definition", "function"),
        callable("method_declaration", "method"),
        record("class_declaration", "class"),
        record("interface_declaration", "interface"),
        record("trait_declaration", "trait"),
        record("enum_declaration", "enum"),
    ],
    calls: &[],
};

/// Fortran: the two procedure forms. Neither names its body, so the whole
/// declaration is the stream — see [`definition_body`].
static FORTRAN_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[
        callable("function", "function"),
        callable("subroutine", "subroutine"),
    ],
    calls: &[],
};

/// Bash: a function, and there is no type to declare.
static BASH_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[callable("function_definition", "function")],
    calls: &[],
};

/// Elixir: every definition is a call to one of four macros. A module is left
/// out on purpose — it is a namespace holding the definitions below it, the
/// way a Rust `mod` is, and neither is a unit this rule compares.
static ELIXIR_DEFINITION_SPEC: DefinitionSpec = DefinitionSpec {
    kinds: &[],
    calls: &[
        callable("def", "def"),
        callable("defp", "defp"),
        callable("defmacro", "defmacro"),
        callable("defmacrop", "defmacrop"),
    ],
};

/// The definitions `language` declares.
fn definition_spec(language: &str) -> &'static DefinitionSpec {
    match language {
        "rust" => &RUST_DEFINITION_SPEC,
        "typescript" | "tsx" | "javascript" => &TYPESCRIPT_DEFINITION_SPEC,
        "python" => &PYTHON_DEFINITION_SPEC,
        "go" => &GO_DEFINITION_SPEC,
        "swift" => &SWIFT_DEFINITION_SPEC,
        "java" => &JAVA_DEFINITION_SPEC,
        "csharp" => &CSHARP_DEFINITION_SPEC,
        "c" => &C_DEFINITION_SPEC,
        "cpp" => &CPP_DEFINITION_SPEC,
        "ruby" => &RUBY_DEFINITION_SPEC,
        "php" => &PHP_DEFINITION_SPEC,
        "fortran" => &FORTRAN_DEFINITION_SPEC,
        "bash" => &BASH_DEFINITION_SPEC,
        "elixir" => &ELIXIR_DEFINITION_SPEC,
        _ => &NO_DEFINITION_SPEC,
    }
}

/// The definition `node` is, `None` when it declares none, when it declares
/// one with no name, or when it starts inside an exempted range.
///
/// The start of the definition decides the exemption, which is what makes a
/// definition inside a test module exempt however far it runs.
fn definition_of(
    node: Node<'_>,
    spec: &'static DefinitionSpec,
    source: &str,
    exempt: &[Range<usize>],
) -> Option<DuplicationDefinition> {
    if exempt
        .iter()
        .any(|range| range.contains(&node.start_byte()))
    {
        return None;
    }
    let (kind, name) = declared_kind(node, spec, source)?;
    let shape = shape_of(node, kind.normalization, source);
    (!shape.is_empty()).then(|| DuplicationDefinition {
        kind: kind.label,
        name: name.to_string(),
        line: node.start_position().row + 1,
        shape,
    })
}

/// The kind `node` declares and the name it declares it under.
fn declared_kind<'a>(
    node: Node<'_>,
    spec: &'static DefinitionSpec,
    source: &'a str,
) -> Option<(&'static DefinitionKind, &'a str)> {
    if let Some(kind) = spec.kinds.iter().find(|kind| kind.node == node.kind()) {
        return Some((kind, definition_name(node, source)?));
    }
    let target = call_target_name(node, source)?;
    let kind = spec.calls.iter().find(|kind| kind.node == target)?;
    Some((kind, call_definition_name(node, source)?))
}

/// The normalized token stream `node` compares by.
fn shape_of(node: Node<'_>, normalization: Normalization, source: &str) -> Vec<String> {
    let root = match normalization {
        Normalization::Positional => definition_body(node),
        Normalization::Nameless => node,
    };
    let mut placeholders = Placeholders::default();
    let mut shape = Vec::new();
    write_shape(root, normalization, source, &mut placeholders, &mut shape);
    shape
}

/// The node holding a callable's body: the `body` the grammar names, else the
/// first block-shaped child, else the whole declaration.
///
/// The last case is Fortran, whose procedure node carries its statements as
/// plain siblings of its header rather than inside a block.
fn definition_body(node: Node<'_>) -> Node<'_> {
    if let Some(body) = node.child_by_field_name("body") {
        return body;
    }
    let mut cursor = node.walk();
    let block = node
        .children(&mut cursor)
        .find(|child| BODY_KINDS.contains(&child.kind()));
    block.unwrap_or(node)
}

/// Write the normalized tokens under `node` into `out`.
fn write_shape(
    node: Node<'_>,
    normalization: Normalization,
    source: &str,
    placeholders: &mut Placeholders,
    out: &mut Vec<String>,
) {
    let declared = (normalization == Normalization::Nameless)
        .then(|| node.child_by_field_name("name"))
        .flatten();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_comment_kind(child.kind()) || child.start_byte() >= child.end_byte() {
            continue;
        }
        if declared.is_some_and(|name| name.id() == child.id()) {
            continue;
        }
        if normalization == Normalization::Nameless && is_member_name_kind(child.kind()) {
            continue;
        }
        if let Some(marker) = literal_marker(child.kind()) {
            out.push(marker.to_string());
            continue;
        }
        if child.child_count() > 0 {
            write_shape(child, normalization, source, placeholders, out);
            continue;
        }
        out.push(leaf_shape(child, normalization, source, placeholders));
    }
}

/// The token one leaf normalizes to.
fn leaf_shape(
    node: Node<'_>,
    normalization: Normalization,
    source: &str,
    placeholders: &mut Placeholders,
) -> String {
    let text = node_text(node, source);
    if normalization == Normalization::Positional && is_identifier_kind(node.kind()) {
        return placeholders.of(text);
    }
    text.to_string()
}

/// The prefix a positional placeholder is spelled with, so the first distinct
/// identifier of a body reads `v1`.
const IDENTIFIER_PLACEHOLDER: &str = "v";

/// The placeholder each identifier of one body takes.
#[derive(Default)]
struct Placeholders {
    /// The index already handed to each identifier this body has spelled.
    seen: HashMap<String, usize>,
}

impl Placeholders {
    /// The placeholder `text` takes — the same one every time it repeats, and
    /// the next unused one the first time it appears.
    fn of(&mut self, text: &str) -> String {
        let next = self.seen.len() + 1;
        let index = *self.seen.entry(text.to_string()).or_insert(next);
        format!("{IDENTIFIER_PLACEHOLDER}{index}")
    }
}

/// The marker each literal kind normalizes to, matched on the first substring
/// the node kind holds.
///
/// The order is the whole of the table's correctness: Go spells a string
/// `interpreted_string_literal`, which holds `int` as well as `string`, so the
/// string entries have to be read first.
const LITERAL_MARKERS: &[(&str, &str)] = &[
    ("string", "#str"),
    ("char", "#char"),
    ("rune", "#char"),
    ("bool", "#bool"),
    ("true", "#bool"),
    ("false", "#bool"),
    ("float", "#float"),
    ("real", "#float"),
    ("double", "#float"),
    ("int", "#num"),
    ("number", "#num"),
    ("numeric", "#num"),
];

/// The marker a literal whose kind names none of [`LITERAL_MARKERS`] takes.
const OTHER_LITERAL_MARKER: &str = "#lit";

/// The literal node kinds that do not end in `_literal`.
const LITERAL_KINDS: &[&str] = &[
    "boolean",
    "character",
    "false",
    "float",
    "integer",
    "nil",
    "none",
    "null",
    "number",
    "raw_string",
    "string",
    "template_string",
    "true",
];

/// The marker `kind` normalizes to, `None` when `kind` names no literal.
fn literal_marker(kind: &str) -> Option<&'static str> {
    if !kind.ends_with("_literal") && !LITERAL_KINDS.contains(&kind) {
        return None;
    }
    LITERAL_MARKERS
        .iter()
        .find(|(needle, _)| kind.contains(needle))
        .map_or(Some(OTHER_LITERAL_MARKER), |(_, marker)| Some(marker))
}

/// Whether `kind` names a leaf that spells an identifier.
fn is_identifier_kind(kind: &str) -> bool {
    kind == "identifier"
        || kind == "alias"
        || kind == "constant"
        || kind == "name"
        || kind.ends_with("_identifier")
        || kind.ends_with("_name")
        || kind.ends_with("_variable")
}

/// The leaf kinds that spell the name a member declares, for the grammars
/// that give a field its name without a `name` field to read it by.
const MEMBER_NAME_KINDS: &[&str] = &["field_identifier", "property_identifier"];

/// Whether `kind` names a leaf that spells a member's declared name.
fn is_member_name_kind(kind: &str) -> bool {
    MEMBER_NAME_KINDS.contains(&kind)
}

/// The name an Elixir definition declares.
///
/// A definition is a call whose first argument is itself the call the
/// definition heads — `def fold(a)` — or a bare name when it takes no
/// argument.
fn call_definition_name<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    let head = node.named_child(1)?.named_child(0)?;
    if head.kind() == "call" {
        return Some(node_text(head.named_child(0)?, source));
    }
    Some(node_text(head, source))
}

/// How one language marks a definition as test code.
///
/// Every field is a structural fact the parse reports — the text of an
/// attribute node, the name a definition declares, the name of the function a
/// call names, or the bases a type declaration lists. None of them is the
/// file's path, and none of them is a judgment.
struct TestSpec {
    /// Attribute, annotation and decorator names that mark the definition
    /// they decorate as test code.
    attributes: &'static [&'static str],
    /// Name prefixes that mark a definition as test code.
    name_prefixes: &'static [&'static str],
    /// Call targets whose whole call is a test definition.
    calls: &'static [&'static str],
    /// Names in a base or conformance list that make a whole type test code.
    bases: &'static [&'static str],
}

/// A language whose test code carries no marker at the definition.
static NO_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[],
    name_prefixes: &[],
    calls: &[],
    bases: &[],
};

/// Rust: the compiler's own two markers, and nothing else.
static RUST_TEST_SPEC: TestSpec = TestSpec {
    attributes: &["test", "cfg(test)"],
    name_prefixes: &[],
    calls: &[],
    bases: &[],
};

/// Python: pytest reads the name, unittest reads the base class, and a
/// pytest fixture is test support that carries its own decorator.
static PYTHON_TEST_SPEC: TestSpec = TestSpec {
    attributes: &["fixture"],
    name_prefixes: &["test_", "Test"],
    calls: &[],
    bases: &["TestCase"],
};

/// Go: `go test` reads these four name prefixes and nothing else.
static GO_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[],
    name_prefixes: &["Test", "Benchmark", "Example", "Fuzz"],
    calls: &[],
    bases: &[],
};

/// JavaScript, TypeScript and TSX: every runner in wide use — jest, vitest,
/// mocha, node:test — spells a test as a call.
static JAVASCRIPT_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[],
    name_prefixes: &[],
    calls: &[
        "describe",
        "it",
        "test",
        "suite",
        "bench",
        "beforeAll",
        "beforeEach",
        "afterAll",
        "afterEach",
    ],
    bases: &[],
};

/// Java: JUnit annotates the method.
static JAVA_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[
        "Test",
        "ParameterizedTest",
        "RepeatedTest",
        "BeforeEach",
        "AfterEach",
        "BeforeAll",
        "AfterAll",
    ],
    name_prefixes: &[],
    calls: &[],
    bases: &[],
};

/// C#: xunit, nunit and mstest each annotate the member.
static CSHARP_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[
        "Fact",
        "Theory",
        "Test",
        "TestCase",
        "TestMethod",
        "TestFixture",
        "TestClass",
        "SetUp",
        "TearDown",
    ],
    name_prefixes: &[],
    calls: &[],
    bases: &[],
};

/// C and C++: the xUnit-family macros open the definition, so the name the
/// declarator carries is the marker.
static C_FAMILY_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[],
    name_prefixes: &["TEST", "TYPED_TEST", "BOOST_AUTO_TEST_CASE"],
    calls: &["TEST_CASE", "SECTION"],
    bases: &[],
};

/// Swift: XCTest reads the base class and the `test` prefix, swift-testing
/// reads the attribute.
static SWIFT_TEST_SPEC: TestSpec = TestSpec {
    attributes: &["Test", "Suite"],
    name_prefixes: &["test"],
    calls: &[],
    bases: &["XCTestCase"],
};

/// Ruby: rspec spells a test as a call, minitest reads the name and the base.
static RUBY_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[],
    name_prefixes: &["test_"],
    calls: &["describe", "context", "it", "specify", "before", "after"],
    bases: &["TestCase", "Minitest::Test"],
};

/// PHP: PHPUnit reads the attribute, the name prefix and the base class.
static PHP_TEST_SPEC: TestSpec = TestSpec {
    attributes: &["Test"],
    name_prefixes: &["test"],
    calls: &[],
    bases: &["TestCase"],
};

/// Elixir: ExUnit spells a test as a call.
static ELIXIR_TEST_SPEC: TestSpec = TestSpec {
    attributes: &[],
    name_prefixes: &[],
    calls: &["test", "describe", "setup", "setup_all"],
    bases: &[],
};

/// The test markers of `language`.
///
/// Bash and Fortran answer [`NO_TEST_SPEC`]: neither writes its tests beside
/// the code they exercise, so neither has a marker at a definition to read.
/// Their test files are whole files of their own — `bats` and pFUnit — and a
/// whole-file rule would have to read the path, which this module never does.
fn test_spec(language: &str) -> &'static TestSpec {
    match language {
        "rust" => &RUST_TEST_SPEC,
        "python" => &PYTHON_TEST_SPEC,
        "go" => &GO_TEST_SPEC,
        "typescript" | "tsx" | "javascript" => &JAVASCRIPT_TEST_SPEC,
        "java" => &JAVA_TEST_SPEC,
        "csharp" => &CSHARP_TEST_SPEC,
        "c" | "cpp" => &C_FAMILY_TEST_SPEC,
        "swift" => &SWIFT_TEST_SPEC,
        "ruby" => &RUBY_TEST_SPEC,
        "php" => &PHP_TEST_SPEC,
        "elixir" => &ELIXIR_TEST_SPEC,
        _ => &NO_TEST_SPEC,
    }
}

/// The node kinds that carry an attribute, an annotation or a decorator,
/// across the whole grammar roster.
const ATTRIBUTE_KINDS: &[&str] = &[
    "attribute_item",
    "attribute_list",
    "attribute",
    "annotation",
    "marker_annotation",
    "modifiers",
    "decorator",
];

/// The node kinds that hold a declaration's members, so the text before one
/// is the declaration's own header.
const BODY_KINDS: &[&str] = &[
    "block",
    "body",
    "body_statement",
    "class_body",
    "compound_statement",
    "declaration_list",
    "do_block",
    "enum_body",
    "field_declaration_list",
];

/// The node-kind endings that name a node which DEFINES something.
const DEFINITION_KIND_ENDINGS: &[&str] =
    &["_item", "_definition", "_declaration", "_method", "_class"];

/// The node kinds that define something and end in none of
/// [`DEFINITION_KIND_ENDINGS`].
const DEFINITION_KINDS: &[&str] = &["method", "class", "module", "singleton_method"];

/// The punctuation an attribute's syntax wraps its name in.
const ATTRIBUTE_PUNCTUATION: &[char] = &['#', '[', ']', '@', ' ', '\t', '\n', '\r'];

/// Whether a walk keeps descending through a node it has just collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Descent {
    /// Keep walking under the node. A language nests one definition inside
    /// another — a Rust `fn` sits in an `impl`, a TypeScript method sits in a
    /// class — so a definition walk must reach both.
    Through,
    /// Stop at the node. An exempted range already covers everything under
    /// it, so a second range for a nested node says nothing new.
    Past,
}

/// Every value `read` reports under `node`, in source order.
fn collect_under<T>(
    node: Node<'_>,
    read: &mut impl FnMut(Node<'_>) -> Option<T>,
    descent: Descent,
    out: &mut Vec<T>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let found = read(child);
        let collected = found.is_some();
        out.extend(found);
        if collected && descent == Descent::Past {
            continue;
        }
        collect_under(child, read, descent, out);
    }
}

/// The byte range `node` exempts, when it is an allow marker or a test
/// definition.
fn exemption_of(node: Node<'_>, spec: &TestSpec, source: &str) -> Option<Range<usize>> {
    if is_allow_marker(node, source) {
        return Some(marker_range(node));
    }
    if is_test_definition(node, spec, source) {
        return Some(definition_range(node));
    }
    None
}

/// Whether `node` is the marker comment that exempts the definition after it.
fn is_allow_marker(node: Node<'_>, source: &str) -> bool {
    is_comment_kind(node.kind()) && node_text(node, source).contains(DUPLICATION_ALLOW_MARKER)
}

/// The range an allow marker covers: itself, and the node it annotates.
fn marker_range(marker: Node<'_>) -> Range<usize> {
    let end = match annotated_sibling(marker) {
        Some(sibling) => sibling.end_byte(),
        None => marker
            .parent()
            .map_or_else(|| marker.end_byte(), |parent| parent.end_byte()),
    };
    marker.start_byte()..end
}

/// The node an allow marker annotates: the next sibling that is neither a
/// comment nor an attribute, so the marker reaches past a doc comment and
/// past the attributes of the item it exempts.
fn annotated_sibling(marker: Node<'_>) -> Option<Node<'_>> {
    let mut next = marker.next_sibling();
    while let Some(node) = next {
        if !is_comment_kind(node.kind()) && !is_attribute_kind(node.kind()) {
            return Some(node);
        }
        next = node.next_sibling();
    }
    None
}

/// Whether `node` defines test code.
fn is_test_definition(node: Node<'_>, spec: &TestSpec, source: &str) -> bool {
    marked_by_call(node, spec, source)
        || marked_by_attribute(node, spec, source)
        || marked_by_name(node, spec, source)
        || marked_by_base(node, spec, source)
}

/// The range a test definition covers: itself, and the attributes that
/// decorate it, so a definition that starts at `#[test]` is inside the range
/// the attribute marks.
fn definition_range(node: Node<'_>) -> Range<usize> {
    let mut start = node.start_byte();
    let mut previous = node.prev_sibling();
    while let Some(sibling) = previous {
        if !is_attribute_kind(sibling.kind()) {
            break;
        }
        start = sibling.start_byte();
        previous = sibling.prev_sibling();
    }
    start..node.end_byte()
}

/// Whether an attribute on `node` names a test marker.
fn marked_by_attribute(node: Node<'_>, spec: &TestSpec, source: &str) -> bool {
    if spec.attributes.is_empty() || !is_definition_kind(node.kind()) {
        return false;
    }
    attribute_texts(node, source).iter().any(|text| {
        spec.attributes
            .iter()
            .any(|marker| attribute_names(text, marker))
    })
}

/// Whether the name `node` declares carries a test prefix.
fn marked_by_name(node: Node<'_>, spec: &TestSpec, source: &str) -> bool {
    if spec.name_prefixes.is_empty() || !is_definition_kind(node.kind()) {
        return false;
    }
    let Some(name) = definition_name(node, source) else {
        return false;
    };
    spec.name_prefixes
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Whether `node` calls a test-defining function.
fn marked_by_call(node: Node<'_>, spec: &TestSpec, source: &str) -> bool {
    if spec.calls.is_empty() {
        return false;
    }
    call_target_name(node, source).is_some_and(|name| spec.calls.contains(&name))
}

/// Whether the declaration header of `node` lists a test base type.
fn marked_by_base(node: Node<'_>, spec: &TestSpec, source: &str) -> bool {
    if spec.bases.is_empty() || !is_definition_kind(node.kind()) {
        return false;
    }
    let header = declaration_header(node, source);
    spec.bases.iter().any(|base| header.contains(base))
}

/// The attribute texts that decorate `node` — the attribute siblings before
/// it, the attribute children inside it, and the attributes nested in either.
fn attribute_texts<'a>(node: Node<'_>, source: &'a str) -> Vec<&'a str> {
    let mut texts = Vec::new();
    let mut previous = node.prev_sibling();
    while let Some(sibling) = previous {
        if is_attribute_kind(sibling.kind()) {
            push_attribute_texts(sibling, source, &mut texts);
        } else if !is_comment_kind(sibling.kind()) {
            break;
        }
        previous = sibling.prev_sibling();
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_attribute_kind(child.kind()) {
            push_attribute_texts(child, source, &mut texts);
        }
    }
    texts
}

/// Push the text of `node` and of every attribute nested in it.
fn push_attribute_texts<'a>(node: Node<'_>, source: &'a str, out: &mut Vec<&'a str>) {
    out.push(node_text(node, source));
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_attribute_kind(child.kind()) {
            push_attribute_texts(child, source, out);
        }
    }
}

/// Whether the attribute text `attribute` names `marker`.
///
/// The text is read with its syntax stripped — `#[`, `]`, `@`, `[` and the
/// whitespace around them — so `#[cfg(test)]`, `@Test` and `[Fact]` all
/// reduce to the name the table lists. A path-qualified attribute also
/// matches on its last segment, so `#[tokio::test]` names `test` and
/// `@pytest.fixture` names `fixture`.
fn attribute_names(attribute: &str, marker: &str) -> bool {
    let stripped = attribute.trim_matches(|c| ATTRIBUTE_PUNCTUATION.contains(&c));
    stripped == marker || last_path_segment(stripped) == marker
}

/// The name of the function `node` calls, when `node` is a call at all.
fn call_target_name<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    if !node.kind().contains("call") && !node.kind().contains("invocation") {
        return None;
    }
    let name = last_path_segment(node_text(node.named_child(0)?, source));
    let is_identifier = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_');
    is_identifier.then_some(name)
}

/// The name `node` declares.
///
/// Three grammar shapes answer, in order: the `name` field, the `declarator`
/// chain the C-family grammars nest a function's name inside, and the `name`
/// field of the first named child — which is how Go spells a `type` and how
/// Fortran spells a procedure.
fn definition_name<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    if let Some(name) = node.child_by_field_name("name") {
        return Some(node_text(name, source));
    }
    if let Some(mut declarator) = node.child_by_field_name("declarator") {
        while let Some(inner) = declarator.child_by_field_name("declarator") {
            declarator = inner;
        }
        return Some(node_text(declarator, source));
    }
    let nested = node.named_child(0)?.child_by_field_name("name")?;
    Some(node_text(nested, source))
}

/// The declaration text of `node` up to its body — the part that names the
/// type's bases, without the members inside it.
fn declaration_header<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    let mut cursor = node.walk();
    let body = node
        .children(&mut cursor)
        .find(|child| BODY_KINDS.contains(&child.kind()));
    let end = body.map_or_else(|| node.end_byte(), |child| child.start_byte());
    source.get(node.start_byte()..end).unwrap_or_default()
}

/// The last segment of a `::`-qualified or `.`-qualified path.
fn last_path_segment(text: &str) -> &str {
    let after_colons = text.rsplit("::").next().unwrap_or(text);
    after_colons.rsplit('.').next().unwrap_or(after_colons)
}

/// The source text of `node`.
fn node_text<'a>(node: Node<'_>, source: &'a str) -> &'a str {
    source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or_default()
}

/// Whether `kind` names a comment node.
fn is_comment_kind(kind: &str) -> bool {
    kind.contains("comment")
}

/// Whether `kind` names an attribute, annotation or decorator node.
fn is_attribute_kind(kind: &str) -> bool {
    ATTRIBUTE_KINDS.contains(&kind)
}

/// Whether `kind` names a node that DEFINES something — the only node kind a
/// name, an attribute or a base list can mark as test code.
///
/// An attribute is never a definition, however its kind is spelled. Rust
/// names its attribute node `attribute_item`, which ends the same way
/// `function_item` and `mod_item` do; without this test `#[cfg(test)]` reads
/// as a test definition of its own and reports a second, nested range for
/// the item it already marks.
fn is_definition_kind(kind: &str) -> bool {
    if is_attribute_kind(kind) {
        return false;
    }
    DEFINITION_KINDS.contains(&kind)
        || DEFINITION_KIND_ENDINGS
            .iter()
            .any(|ending| kind.ends_with(ending))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// The read of `source`, parsed as the language `path` names.
    fn read(path: &str, source: &str) -> DuplicationSource {
        duplication_source(path, source).expect("the roster maps this extension")
    }

    /// The `kind` and `name` of every definition the read holds.
    fn declared(read: &DuplicationSource) -> Vec<(&str, &str)> {
        read.definitions
            .iter()
            .map(|definition| (definition.kind, definition.name.as_str()))
            .collect()
    }

    /// The shape of the definition named `name`.
    fn shape_named(read: &DuplicationSource, name: &str) -> Vec<String> {
        read.definitions
            .iter()
            .find(|definition| definition.name == name)
            .unwrap_or_else(|| panic!("`{name}` must be read: {:?}", declared(read)))
            .shape
            .clone()
    }

    /// Whether any definition of the read is named `name`.
    fn declares(read: &DuplicationSource, name: &str) -> bool {
        read.definitions
            .iter()
            .any(|definition| definition.name == name)
    }

    #[test]
    fn a_file_the_roster_does_not_claim_is_not_read() {
        assert!(duplication_source("notes.txt", "fn one() {}\n").is_none());
    }

    #[test]
    fn the_read_and_its_parts_key_a_hash_set() {
        let read = read("src/lib.rs", "fn one() { let a = 1; }\n");

        let definitions: HashSet<DuplicationDefinition> =
            read.definitions.iter().cloned().collect();
        let reads: HashSet<DuplicationSource> = HashSet::from([read.clone(), read]);

        assert_eq!(definitions.len(), 1);
        assert_eq!(reads.len(), 1, "two equal reads hash to one entry");
    }

    #[test]
    fn a_rust_function_is_read_with_its_kind_name_and_line() {
        let source = "//! A note\n\npub fn folded(limit: i32) -> i32 {\n    limit + 1\n}\n";

        let read = read("src/lib.rs", source);

        assert_eq!(declared(&read), [("fn", "folded")]);
        assert_eq!(read.definitions[0].line, 3);
    }

    #[test]
    fn two_functions_that_differ_only_by_a_variable_name_share_a_shape() {
        let renamed = concat!(
            "pub fn first(limit: i32) -> i32 {\n",
            "    let band = limit + 1;\n",
            "    band\n",
            "}\n",
            "pub fn second(limit: i32) -> i32 {\n",
            "    let total = limit + 1;\n",
            "    total\n",
            "}\n",
        );

        let read = read("src/lib.rs", renamed);

        assert_eq!(
            shape_named(&read, "first"),
            shape_named(&read, "second"),
            "a renamed variable must not change the shape"
        );
    }

    #[test]
    fn two_functions_that_differ_only_by_a_literal_share_a_shape() {
        let substituted = concat!(
            "pub fn first(limit: i32) -> i32 {\n",
            "    limit + 1\n",
            "}\n",
            "pub fn second(limit: i32) -> i32 {\n",
            "    limit + 4096\n",
            "}\n",
        );

        let read = read("src/lib.rs", substituted);

        assert_eq!(shape_named(&read, "first"), shape_named(&read, "second"));
    }

    #[test]
    fn a_literal_normalizes_to_a_marker_of_its_kind() {
        let source = "pub fn one() { let a = (1, 1.5, \"x\", 'c', true); }\n";

        let shape = shape_named(&read("src/lib.rs", source), "one");

        assert!(shape.contains(&"#num".to_string()), "{shape:?}");
        assert!(shape.contains(&"#float".to_string()), "{shape:?}");
        assert!(shape.contains(&"#str".to_string()), "{shape:?}");
        assert!(shape.contains(&"#char".to_string()), "{shape:?}");
        assert!(shape.contains(&"#bool".to_string()), "{shape:?}");
    }

    #[test]
    fn a_string_of_two_kinds_never_reads_as_a_number() {
        let source = "package band\n\nfunc One() string {\n\treturn \"one\"\n}\n";

        let shape = shape_named(&read("band.go", source), "One");

        assert!(shape.contains(&"#str".to_string()), "{shape:?}");
        assert!(!shape.contains(&"#num".to_string()), "{shape:?}");
    }

    #[test]
    fn two_functions_with_different_bodies_do_not_share_a_shape() {
        let different = concat!(
            "pub fn first(limit: i32) -> i32 {\n",
            "    limit + 1\n",
            "}\n",
            "pub fn second(limit: i32) -> i32 {\n",
            "    for step in 0..limit {\n",
            "        println!(\"{step}\");\n",
            "    }\n",
            "    limit\n",
            "}\n",
        );

        let read = read("src/lib.rs", different);

        assert_ne!(shape_named(&read, "first"), shape_named(&read, "second"));
    }

    #[test]
    fn a_rust_struct_drops_its_field_names_and_keeps_its_field_types() {
        let source = "pub struct Row {\n    pub width: usize,\n    pub label: String,\n}\n";

        let shape = shape_named(&read("src/lib.rs", source), "Row");

        assert!(!shape.contains(&"width".to_string()), "{shape:?}");
        assert!(!shape.contains(&"Row".to_string()), "{shape:?}");
        assert!(shape.contains(&"usize".to_string()), "{shape:?}");
        assert!(shape.contains(&"String".to_string()), "{shape:?}");
    }

    #[test]
    fn two_structs_with_the_same_field_types_share_a_shape() {
        let source = concat!(
            "pub struct Row {\n    pub width: usize,\n    pub label: String,\n}\n",
            "pub struct Band {\n    pub height: usize,\n    pub title: String,\n}\n",
        );

        let read = read("src/lib.rs", source);

        assert_eq!(shape_named(&read, "Row"), shape_named(&read, "Band"));
    }

    #[test]
    fn two_structs_with_different_field_types_do_not_share_a_shape() {
        let source = concat!(
            "pub struct Row {\n    pub width: usize,\n}\n",
            "pub struct Band {\n    pub height: String,\n}\n",
        );

        let read = read("src/lib.rs", source);

        assert_ne!(shape_named(&read, "Row"), shape_named(&read, "Band"));
    }

    #[test]
    fn a_rust_enum_and_a_trait_are_read_as_definitions() {
        let source = concat!(
            "pub enum Mode {\n    One,\n    Two(u32),\n}\n",
            "pub trait Reads {\n    fn read(&self) -> u32;\n}\n",
        );

        let read = read("src/lib.rs", source);

        assert_eq!(declared(&read), [("enum", "Mode"), ("trait", "Reads")]);
    }

    #[test]
    fn a_rust_impl_method_is_read_as_a_definition_of_its_own() {
        let source =
            "pub struct Row;\nimpl Row {\n    pub fn read(&self) -> u32 {\n        1\n    }\n}\n";

        let read = read("src/lib.rs", source);

        assert!(declares(&read, "read"), "{:?}", declared(&read));
    }

    #[test]
    fn a_rust_test_function_contributes_no_definition() {
        let source = concat!(
            "pub fn live(limit: i32) -> i32 {\n    limit + 1\n}\n",
            "#[test]\nfn reads_a_row() {\n    assert_eq!(live(1), 2);\n}\n",
        );

        let read = read("src/lib.rs", source);

        assert_eq!(declared(&read), [("fn", "live")]);
    }

    #[test]
    fn a_rust_test_module_contributes_no_definition() {
        let source = concat!(
            "pub fn live(limit: i32) -> i32 {\n    limit + 1\n}\n",
            "#[cfg(test)]\nmod tests {\n    fn helper(limit: i32) -> i32 {\n        limit\n    }\n}\n",
        );

        let read = read("src/lib.rs", source);

        assert_eq!(declared(&read), [("fn", "live")]);
    }

    #[test]
    fn a_marker_comment_removes_the_definition_after_it() {
        let source = concat!(
            "// sah:allow duplication the two shapes drift apart next week\n",
            "pub fn copied(limit: i32) -> i32 {\n    limit + 1\n}\n",
            "pub fn other(limit: i32) -> i32 {\n    limit + 1\n}\n",
        );

        let read = read("src/lib.rs", source);

        assert_eq!(
            declared(&read),
            [("fn", "other")],
            "the marker reaches one definition only"
        );
    }

    #[test]
    fn a_marker_comment_reaches_past_the_attributes_of_the_definition_it_exempts() {
        let source = concat!(
            "// sah:allow duplication the derive stays\n",
            "#[derive(Debug)]\n",
            "pub struct Row {\n    pub width: usize,\n}\n",
        );

        let read = read("src/lib.rs", source);

        assert!(read.definitions.is_empty(), "{:?}", declared(&read));
    }

    #[test]
    fn a_marker_comment_is_honored_in_the_python_comment_syntax() {
        let source = concat!(
            "# sah:allow duplication the two shapes drift apart next week\n",
            "def copied(limit):\n    return limit + 1\n",
        );

        let read = read("src/band.py", source);

        assert!(read.definitions.is_empty(), "{:?}", declared(&read));
    }

    #[test]
    fn a_python_def_and_class_are_read_and_the_test_code_is_not() {
        let source = concat!(
            "class Row:\n    width = 0\n\n",
            "def live(limit):\n    return limit + 1\n\n",
            "def test_reads_a_row():\n    assert live(1) == 2\n\n",
            "class RowCase(unittest.TestCase):\n    def check(self):\n        return 1\n",
        );

        let read = read("src/band.py", source);

        assert_eq!(declared(&read), [("class", "Row"), ("def", "live")]);
    }

    #[test]
    fn a_typescript_function_method_interface_and_type_are_read() {
        let source = concat!(
            "export function fold(limit: number): number {\n    return limit + 1;\n}\n",
            "interface Row {\n    width: number;\n}\n",
            "type Pair = { a: number; b: number };\n",
            "class Band {\n    read(): number {\n        return 1;\n    }\n}\n",
        );

        let read = read("src/band.ts", source);

        assert_eq!(
            declared(&read),
            [
                ("function", "fold"),
                ("interface", "Row"),
                ("type", "Pair"),
                ("method", "read"),
            ]
        );
    }

    #[test]
    fn a_typescript_describe_block_contributes_no_definition() {
        let source = concat!(
            "export function live(): number {\n    return 1;\n}\n\n",
            "describe('rows', () => {\n",
            "    function helper(): number {\n        return 1;\n    }\n",
            "});\n",
        );

        let read = read("src/band.ts", source);

        assert_eq!(declared(&read), [("function", "live")]);
    }

    #[test]
    fn a_go_func_method_and_type_are_read_and_the_test_is_not() {
        let source = concat!(
            "package band\n\n",
            "type Row struct {\n\tName string\n}\n\n",
            "func Live() int {\n\treturn 1\n}\n\n",
            "func (r Row) Read() int {\n\treturn 1\n}\n\n",
            "func TestLive(t *testing.T) {\n\tif Live() != 1 {\n\t\tt.Fatal(\"no\")\n\t}\n}\n",
        );

        let read = read("band.go", source);

        assert_eq!(
            declared(&read),
            [("type", "Row"), ("func", "Live"), ("method", "Read")]
        );
    }

    #[test]
    fn a_swift_func_type_and_protocol_are_read_and_the_xctestcase_is_not() {
        let source = concat!(
            "func live() -> Int {\n    return 1\n}\n\n",
            "struct Row {\n    var width: Int\n}\n\n",
            "protocol Reads {\n    func read() -> Int\n}\n\n",
            "final class BandTests: XCTestCase {\n",
            "    func testReadsARow() {\n        XCTAssertEqual(live(), 1)\n    }\n}\n",
        );

        let read = read("Band.swift", source);

        assert_eq!(
            declared(&read),
            [("func", "live"), ("type", "Row"), ("protocol", "Reads")]
        );
    }

    #[test]
    fn a_java_method_is_read_and_the_annotated_test_is_not() {
        let source = concat!(
            "class Band {\n",
            "    int live() { return 1; }\n\n",
            "    @Test\n    void readsARow() {\n        assertEquals(1, live());\n    }\n",
            "}\n",
        );

        let read = read("Band.java", source);

        assert_eq!(declared(&read), [("class", "Band"), ("method", "live")]);
    }

    #[test]
    fn a_csharp_method_is_read_and_the_fact_is_not() {
        let source = concat!(
            "class Band {\n",
            "    int Live() { return 1; }\n\n",
            "    [Fact]\n    void ReadsARow() {\n        Assert.Equal(1, Live());\n    }\n",
            "}\n",
        );

        let read = read("Band.cs", source);

        assert_eq!(declared(&read), [("class", "Band"), ("method", "Live")]);
    }

    #[test]
    fn a_c_function_and_struct_are_read_and_the_gtest_is_not() {
        let source = concat!(
            "struct Row { int width; };\n",
            "int live(int a) { return a + 1; }\n\n",
            "TEST(BandSuite, ReadsARow) {\n    EXPECT_EQ(live(1), 2);\n}\n",
        );

        let read = read("band.cpp", source);

        assert_eq!(declared(&read), [("struct", "Row"), ("function", "live")]);
    }

    #[test]
    fn a_c_struct_drops_its_field_names_and_keeps_its_field_types() {
        let source = "struct Row { int width; int height; };\n";

        let shape = shape_named(&read("band.c", source), "Row");

        assert!(!shape.contains(&"width".to_string()), "{shape:?}");
        assert!(shape.contains(&"int".to_string()), "{shape:?}");
    }

    #[test]
    fn a_ruby_method_is_read_and_the_describe_block_is_not() {
        let source = concat!(
            "def live\n  1\nend\n\n",
            "describe 'rows' do\n  def helper\n    1\n  end\nend\n",
        );

        let read = read("band.rb", source);

        assert_eq!(declared(&read), [("def", "live")]);
    }

    #[test]
    fn a_php_function_and_class_are_read_and_the_test_class_is_not() {
        let source = concat!(
            "<?php\n",
            "function live() { return 1; }\n",
            "class BandTest extends TestCase {\n",
            "    function testReadsARow() {\n        $this->assertEquals(1, 1);\n    }\n",
            "}\n",
        );

        let read = read("Band.php", source);

        assert_eq!(declared(&read), [("function", "live")]);
    }

    #[test]
    fn an_elixir_def_is_read_and_the_test_block_is_not() {
        let source = concat!(
            "defmodule Band do\n",
            "  def live(limit) do\n    limit + 1\n  end\n",
            "  defp read(limit) do\n    limit\n  end\n",
            "end\n\n",
            "defmodule BandTest do\n",
            "  test \"reads a row\" do\n    assert Band.live(1) == 2\n  end\n",
            "end\n",
        );

        let read = read("band.ex", source);

        assert_eq!(declared(&read), [("def", "live"), ("defp", "read")]);
    }

    #[test]
    fn a_bash_function_is_read() {
        let source = "live() {\n  local band=1\n  echo $band\n}\n";

        let read = read("band.sh", source);

        assert_eq!(declared(&read), [("function", "live")]);
    }

    #[test]
    fn a_fortran_function_and_subroutine_are_read() {
        let source = concat!(
            "function fold(a) result(b)\n  integer :: a, b\n  b = a\nend function fold\n",
            "subroutine emit(a)\n  integer :: a\nend subroutine emit\n",
        );

        let read = read("band.f90", source);

        assert_eq!(
            declared(&read),
            [("function", "fold"), ("subroutine", "emit")]
        );
    }
}
