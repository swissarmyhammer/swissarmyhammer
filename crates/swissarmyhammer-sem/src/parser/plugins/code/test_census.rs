//! What each test function in a file actually measures, computed from the parse.
//!
//! A test that asserts nothing, a test the runner skips, and a test whose body
//! is an empty pair of braces all pass every time. They read as coverage and
//! prove nothing. This module finds them as a pure measurement over a
//! tree-sitter parse, so the review rule compares rows instead of counting
//! calls by eye.
//!
//! # What a test is
//!
//! A test is whatever
//! [`is_test_definition`] says it is — an attribute at the definition
//! (`#[test]`, `@Test`), a framework
//! name+signature convention (`func TestX(t *testing.T)`, `def test_foo`), or a
//! call-based definition (ExUnit's `test "..." do`, jest's `it("...", () =>
//! {})`). That judgement is read off the one [`DefinitionSpec`] roster, and it
//! never consults the file name. A helper in a file called `foo_test.rs` is not
//! a test here.
//!
//! # What a body is worth
//!
//! [`TEST_CENSUS_SPECS`] is the vocabulary half: one row per language naming the
//! words an assertion is spelled with, the markers that skip a test, and the
//! node kinds that catch a failure. Adding a language is adding a row.
//!
//! A language absent from that roster is **not measured** — `test_census`
//! returns `None` — and a caller must report "not computed" rather than "no
//! suspect tests". The distinction is not academic: Elixir's `test "..." do` IS
//! a test definition the [`definitions`](mod@super::definitions) roster recognizes,
//! yet the language has no row here, so mapping its vocabulary would be needed
//! before a `.exs` file could ever read as clean.

use tree_sitter::Node;

use super::definitions::{
    attribute_marker_name, child_by_field_or_kind, defining_call, definition_attributes,
    for_each_function, function_header, function_name, is_test_definition, node_text,
    spec_for_language, DefinitionSpec, MAX_TRAVERSAL_DEPTH,
};
use crate::parser::plugins::code::ParsedCode;

/// One defect measured in a test function's body.
///
/// Every variant is a fact about the parse. Whether the fact makes the test
/// *cheating* is the reviewer's judgement — a `#[should_panic]` test asserts
/// through the panic, and a test whose assertions live in a shared helper is
/// honest while measuring zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestDefect {
    /// A marker at the definition, or a call in the body, keeps the runner from
    /// running this test.
    Skipped,
    /// The body holds no statement at all.
    EmptyBody,
    /// The body holds comments and nothing else.
    CommentsOnly,
    /// The body runs code, but none of it is spelled like an assertion.
    NoAssertions,
    /// A catch/except/rescue block swallows the failure without asserting
    /// anything about it.
    SwallowedFailure,
}

impl TestDefect {
    /// The measure as an evidence row states it.
    pub fn detail(self) -> &'static str {
        match self {
            Self::Skipped => {
                "skipped: a marker at the definition or a call in the body keeps the \
                              runner from running it"
            }
            Self::EmptyBody => "empty: the body holds no statement",
            Self::CommentsOnly => "commented out: the body holds comments and nothing else",
            Self::NoAssertions => "no assertion: the body runs code, none of it an assertion",
            Self::SwallowedFailure => {
                "swallowed: a catch/except/rescue block asserts nothing about the failure it caught"
            }
        }
    }
}

impl std::fmt::Display for TestDefect {
    /// The measure as an evidence row states it — [`TestDefect::detail`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.detail())
    }
}

/// One test function, and everything its body measured.
///
/// An entry with no defects is a positive measurement — this test asserts
/// something and the runner runs it — so a caller filters rather than assuming
/// the list holds only suspects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCensus {
    /// The test's name as the source spells it.
    pub name: String,
    /// The 1-based line the definition starts on.
    pub start_line: usize,
    /// What the body measured, in [`TestDefect`] declaration order.
    pub defects: Vec<TestDefect>,
}

