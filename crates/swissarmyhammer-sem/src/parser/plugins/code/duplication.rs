//! The tokens and the exemptions one file contributes to the verbatim
//! duplicate gate.
//!
//! The detector that pairs the tokens lives in
//! `swissarmyhammer-code-context`. This module owns the grammar half of the
//! question, and only that half: which text is a code token, which
//! definitions are test code, and which blocks a marker comment exempts.
//! Every one of the three is read off the tree-sitter parse. None of them
//! reads the file's path.

use std::ops::Range;

use tree_sitter::Node;

use super::parse_code;

/// The marker comment that exempts the block after it from the gate.
///
/// One form, in whatever comment syntax the language spells. The text is what
/// counts and the delimiter never does, so `// sah:allow duplication <reason>`,
/// `# sah:allow duplication <reason>` and
/// `/* sah:allow duplication <reason> */` all say the same thing.
pub const DUPLICATION_ALLOW_MARKER: &str = "sah:allow duplication";

/// One position in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPoint {
    /// The one-based line.
    pub line: usize,
    /// The zero-based column, counted in bytes.
    pub column: usize,
    /// The byte offset from the start of the file.
    pub offset: usize,
}

/// One code token of a file.
///
/// The token carries where it is and not what it says: the caller already
/// holds the source the parse was made from, and slices
/// `source[start.offset..end.offset]` for the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicationToken {
    /// Where the token starts.
    pub start: TokenPoint,
    /// One byte past where the token ends.
    pub end: TokenPoint,
}

/// One file's contribution to the verbatim duplicate gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicationSource {
    /// The language id the grammar roster routed the file to.
    pub language: &'static str,
    /// Every code token of the file, in source order, comments left out.
    pub tokens: Vec<DuplicationToken>,
    /// The byte ranges no finding may start inside: every test definition,
    /// and every block a marker comment exempts.
    pub exempt: Vec<Range<usize>>,
}

impl DuplicationSource {
    /// Whether a block that starts at `offset` sits in exempted code.
    ///
    /// The start of the block decides, so a copy that begins inside a test
    /// definition is exempt however far past its end it runs, and a copy that
    /// begins in production code is reported however far it runs into one.
    pub fn exempts(&self, offset: usize) -> bool {
        self.exempt.iter().any(|range| range.contains(&offset))
    }
}

/// Read the tokens and the exemptions of `source`, parsed as the language the
/// grammar roster routes `path` to.
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
/// let read = duplication_source("src/lib.rs", "fn one() {}\n").ok_or("rust is mapped")?;
/// assert_eq!(read.language, "rust");
/// assert!(duplication_source("notes.txt", "fn one() {}\n").is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn duplication_source(path: &str, source: &str) -> Option<DuplicationSource> {
    let parsed = parse_code(path, source)?;
    let root = parsed.tree().root_node();
    let spec = test_spec(parsed.language());

    let mut tokens = Vec::new();
    collect_tokens(root, &mut tokens);
    let mut exempt = Vec::new();
    collect_exemptions(root, spec, source, &mut exempt);

    Some(DuplicationSource {
        language: parsed.language(),
        tokens,
        exempt,
    })
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

/// Every code token under `node`, in source order.
fn collect_tokens(node: Node<'_>, out: &mut Vec<DuplicationToken>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_comment_kind(child.kind()) {
            continue;
        }
        if child.child_count() > 0 {
            collect_tokens(child, out);
            continue;
        }
        if child.start_byte() < child.end_byte() {
            out.push(token_of(child));
        }
    }
}

/// The token `node` is.
fn token_of(node: Node<'_>) -> DuplicationToken {
    DuplicationToken {
        start: TokenPoint {
            line: node.start_position().row + 1,
            column: node.start_position().column,
            offset: node.start_byte(),
        },
        end: TokenPoint {
            line: node.end_position().row + 1,
            column: node.end_position().column,
            offset: node.end_byte(),
        },
    }
}

/// Every exempted byte range under `node`.
fn collect_exemptions(node: Node<'_>, spec: &TestSpec, source: &str, out: &mut Vec<Range<usize>>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(range) = exemption_of(child, spec, source) {
            out.push(range);
            continue;
        }
        collect_exemptions(child, spec, source, out);
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

/// Whether `node` is the marker comment that exempts the block after it.
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
/// decorate it, so a clone that starts at `#[test]` is inside the definition
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