/// Measure every test function in a parse the caller already holds.
///
/// Returns `None` — meaning **not measured** — when the parse's language has no
/// grammar row to find test definitions with, or no test vocabulary to read a
/// body with. A caller must report "not computed" for `None` and never
/// substitute an empty list, which would read as "every test in this file
/// asserts something".
///
/// `source` must be the text the parse was made from; the node ranges index
/// into it.
pub fn test_census(parsed: &ParsedCode, source: &str) -> Option<Vec<TestCensus>> {
    let definitions = spec_for_language(parsed.language())?;
    let census = census_spec_for_language(parsed.language())?;
    let mut measured = Vec::new();
    for_each_function(
        parsed.tree().root_node(),
        source,
        definitions,
        0,
        &mut |node| {
            if is_test_definition(node, source, definitions) {
                measured.push(measure_test(node, source, definitions, census));
            }
        },
    );
    Some(measured)
}

/// One language's test vocabulary: how an assertion, a skip, a comment, and a
/// swallowed failure are spelled in it.
///
/// One row read as data, beside the [`DefinitionSpec`] row that finds the test
/// definitions themselves. A language is mapped here only once both halves
/// work for it.
struct TestCensusSpec {
    /// The language id, mirroring the [`DefinitionSpec`] row it pairs with.
    language: &'static str,
    /// The field (or, through [`child_by_field_or_kind`], the child kind)
    /// holding a definition's body. A definition with no such child has an
    /// empty body — Ruby's `def test_empty; end` carries no `body_statement` at
    /// all.
    body_field: &'static str,
    /// Node kinds that spell an identifier, whose text is matched against
    /// [`Self::assertion_words`] and [`Self::skip_words`].
    name_kinds: &'static [&'static str],
    /// Node kinds that spell a comment.
    comment_kinds: &'static [&'static str],
    /// Node kinds that ARE an assertion whatever they name — a language whose
    /// `assert` is a statement rather than a call.
    assertion_kinds: &'static [&'static str],
    /// Lower-case words an identifier in the body must contain to count as an
    /// assertion. Deliberately generous: a helper named `assert_state` counts,
    /// because a census that accuses an honest test is worse than one that
    /// misses a lazy one.
    assertion_words: &'static [&'static str],
    /// Lower-case marker names at the definition that skip the test —
    /// the last path segment of an attribute/annotation, so `#[ignore]`,
    /// `@Ignore`, and `@Disabled("why")` all reduce to one word.
    skip_markers: &'static [&'static str],
    /// Lower-case words an identifier in the body must contain for the test to
    /// skip itself at run time — Go's `t.Skip()`, minitest's `skip`.
    skip_words: &'static [&'static str],
    /// Node kinds that catch a failure, and so can swallow one.
    catch_kinds: &'static [&'static str],
}

/// Shared field values for every language whose comments, bodies, and
/// identifiers follow the C-like shape: one `body` field, one `identifier`
/// kind, and no `assert` statement of its own.
const CENSUS_SPEC_DEFAULTS: TestCensusSpec = TestCensusSpec {
    language: "",
    body_field: "body",
    name_kinds: &["identifier"],
    comment_kinds: &["comment", "line_comment", "block_comment"],
    assertion_kinds: &[],
    assertion_words: &[],
    skip_markers: &[],
    skip_words: &[],
    catch_kinds: &[],
};

/// Rust. `#[test]`/`#[tokio::test]` marks the definition and `#[ignore]` skips
/// it, both `attribute_item` siblings. Every assertion in the language and in
/// its test frameworks is a macro or helper whose name carries `assert`
/// (`assert!`, `assert_eq!`, `debug_assert!`, `assert_matches!`,
/// `logs_assert`), read off the `macro_invocation`'s `macro: (identifier)`.
/// Rust has no catch construct, so nothing swallows.
const RUST_CENSUS: TestCensusSpec = TestCensusSpec {
    language: "rust",
    assertion_words: &["assert"],
    skip_markers: &["ignore"],
    ..CENSUS_SPEC_DEFAULTS
};

/// Go. `func TestX(t *testing.T)` marks the definition. `go test` has no
/// assertion of its own: a failure is reported through the `testing.T` handle
/// (`t.Error`, `t.Errorf`, `t.Fatal`, `t.Fatalf`) or through the testify
/// helpers (`assert.Equal`, `require.NoError`), all of them a
/// `field_identifier` or `identifier` leaf. `t.Skip()` skips at run time; Go
/// has no attribute to mark it with.
const GO_CENSUS: TestCensusSpec = TestCensusSpec {
    language: "go",
    name_kinds: &["identifier", "field_identifier"],
    assertion_words: &["assert", "require", "error", "fatal", "expect"],
    skip_words: &["skip"],
    ..CENSUS_SPEC_DEFAULTS
};

/// Java. `@Test` marks the method; `@Ignore` (JUnit 4) and `@Disabled`
/// (JUnit 5) skip it. Assertions are the JUnit/AssertJ/Hamcrest families
/// (`assertEquals`, `assertThat`, `Assertions.assertAll`) plus Mockito's
/// `verify`, and the language's own `assert` statement is an assertion whatever
/// it names. A `catch_clause` that asserts nothing swallows the failure.
const JAVA_CENSUS: TestCensusSpec = TestCensusSpec {
    language: "java",
    assertion_kinds: &["assert_statement"],
    assertion_words: &["assert", "verify", "expect"],
    skip_markers: &["ignore", "disabled"],
    catch_kinds: &["catch_clause"],
    ..CENSUS_SPEC_DEFAULTS
};

/// Ruby. minitest's `def test_foo` marks the definition, and `skip` inside the
/// body skips it. Assertions are minitest's `assert_*`/`refute_*` and RSpec's
/// `expect`/`must_*`. A `method` with an empty body carries no `body_statement`
/// child at all, which [`child_by_field_or_kind`] reports as a missing body and
/// the census reads as empty.
const RUBY_CENSUS: TestCensusSpec = TestCensusSpec {
    language: "ruby",
    name_kinds: &["identifier", "constant"],
    assertion_words: &["assert", "refute", "expect", "must_"],
    skip_words: &["skip"],
    catch_kinds: &["rescue"],
    ..CENSUS_SPEC_DEFAULTS
};

/// Python. pytest's and `unittest`'s `def test_foo` marks the definition, and
/// a `@pytest.mark.skip`/`@pytest.mark.skipif`/`@pytest.mark.xfail` or
/// `@unittest.skip` decorator skips it — each one reducing to its last path
/// segment, and `skip` covering `skipif` too because the marker match is a
/// substring one. The language spells its own assertion as an
/// `assert_statement` rather than as a call, so that kind is an assertion
/// whatever it names; the rest are pytest's `pytest.raises` context manager
/// and `unittest`'s `assertEqual` family, both an `identifier` leaf. An
/// `except_clause` that asserts nothing swallows the failure.
const PYTHON_CENSUS: TestCensusSpec = TestCensusSpec {
    language: "python",
    assertion_kinds: &["assert_statement"],
    assertion_words: &["assert", "raises", "expect"],
    skip_markers: &["skip", "xfail"],
    catch_kinds: &["except_clause"],
    ..CENSUS_SPEC_DEFAULTS
};

/// The JavaScript family — JavaScript, TypeScript, and TSX, whose grammars
/// spell all three of these the same way. jest and mocha define a test as a
/// call, so the definition the census measures is the callback that call takes
/// (`it("...", () => { ... })`), and the marker that skips it is the callee
/// itself rather than an annotation: `it.skip`, `test.skip`, `xit`, and `xtest`
/// all reduce to one of the words below through the substring match. Assertions
/// are jest's `expect(...)`, node's and chai's `assert`, and chai's `should`
/// style, which is a `property_identifier` rather than a leading call. A
/// `catch_clause` that asserts nothing swallows the failure.
const fn javascript_family_census(language: &'static str) -> TestCensusSpec {
    TestCensusSpec {
        language,
        name_kinds: &["identifier", "property_identifier"],
        assertion_words: &["assert", "expect", "should"],
        skip_markers: &["skip", "xit", "xtest"],
        catch_kinds: &["catch_clause"],
        ..CENSUS_SPEC_DEFAULTS
    }
}

/// JavaScript — [`javascript_family_census`].
const JAVASCRIPT_CENSUS: TestCensusSpec = javascript_family_census("javascript");

/// TypeScript — [`javascript_family_census`].
const TYPESCRIPT_CENSUS: TestCensusSpec = javascript_family_census("typescript");