/// The name `node` declares, following the `declarator` chain the C-family
/// grammars nest a function's name inside.
fn definition_name<'a>(node: Node<'_>, source: &'a str) -> Option<&'a str> {
    if let Some(name) = node.child_by_field_name("name") {
        return Some(node_text(name, source));
    }
    let mut declarator = node.child_by_field_name("declarator")?;
    while let Some(inner) = declarator.child_by_field_name("declarator") {
        declarator = inner;
    }
    Some(node_text(declarator, source))
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
    use super::*;

    /// The text `source` holds over `range`.
    fn exempted<'a>(read: &DuplicationSource, source: &'a str) -> Vec<&'a str> {
        read.exempt
            .iter()
            .map(|range| &source[range.clone()])
            .collect()
    }

    /// Whether any exempted range holds `needle`.
    fn exempts_text(read: &DuplicationSource, source: &str, needle: &str) -> bool {
        exempted(read, source)
            .iter()
            .any(|text| text.contains(needle))
    }

    /// The read of `source`, parsed as the language `path` names.
    fn read(path: &str, source: &str) -> DuplicationSource {
        duplication_source(path, source).expect("the roster maps this extension")
    }

    #[test]
    fn a_file_the_roster_does_not_claim_is_not_read() {
        assert!(duplication_source("notes.txt", "fn one() {}\n").is_none());
    }

    #[test]
    fn tokens_carry_one_based_lines_and_their_own_text() {
        let source = "fn one() {}\n";

        let tokens = read("src/lib.rs", source).tokens;

        let texts: Vec<&str> = tokens
            .iter()
            .map(|token| &source[token.start.offset..token.end.offset])
            .collect();
        assert_eq!(texts, ["fn", "one", "(", ")", "{", "}"]);
        assert!(tokens.iter().all(|token| token.start.line == 1));
    }

    #[test]
    fn a_comment_contributes_no_token() {
        let source = "// a note about one\nfn one() {}\n";

        let tokens = read("src/lib.rs", source).tokens;

        let texts: Vec<&str> = tokens
            .iter()
            .map(|token| &source[token.start.offset..token.end.offset])
            .collect();
        assert_eq!(texts, ["fn", "one", "(", ")", "{", "}"]);
    }

    #[test]
    fn a_rust_test_module_is_exempt_together_with_its_attribute() {
        let source = concat!(
            "pub fn live() {}\n\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    pub fn helper() {}\n",
            "}\n",
        );

        let read = read("src/lib.rs", source);

        let exempted = exempted(&read, source);
        assert_eq!(exempted.len(), 1, "one range: {exempted:?}");
        assert!(
            exempted[0].starts_with("#[cfg(test)]"),
            "the attribute must be inside the range: {}",
            exempted[0]
        );
        assert!(exempted[0].contains("fn helper"));
        assert!(!read.exempts(0), "live code stays reportable");
    }

    #[test]
    fn a_rust_test_function_is_exempt_together_with_its_attribute() {
        let source = concat!(
            "#[test]\n",
            "fn reads_a_row() {\n",
            "    assert_eq!(1, 1);\n",
            "}\n",
        );

        let read = read("src/lib.rs", source);

        assert!(exempts_text(&read, source, "fn reads_a_row"));
        assert!(read.exempts(0), "the attribute itself is inside the range");
    }

    #[test]
    fn a_rust_path_qualified_test_attribute_is_exempt() {
        let source = concat!(
            "#[tokio::test]\n",
            "async fn reads_a_row() {\n",
            "    assert_eq!(1, 1);\n",
            "}\n",
        );

        let read = read("src/lib.rs", source);

        assert!(exempts_text(&read, source, "fn reads_a_row"));
    }

    #[test]
    fn a_rust_derive_attribute_is_not_a_test_marker() {
        let source = "#[derive(Debug)]\npub struct Row;\n";

        let read = read("src/lib.rs", source);

        assert!(read.exempt.is_empty(), "{:?}", exempted(&read, source));
    }

    #[test]
    fn a_marker_comment_exempts_the_item_after_it() {
        let source = concat!(
            "// sah:allow duplication the two shapes drift apart next week\n",
            "pub fn copied() {\n",
            "    let total = 1;\n",
            "}\n\n",
            "pub fn other() {}\n",
        );

        let read = read("src/lib.rs", source);

        assert!(exempts_text(&read, source, "fn copied"));
        assert!(
            !exempts_text(&read, source, "fn other"),
            "the marker reaches one item only: {:?}",
            exempted(&read, source)
        );
    }

    #[test]
    fn a_marker_comment_reaches_past_the_attributes_of_the_item_it_exempts() {
        let source = concat!(
            "// sah:allow duplication the derive stays\n",
            "#[derive(Debug)]\n",
            "pub struct Row {\n",
            "    pub width: usize,\n",
            "}\n",
        );

        let read = read("src/lib.rs", source);

        assert!(exempts_text(&read, source, "pub width"));
    }

    #[test]
    fn a_marker_comment_is_honored_in_the_python_comment_syntax() {
        let source = concat!(
            "# sah:allow duplication the two shapes drift apart next week\n",
            "def copied():\n",
            "    return 1\n",
        );

        let read = read("src/band.py", source);

        assert!(exempts_text(&read, source, "def copied"));
    }

    #[test]
    fn a_python_test_function_and_a_unittest_class_are_exempt() {
        let source = concat!(
            "def live():\n    return 1\n\n",
            "def test_reads_a_row():\n    assert live() == 1\n\n",
            "class RowCase(unittest.TestCase):\n    def check(self):\n        return 1\n",
        );

        let read = read("src/band.py", source);

        assert!(exempts_text(&read, source, "def test_reads_a_row"));
        assert!(exempts_text(&read, source, "class RowCase"));
        assert!(!exempts_text(&read, source, "def live"));
    }

    #[test]
    fn a_typescript_describe_block_is_exempt() {
        let source = concat!(
            "export function live(): number {\n    return 1;\n}\n\n",
            "describe('rows', () => {\n",
            "    it('reads a row', () => {\n",
            "        expect(live()).toBe(1);\n",
            "    });\n",
            "});\n",
        );

        let read = read("src/band.ts", source);

        assert!(exempts_text(&read, source, "describe('rows'"));
        assert!(!exempts_text(&read, source, "export function live"));
    }

    #[test]
    fn a_go_test_function_is_exempt() {
        let source = concat!(
            "package band\n\n",
            "func Live() int { return 1 }\n\n",
            "func TestLive(t *testing.T) {\n",
            "\tif Live() != 1 {\n\t\tt.Fatal(\"no\")\n\t}\n",
            "}\n",
        );

        let read = read("band.go", source);

        assert!(exempts_text(&read, source, "func TestLive"));
        assert!(!exempts_text(&read, source, "func Live()"));
    }

    #[test]
    fn a_java_annotated_test_is_exempt() {
        let source = concat!(
            "class Band {\n",
            "    int live() { return 1; }\n\n",
            "    @Test\n",
            "    void readsARow() {\n",
            "        assertEquals(1, live());\n",
            "    }\n",
            "}\n",
        );

        let read = read("Band.java", source);

        assert!(exempts_text(&read, source, "void readsARow"));
        assert!(!exempts_text(&read, source, "int live()"));
    }

    #[test]
    fn a_csharp_fact_is_exempt() {
        let source = concat!(
            "class Band {\n",
            "    int Live() { return 1; }\n\n",
            "    [Fact]\n",
            "    void ReadsARow() {\n",
            "        Assert.Equal(1, Live());\n",
            "    }\n",
            "}\n",
        );

        let read = read("Band.cs", source);

        assert!(exempts_text(&read, source, "void ReadsARow"));
        assert!(!exempts_text(&read, source, "int Live()"));
    }

    #[test]
    fn a_cpp_gtest_definition_is_exempt() {
        let source = concat!(
            "int live() { return 1; }\n\n",
            "TEST(BandSuite, ReadsARow) {\n",
            "    EXPECT_EQ(live(), 1);\n",
            "}\n",
        );

        let read = read("band.cpp", source);

        assert!(exempts_text(&read, source, "TEST(BandSuite"));
        assert!(!exempts_text(&read, source, "int live()"));
    }

    #[test]
    fn a_swift_xctestcase_is_exempt() {
        let source = concat!(
            "func live() -> Int { return 1 }\n\n",
            "final class BandTests: XCTestCase {\n",
            "    func testReadsARow() {\n",
            "        XCTAssertEqual(live(), 1)\n",
            "    }\n",
            "}\n",
        );

        let read = read("Band.swift", source);

        assert!(exempts_text(&read, source, "class BandTests"));
        assert!(!exempts_text(&read, source, "func live()"));
    }

    #[test]
    fn a_ruby_describe_block_is_exempt() {
        let source = concat!(
            "def live\n  1\nend\n\n",
            "describe 'rows' do\n",
            "  it 'reads a row' do\n",
            "    expect(live).to eq(1)\n",
            "  end\n",
            "end\n",
        );

        let read = read("band.rb", source);

        assert!(exempts_text(&read, source, "describe 'rows'"));
        assert!(!exempts_text(&read, source, "def live"));
    }

    #[test]
    fn an_elixir_test_block_is_exempt() {
        let source = concat!(
            "defmodule Band do\n",
            "  def live, do: 1\n",
            "end\n\n",
            "defmodule BandTest do\n",
            "  test \"reads a row\" do\n",
            "    assert Band.live() == 1\n",
            "  end\n",
            "end\n",
        );

        let read = read("band.ex", source);

        assert!(exempts_text(&read, source, "test \"reads a row\""));
        assert!(!exempts_text(&read, source, "def live"));
    }

    #[test]
    fn a_php_test_method_is_exempt() {
        let source = concat!(
            "<?php\n",
            "class BandTest extends TestCase {\n",
            "    function testReadsARow() {\n",
            "        $this->assertEquals(1, 1);\n",
            "    }\n",
            "}\n",
        );

        let read = read("Band.php", source);

        assert!(exempts_text(&read, source, "class BandTest"));
    }

    #[test]
    fn a_language_with_no_definition_marker_exempts_nothing() {
        let source = "live() {\n  echo 1\n}\n";

        let read = read("band.sh", source);

        assert!(read.exempt.is_empty(), "{:?}", exempted(&read, source));
    }

    #[test]
    fn exempts_reads_the_start_of_a_block_and_not_its_end() {
        let source = concat!(
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    pub fn helper() {}\n",
            "}\n",
            "pub fn live() {}\n",
        );

        let read = read("src/lib.rs", source);

        let live = source
            .find("pub fn live")
            .expect("the live item is present");
        assert!(read.exempts(0));
        assert!(!read.exempts(live));
    }
}