/// TSX — [`javascript_family_census`].
const TSX_CENSUS: TestCensusSpec = javascript_family_census("tsx");

/// Every language the census can measure a test body in.
///
/// A language absent here is "not measured", never "no suspect tests".
static TEST_CENSUS_SPECS: &[&TestCensusSpec] = &[
    &RUST_CENSUS,
    &GO_CENSUS,
    &JAVA_CENSUS,
    &RUBY_CENSUS,
    &PYTHON_CENSUS,
    &JAVASCRIPT_CENSUS,
    &TYPESCRIPT_CENSUS,
    &TSX_CENSUS,
];

/// The census vocabulary for a language id, or `None` when it has no mapping.
fn census_spec_for_language(language: &str) -> Option<&'static TestCensusSpec> {
    TEST_CENSUS_SPECS
        .iter()
        .find(|spec| spec.language == language)
        .copied()
}

/// Measure one test definition.
fn measure_test(
    node: Node<'_>,
    source: &str,
    definitions: &DefinitionSpec,
    census: &TestCensusSpec,
) -> TestCensus {
    TestCensus {
        name: function_name(node, source, definitions),
        start_line: node.start_position().row + 1,
        defects: body_defects(node, source, definitions, census),
    }
}

/// Everything one test's body measured, in [`TestDefect`] declaration order.
///
/// An empty or comments-only body already says there is no assertion, so
/// [`TestDefect::NoAssertions`] is reported only for a body that runs code.
fn body_defects(
    node: Node<'_>,
    source: &str,
    definitions: &DefinitionSpec,
    census: &TestCensusSpec,
) -> Vec<TestDefect> {
    let statements = body_statements(node, definitions, census);
    let mut defects = Vec::new();
    if is_skipped(node, source, definitions, census) {
        defects.push(TestDefect::Skipped);
    }
    if statements.is_empty() {
        defects.push(TestDefect::EmptyBody);
    } else if statements
        .iter()
        .all(|statement| census.comment_kinds.contains(&statement.kind()))
    {
        defects.push(TestDefect::CommentsOnly);
    } else if !statements
        .iter()
        .any(|statement| asserts_anything(*statement, source, census))
    {
        defects.push(TestDefect::NoAssertions);
    }
    if swallows_a_failure(node, source, census) {
        defects.push(TestDefect::SwallowedFailure);
    }
    defects
}

/// The statements one definition's body holds.
///
/// Empty when the grammar attaches no body node at all (Ruby's `def test_empty;
/// end`). The definition's own name and parameters are excluded, because a
/// grammar that carries no body node keeps the body's statements as siblings of
/// them.
fn body_statements<'t>(
    node: Node<'t>,
    definitions: &DefinitionSpec,
    census: &TestCensusSpec,
) -> Vec<Node<'t>> {
    let Some(body) = child_by_field_or_kind(node, census.body_field) else {
        return Vec::new();
    };
    let header = function_header(node, definitions);
    let signature = [definitions.name_field, definitions.parameters_field]
        .map(|field| header.child_by_field_name(field).map(|child| child.id()));
    let mut cursor = body.walk();
    let statements = body
        .named_children(&mut cursor)
        .filter(|child| !signature.contains(&Some(child.id())))
        .collect();
    drop(cursor);
    statements
}

/// Whether a marker at the definition, or a call in the body, keeps the runner
/// from running this test.
///
/// A marker at the definition is an attribute/annotation (`#[ignore]`,
/// `@Disabled`) or, where a call defines the test, that call's own callee —
/// jest spells a skipped test `it.skip("...", ...)` and mocha spells it
/// `xit("...", ...)`, both of which carry the marker in the name they are
/// called by.
fn is_skipped(
    node: Node<'_>,
    source: &str,
    definitions: &DefinitionSpec,
    census: &TestCensusSpec,
) -> bool {
    let marked = definition_attributes(node, definitions)
        .into_iter()
        .filter_map(|attribute| attribute_marker_name(attribute, source))
        .chain(defining_call(node, definitions, source).map(|(_, target)| target))
        .any(|marker| word_matches(marker, census.skip_markers));
    marked || names_in(node, source, census).any(|name| word_matches(name, census.skip_words))
}

/// Whether anything inside `node` is spelled like an assertion.
fn asserts_anything(node: Node<'_>, source: &str, census: &TestCensusSpec) -> bool {
    let mut found = false;
    for_each_descendant(node, 0, &mut |current| {
        found = found
            || census.assertion_kinds.contains(&current.kind())
            || (census.name_kinds.contains(&current.kind())
                && node_text(current, source)
                    .is_some_and(|text| word_matches(text, census.assertion_words)));
    });
    found
}

/// Whether any catch/except/rescue block in the body asserts nothing about the
/// failure it caught.
fn swallows_a_failure(node: Node<'_>, source: &str, census: &TestCensusSpec) -> bool {
    let mut swallows = false;
    for_each_descendant(node, 0, &mut |current| {
        swallows = swallows
            || (census.catch_kinds.contains(&current.kind())
                && !asserts_anything(current, source, census));
    });
    swallows
}

/// Every identifier `node` holds, in traversal order.
fn names_in<'a>(
    node: Node<'_>,
    source: &'a str,
    census: &TestCensusSpec,
) -> impl Iterator<Item = &'a str> {
    let mut names = Vec::new();
    for_each_descendant(node, 0, &mut |current| {
        if census.name_kinds.contains(&current.kind()) {
            names.extend(node_text(current, source));
        }
    });
    names.into_iter()
}

/// Whether `text` carries any of `words`, compared without case.
///
/// A substring match rather than a whole-name match, so one word covers a
/// family: `assert` covers `assert_eq!`, `assertEquals`, and `XCTAssertTrue`
/// alike, and no per-framework spelling list is needed.
fn word_matches(text: &str, words: &[&str]) -> bool {
    let lowered = text.to_ascii_lowercase();
    words.iter().any(|word| lowered.contains(word))
}

/// Visit `node` and every descendant of it, stopping at
/// [`MAX_TRAVERSAL_DEPTH`] for the same reason [`for_each_function`] stops: a
/// pathological tree must never take the native call stack down.
fn for_each_descendant<'t>(node: Node<'t>, depth: u32, visit: &mut dyn FnMut(Node<'t>)) {
    if depth > MAX_TRAVERSAL_DEPTH {
        return;
    }
    visit(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        for_each_descendant(child, depth + 1, visit);
    }
}

#[cfg(test)]
mod tests {
    use super::{test_census, TestCensus, TestDefect};
    use crate::parser::plugins::code::parse_code;

    /// Measure one sample file through the real parse-and-census path.
    fn census(path: &str, source: &str) -> Vec<TestCensus> {
        let parsed = parse_code(path, source).expect("the sample's language is on the roster");
        test_census(&parsed, source).expect("the sample's language has a census mapping")
    }

    /// What the census measured for one named test.
    fn defects_of(measured: &[TestCensus], name: &str) -> Vec<TestDefect> {
        measured
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("`{name}` was measured, got: {measured:?}"))
            .defects
            .clone()
    }

    /// A Rust module holding one honest test, one that asserts nothing, one the
    /// runner skips, one empty, one commented out, and a helper that is no test
    /// at all.
    const RUST_TESTS: &str = "\
        #[test]\n\
        fn asserts() { assert_eq!(1, 1); }\n\
        \n\
        #[test]\n\
        fn measures_nothing() { let value = compute(); drop(value); }\n\
        \n\
        #[test]\n\
        #[ignore]\n\
        fn skipped() { assert!(true); }\n\
        \n\
        #[test]\n\
        fn empty() {}\n\
        \n\
        #[test]\n\
        fn commented_out() {\n\
            // assert_eq!(1, 1);\n\
        }\n\
        \n\
        fn compute() -> u8 { 1 }\n";

    #[test]
    fn a_test_that_asserts_measures_no_defect() {
        assert_eq!(defects_of(&census("src/lib.rs", RUST_TESTS), "asserts"), []);
    }

    #[test]
    fn a_test_that_asserts_nothing_is_measured() {
        assert_eq!(
            defects_of(&census("src/lib.rs", RUST_TESTS), "measures_nothing"),
            [TestDefect::NoAssertions]
        );
    }

    #[test]
    fn an_ignore_marker_at_the_definition_is_measured_as_skipped() {
        assert_eq!(
            defects_of(&census("src/lib.rs", RUST_TESTS), "skipped"),
            [TestDefect::Skipped],
            "the test asserts, so `skipped` is the only measure"
        );
    }

    #[test]
    fn an_empty_body_is_measured_once_as_empty_rather_than_twice() {
        assert_eq!(
            defects_of(&census("src/lib.rs", RUST_TESTS), "empty"),
            [TestDefect::EmptyBody],
            "an empty body already says there is no assertion"
        );
    }

    #[test]
    fn a_body_of_comments_only_is_measured_as_commented_out() {
        assert_eq!(
            defects_of(&census("src/lib.rs", RUST_TESTS), "commented_out"),
            [TestDefect::CommentsOnly]
        );
    }

    /// The census reads the marker at the definition, never the file name: a
    /// helper beside the tests is not one of them.
    #[test]
    fn a_function_with_no_test_marker_is_never_measured() {
        let names: Vec<String> = census("src/lib.rs", RUST_TESTS)
            .into_iter()
            .map(|entry| entry.name)
            .collect();

        assert!(
            !names.contains(&"compute".to_string()),
            "only test definitions are measured, got: {names:?}"
        );
    }

    #[test]
    fn a_file_with_no_tests_measures_nothing() {
        assert_eq!(census("src/lib.rs", "pub fn one() -> u8 { 1 }\n"), []);
    }

    /// Go marks a test by name+signature and skips it with a call rather than a
    /// marker, so both halves of the skip measure are exercised.
    #[test]
    fn a_go_test_that_skips_itself_is_measured_as_skipped() {
        let measured = census(
            "a_test.go",
            "package a\n\nfunc TestSkipped(t *testing.T) {\n\tt.Skip(\"why\")\n}\n\nfunc \
             TestAsserts(t *testing.T) {\n\tt.Fatalf(\"bad\")\n}\n",
        );

        assert_eq!(
            defects_of(&measured, "TestSkipped"),
            [TestDefect::Skipped, TestDefect::NoAssertions]
        );
        assert_eq!(defects_of(&measured, "TestAsserts"), []);
    }

    /// Java's `catch` is what "swallowed failure" is about: the test passes
    /// whether or not the call under test threw.
    #[test]
    fn a_catch_block_that_asserts_nothing_is_measured_as_swallowing() {
        let measured = census(
            "A.java",
            "class A {\n  @Test\n  void swallows() {\n    try {\n      doThing();\n    } catch \
             (Exception e) {\n      // ignored\n    }\n  }\n\n  @Test\n  void rethrows() {\n    \
             try {\n      doThing();\n    } catch (Exception e) {\n      assertEquals(1, 1);\n    \
             }\n  }\n}\n",
        );

        assert_eq!(
            defects_of(&measured, "swallows"),
            [TestDefect::NoAssertions, TestDefect::SwallowedFailure]
        );
        assert_eq!(defects_of(&measured, "rethrows"), []);
    }

    #[test]
    fn a_disabled_annotation_is_measured_as_skipped() {
        let measured = census(
            "A.java",
            "class A {\n  @Test\n  @Disabled(\"flaky\")\n  void off() {\n    assertEquals(1, \
             1);\n  }\n}\n",
        );

        assert_eq!(defects_of(&measured, "off"), [TestDefect::Skipped]);
    }

    /// Ruby attaches no body node at all to an empty method, so the census
    /// reads a missing body as an empty one rather than as "not measured".
    #[test]
    fn a_ruby_test_with_no_body_node_is_measured_as_empty() {
        let measured = census(
            "a_test.rb",
            "class A\n  def test_empty\n  end\n\n  def test_asserts\n    assert_equal 1, 1\n  \
             end\nend\n",
        );

        assert_eq!(defects_of(&measured, "test_empty"), [TestDefect::EmptyBody]);
        assert_eq!(defects_of(&measured, "test_asserts"), []);
    }

    /// A pytest module holding one honest test, one that asserts nothing, and
    /// one the runner skips through a marker.
    const PYTHON_TESTS: &str = "\
        import pytest\n\
        \n\
        \n\
        def test_asserts():\n\
        \x20   assert build() == 1\n\
        \n\
        \n\
        def test_measures_nothing():\n\
        \x20   value = build()\n\
        \x20   print(value)\n\
        \n\
        \n\
        @pytest.mark.skip(\"flaky\")\n\
        def test_skipped():\n\
        \x20   assert build() == 1\n";

    /// Python spells its assertion as a statement rather than as a call, so the
    /// honest test proves the statement kind is read as well.
    #[test]
    fn a_python_test_that_asserts_nothing_is_measured() {
        let measured = census("test_a.py", PYTHON_TESTS);

        assert_eq!(
            defects_of(&measured, "test_measures_nothing"),
            [TestDefect::NoAssertions]
        );
        assert_eq!(defects_of(&measured, "test_asserts"), []);
    }

    #[test]
    fn a_pytest_skip_marker_is_measured_as_skipped() {
        assert_eq!(
            defects_of(&census("test_a.py", PYTHON_TESTS), "test_skipped"),
            [TestDefect::Skipped],
            "the test asserts, so `skipped` is the only measure"
        );
    }

    /// Elixir's `test "..." do` IS recognized as a test definition, but the
    /// language has no census vocabulary to read a body with, so the census must
    /// say "not measured" rather than "nothing suspect".
    #[test]
    fn a_language_with_no_census_mapping_is_not_measured() {
        let source = "defmodule ATest do\n  test \"works\" do\n    assert 1 == 1\n  end\nend\n";
        let parsed = parse_code("a_test.exs", source).expect("elixir is on the grammar roster");

        assert_eq!(test_census(&parsed, source), None);
    }

    /// A jest module holding one honest test, one whose body is empty, one the
    /// runner skips through the call itself, one nested in a `describe` suite,
    /// and a helper beside them that is no test at all.
    const JEST_TESTS: &str = "\
        it(\"asserts\", () => {\n\
        \x20   expect(build()).toBe(1);\n\
        });\n\
        \n\
        it(\"empty\", () => {});\n\
        \n\
        it.skip(\"skipped\", () => {\n\
        \x20   expect(build()).toBe(1);\n\
        });\n\
        \n\
        describe(\"group\", () => {\n\
        \x20   it(\"nested\", () => {\n\
        \x20       expect(build()).toBe(1);\n\
        \x20   });\n\
        });\n\
        \n\
        function build() {\n\
        \x20   return 1;\n\
        }\n";

    #[test]
    fn a_jest_test_that_asserts_measures_no_defect() {
        assert_eq!(defects_of(&census("a.test.js", JEST_TESTS), "asserts"), []);
    }

    #[test]
    fn a_jest_test_with_an_empty_body_is_measured_as_empty() {
        assert_eq!(
            defects_of(&census("a.test.js", JEST_TESTS), "empty"),
            [TestDefect::EmptyBody]
        );
    }

    #[test]
    fn a_jest_test_the_call_itself_skips_is_measured_as_skipped() {
        assert_eq!(
            defects_of(&census("a.test.js", JEST_TESTS), "skipped"),
            [TestDefect::Skipped],
            "the test asserts, so `skipped` is the only measure"
        );
    }

    /// The census reads the call that defines the test, never the file name: a
    /// plain function beside the tests is not one of them, and neither is a
    /// `describe` suite, whose own body holds tests rather than assertions.
    #[test]
    fn only_the_jest_tests_themselves_are_measured() {
        let names: Vec<String> = census("a.test.js", JEST_TESTS)
            .into_iter()
            .map(|entry| entry.name)
            .collect();

        assert_eq!(names, ["asserts", "empty", "skipped", "nested"]);
    }

    /// The census maps the whole JavaScript family, so the same jest module
    /// measures the same way whichever of the three extensions carries it.
    #[test]
    fn every_javascript_family_extension_measures_a_jest_test() {
        for path in ["a.test.js", "a.test.ts", "a.test.tsx"] {
            assert_eq!(
                defects_of(&census(path, JEST_TESTS), "empty"),
                [TestDefect::EmptyBody],
                "{path} measures the empty jest test"
            );
        }
    }
}
