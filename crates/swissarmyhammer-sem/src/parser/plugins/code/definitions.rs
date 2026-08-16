//! Where a function definition is, what it is named, and whether it is a test —
//! read from the tree-sitter parse rather than from the file name.
//!
//! This is the grammar layer the [`test_census`](mod@super::test_census) sits on.
//! It
//! answers three questions and no more:
//!
//! - which nodes in a parse ARE function definitions ([`for_each_function`]),
//! - what each one is called ([`function_name`]),
//! - whether the definition is marked as a test ([`is_test_definition`]).
//!
//! Nothing here measures a function. It locates one.
//!
//! # Language coverage
//!
//! Node kinds are per grammar, so each language needs a [`DefinitionSpec`] row.
//! A language with no row is **not mapped**: [`spec_for_language`] answers
//! [`None`] and a caller must report "not computed" rather than an empty
//! result, which would read as "this file holds no test at all".
//!
//! Every row is built the same way: parse a real sample in the target grammar
//! and read the s-expression it actually produces, then transcribe the node
//! kinds and field names verbatim. None of them is a guess.
//!
//! The roster here is the WIDER of the two halves a census needs. A language
//! reaches a measured result only when it carries a row here AND a vocabulary
//! row in [`test_census`](mod@super::test_census). Seven grammars — C, C++,
//! C#,
//! PHP, Fortran, Swift and Elixir — carry a definition row and no census
//! vocabulary yet, so a file in one of them reports "not measured". The rows
//! stay because they are the verified grammar mapping a census row is added
//! ON TOP of; deleting one would put that grammar work back to the start.
//!
//! # Test marking beyond attributes
//!
//! Most mapped grammars mark a test with an attribute/annotation/decorator
//! node. Three — Go, Ruby, Fortran — have no such grammar construct at all;
//! their real convention is name+signature based instead (Go's `func TestXxx(t
//! *testing.T)`, Ruby's minitest `def test_foo`, Fortran's FRUIT-style `test_*`
//! naming), checked by
//! [`DefinitionSpec::test_name_prefix`]/[`DefinitionSpec::test_param_type`] via
//! [`name_signature_marks_test`] rather than a per-language attribute branch.
//! Python has the decorator construct, but neither pytest nor `unittest` marks
//! a test with one: both read the `test_` prefix at the definition, so Python
//! carries a name prefix BESIDE its decorator kinds rather than instead of
//! them. Elixir needs neither: its ExUnit `test` block is itself a `call`
//! classified exactly like `def` ([`DefinitionSpec::call_target_test_kinds`]),
//! so being named `test` at the definition IS the marker.
//!
//! The JavaScript family marks a test the same way, one level out: jest and
//! mocha spell a test `it("...", () => { ... })`, a call whose callback holds
//! the body. The callback is the definition — it is where the statements are —
//! and the enclosing call's callee is its marker, reached by [`defining_call`]
//! off the same [`DefinitionSpec::call_target_test_kinds`] list Elixir uses. A
//! `describe` suite is not on that list, so only the tests themselves read as
//! tests.
//!
//! The prefix match is case-sensitive by default (Go, Python, and Ruby are all
//! case-sensitive languages, and `go test` itself requires the exact-case
//! `Test` prefix), but Fortran overrides it with
//! [`DefinitionSpec::test_name_case_insensitive`]: Fortran identifiers are
//! case-insensitive by language semantics, so `TEST_DEEPLY_NESTED` and
//! `test_deeply_nested` name the same subroutine and must both be recognized
//! as a FRUIT-style test.
//!
//! Bash has no attribute/annotation grammar construct either, and its one
//! real-world convention — bats-core's `# @test "description"` comment — is
//! unstructured free text inside a generic `comment` node, indistinguishable
//! by KIND from an ordinary doc comment or license header (verified by
//! parsing one). Treating any comment as a potential test marker would be
//! unsafe and overbroad, so Bash has no [`DefinitionSpec`] row and reports
//! not-mapped like any other unmapped language.

use tree_sitter::Node;

/// The deepest tree-sitter tree depth a walk descends before stopping rather
/// than continuing to recurse.
///
/// This layer runs on real diffs, including third-party repository content
/// parsed directly (see `mirdan/src/git_source.rs`). A pathologically deep but
/// finite source file — generated code, deeply nested literals from a
/// minifier — produces a parse tree deep enough to exhaust the native call
/// stack through [`for_each_function`], which mirrors the tree with one Rust
/// stack frame per level. That is a hard crash, not a graceful error, and it
/// would take down the whole review process rather than fail one file.
///
/// Real code rarely exceeds depth 10-20, so this cap is two orders of
/// magnitude above any plausible real file. It never touches legitimate code
/// while stopping the walk far short of exhausting even a small thread stack.
pub(super) const MAX_TRAVERSAL_DEPTH: u32 = 256;

/// The per-grammar node kinds a function definition and its test marker are
/// spelled with.
///
/// Every language is one row of data. The lookups are a single traversal
/// parameterized by the row — there is no per-language branch — so the node set
/// for a language is reviewable in one place instead of inferred from code.
pub(super) struct DefinitionSpec {
    /// The language id, mirroring the `LanguageConfig` id it is keyed to.
    language: &'static str,
    /// Node kinds that define a function. A function nested inside another is
    /// its own definition, never folded into its parent.
    function_kinds: &'static [&'static str],
    /// The field name holding a function's name, when the grammar names it.
    /// Resolved through [`resolve_declarator_name`], which unwraps a nested
    /// `declarator` field chain when the grammar needs one (C/C++'s
    /// `function_declarator`/`pointer_declarator`).
    pub(super) name_field: &'static str,
    /// The node kind of an attribute/annotation/decorator that can mark a
    /// function as a test, wherever the grammar attaches it: a preceding
    /// sibling ([`definition_attributes`]'s sibling scan) or a child of the
    /// definition itself, optionally one more level down inside
    /// [`Self::attribute_container_kinds`].
    attribute_kinds: &'static [&'static str],
    /// A wrapper node the grammar nests INSIDE the definition itself, holding
    /// [`Self::attribute_kinds`] as its children — Java's `modifiers`, C#'s
    /// `attribute_list`, PHP's `attribute_list`/`attribute_group`, C/C++'s
    /// `attribute_declaration`. Empty when the grammar never nests an
    /// attribute inside the definition (Rust, Python, TypeScript's sibling
    /// model; JavaScript's bare `decorator` field needs no unwrap at all).
    attribute_container_kinds: &'static [&'static str],

    // ---- name+signature test marking (go, ruby, fortran, python) ----
    //
    // Three grammars have no attribute/annotation node kind at all, so their
    // real test-marking convention is name+signature based instead: Go's
    // `func TestXxx(t *testing.T)`, Ruby's minitest `def test_foo`, and
    // Fortran's FRUIT-style `test_*` subroutine naming. Python has a
    // decorator kind, yet pytest and `unittest` both mark a test by the same
    // `test_` prefix, so it uses this mechanism BESIDE its decorator. One
    // generic mechanism — a name prefix, plus an optional first-parameter
    // type substring for grammars (Go) where the prefix alone is ambiguous
    // with an ordinary helper — covers all four, checked by
    // [`name_signature_marks_test`] rather than a per-language branch.
    /// The prefix a function's own name must start with to count as a
    /// name+signature test — Go's `"Test"`, Ruby's/Fortran's/Python's
    /// `"test_"`. `None` for every grammar that marks tests only through an
    /// attribute (or not at all).
    test_name_prefix: Option<&'static str>,
    /// A substring the function's FIRST parameter's own `type` field text
    /// must contain for [`Self::test_name_prefix`] to count — Go's
    /// `"testing.T"`, disambiguating a `TestXxx` HELPER (no such parameter)
    /// from a real `go test` entry point. `None` when the name prefix alone
    /// is enough (Ruby's, Fortran's conventions carry no required
    /// signature).
    test_param_type: Option<&'static str>,
    /// The field name holding a function's parameter list, read together
    /// with [`Self::test_param_type`]. Empty when [`Self::test_param_type`]
    /// is `None`.
    pub(super) parameters_field: &'static str,
    /// Whether [`Self::test_name_prefix`] matching in
    /// [`name_signature_marks_test`] must ignore case — Fortran's, whose
    /// identifiers are case-insensitive by language semantics (`TEST_FOO`,
    /// `test_foo`, and `Test_Foo` all name the same subroutine), unlike Go's,
    /// Ruby's, and Python's, all case-sensitive languages where `go
    /// test`/minitest/pytest require the exact-case prefix. `false` for every
    /// other grammar, including Go, Ruby, and Python.
    test_name_case_insensitive: bool,

    // ---- indirect header fields (fortran) ----
    //
    /// A child node kind that owns the function's OWN `name`/parameters
    /// fields when the function-level container itself carries none —
    /// Fortran's `subroutine`/`function` wrap a
    /// `subroutine_statement`/`function_statement` child that actually owns
    /// `name:`/`parameters:`/`type:` (verified: `subroutine` and
    /// `subroutine_statement` both list `name` in `node-types.json`, but only
    /// the STATEMENT child is ever the field's OWNER — the wrapping
    /// `subroutine` node's own field list is empty). Empty for every other
    /// mapped grammar, whose function-level node owns its own fields
    /// directly; [`function_header`] is the identity function when this is
    /// empty.
    header_child_kinds: &'static [&'static str],

    // ---- call-classified definitions (elixir, the javascript family) ----
    //
    // Elixir represents `def`/`defp`/`defmacro`/`defmacrop` and ExUnit's
    // `test` as one generic `call` node — verified by parsing `defmodule Foo
    // do def pick(a, b) do ... end end` and reading the labelled tree: every
    // one of `defmodule`/`def` is a `call` node with `target: (identifier)`,
    // distinguished ONLY by that target's text. This breaks
    // `function_kinds.contains(&node.kind())` — EVERY call in the file has
    // kind `"call"`, definitions and ordinary calls alike.
    /// The grammar's call node kind — Elixir's `"call"`, the JS family's
    /// `"call_expression"`, each verified by parsing a sample and reading the
    /// labelled tree. Empty for a grammar that spells no definition as a call,
    /// where every call-based lookup below reports nothing, because no node's
    /// kind is the empty string.
    call_kind: &'static str,
    /// The field on a [`Self::call_kind`] node holding the callee — Elixir's
    /// `"target"`, the JS family's `"function"`. [`call_target_text`] reads it
    /// by name and has no second, per-grammar lookup beside it.
    callee_field: &'static str,
    /// The callee node kinds whose text names the call. Elixir reads only
    /// `identifier`, so `Mod.fun()` — whose target is a `dot` node, verified —
    /// is never misread as a definition. The JS family reads
    /// `member_expression` as well, because jest spells a skipped test
    /// `it.skip(...)`, whose callee is a member expression reading `it.skip`.
    callee_kinds: &'static [&'static str],
    /// The set of callee texts that reclassify a call node's EFFECTIVE kind
    /// (see [`effective_kind`]) to that text, so
    /// [`Self::function_kinds`] can name it like any other definition kind.
    /// An ordinary call (`Repo.insert(a)`, `foo()`) is never affected: its
    /// callee is either not one of [`Self::callee_kinds`] or an identifier
    /// whose text is not in this list. Empty for every grammar whose
    /// definitions are real dedicated node kinds, which is every grammar but
    /// Elixir's.
    call_target_kinds: &'static [&'static str],
    /// The callee texts whose mere identity marks a call-based definition as a
    /// test, needing no attribute lookup at all: being named that at the
    /// definition IS the marker. Elixir's `"test"` is ExUnit's real macro
    /// (`test "description" do ... end`, verified as a `call` node exactly like
    /// `def`), and is a [`Self::call_target_kinds`] member too because Elixir
    /// classifies the whole definition off its callee. The JS family's are
    /// jest/mocha's `it`/`test` and the runner's own `.only`/`.skip`/`x`-prefixed
    /// filters; none of them reclassifies anything, so the JS family leaves
    /// [`Self::call_target_kinds`] empty.
    call_target_test_kinds: &'static [&'static str],
    /// Function kinds a call-based definition takes as its BODY argument rather
    /// than BEING — jest/mocha's `it("...", () => { ... })`, where the call
    /// carries the marker and the callback carries the statements, so the
    /// callback is the definition. A callback sits directly in the call's
    /// argument list (`arrow_function` inside `arguments` inside
    /// `call_expression`, verified), which is how [`defining_call`] reaches its
    /// call. Empty for Elixir, whose `test "..." do ... end` IS the definition
    /// node.
    test_callback_kinds: &'static [&'static str],
}

/// The value every field of a row takes unless the grammar needs otherwise.
///
/// Each row below sets only the fields that actually differ and inherits the
/// rest through struct-update syntax (`..SPEC_DEFAULTS`), instead of repeating
/// the same dozen field values in every definition. Its own `language` and
/// `function_kinds` are placeholders, never read: every row sets both, and this
/// constant is never used by itself.
const SPEC_DEFAULTS: DefinitionSpec = DefinitionSpec {
    language: "",
    function_kinds: &[],
    name_field: "",
    attribute_kinds: &[],
    attribute_container_kinds: &[],
    test_name_prefix: None,
    test_param_type: None,
    parameters_field: "",
    test_name_case_insensitive: false,
    header_child_kinds: &[],
    call_kind: "",
    callee_field: "",
    callee_kinds: &[],
    call_target_kinds: &[],
    call_target_test_kinds: &[],
    test_callback_kinds: &[],
};

/// Rust. Verified against `tree_sitter_rust` by parsing samples covering every
/// listed kind — the node names below are the grammar's, not guesses. `#[test]`
/// and `#[tokio::test]` are `attribute_item` siblings of the definition.
static RUST_SPEC: DefinitionSpec = DefinitionSpec {
    language: "rust",
    function_kinds: &["function_item"],
    name_field: "name",
    attribute_kinds: &["attribute_item"],
    ..SPEC_DEFAULTS
};

/// Shared field values for TypeScript, TSX, and JavaScript. All three
/// grammars are C-like and produce identical node kinds for every field
/// except the language id itself, confirmed by parsing the same jest and
/// decorator samples under each grammar.
///
/// A jest/mocha test is a call — `it("...", () => { ... })` — rather than a
/// declaration, and the call is not the function: the grammar hangs the body on
/// the `arrow_function`/`function_expression` the call takes as its second
/// argument (verified). So the callback is the definition, marked a test
/// through the enclosing call's callee
/// ([`DefinitionSpec::call_target_test_kinds`] reached by [`defining_call`]) and
/// named by the call's description string. `describe` is deliberately absent
/// from that list: a suite asserts nothing itself, and marking its callback a
/// test would read ordinary code as a test.
const fn typescript_family_spec(language: &'static str) -> DefinitionSpec {
    DefinitionSpec {
        language,
        function_kinds: &[
            "function_declaration",
            "method_definition",
            "arrow_function",
            "function_expression",
        ],
        name_field: "name",
        attribute_kinds: &["decorator"],
        call_kind: "call_expression",
        callee_field: "function",
        callee_kinds: &["identifier", "member_expression"],
        call_target_test_kinds: &[
            "it",
            "it.only",
            "it.skip",
            "test",
            "test.only",
            "test.skip",
            "xit",
            "xtest",
        ],
        test_callback_kinds: &["arrow_function", "function_expression"],
        ..SPEC_DEFAULTS
    }
}

/// TypeScript. Verified against `tree_sitter_typescript` (the `LANGUAGE_TYPESCRIPT`
/// grammar). Its decorator is a sibling of the `method_definition` it marks
/// inside `class_body` — unlike JavaScript's, which nests it as a field of the
/// method itself — confirmed by parsing a two-method class with only one
/// decorated.
static TYPESCRIPT_SPEC: DefinitionSpec = typescript_family_spec("typescript");

/// TSX. Verified against `tree_sitter_typescript` (the `LANGUAGE_TSX` grammar)
/// by parsing the same jest and decorator samples used for TypeScript — the
/// node kinds are identical; only the JSX-extended grammar differs, and none of
/// the samples used JSX syntax.
static TSX_SPEC: DefinitionSpec = typescript_family_spec("tsx");

/// JavaScript. Verified against `tree_sitter_javascript`. Its decorator is a
/// `decorator:` field of the `method_definition` itself — unlike TypeScript's
/// sibling placement — confirmed by parsing a decorated class method and
/// reading the field name on the s-expression.
static JAVASCRIPT_SPEC: DefinitionSpec = typescript_family_spec("javascript");

/// Python. Verified against `tree_sitter_python`.
///
/// Both pytest and `unittest` mark a test by the `test_` prefix at the
/// definition rather than by a decorator, so the spec carries
/// [`DefinitionSpec::test_name_prefix`] `"test_"` as well — a plain name
/// prefix with no signature check, exactly as Ruby's minitest convention does.
/// The `@...test` decorator branch stays: a `decorator` is a preceding named
/// sibling inside the `decorated_definition` wrapper (confirmed by parsing
/// `@pytest.mark.skip("why")` above a `def` and reading the s-expression).
static PYTHON_SPEC: DefinitionSpec = DefinitionSpec {
    language: "python",
    function_kinds: &["function_definition"],
    name_field: "name",
    attribute_kinds: &["decorator"],
    test_name_prefix: Some("test_"),
    ..SPEC_DEFAULTS
};

/// Java. Verified against `tree_sitter_java`. Its annotation sits inside the
/// method's own `modifiers` child rather than as a preceding sibling
/// (confirmed by parsing `@Test`/`@Test(timeout = 100)` and reading the exact
/// byte span of each node).
static JAVA_SPEC: DefinitionSpec = DefinitionSpec {
    language: "java",
    function_kinds: &["method_declaration"],
    name_field: "name",
    attribute_kinds: &["marker_annotation", "annotation"],
    attribute_container_kinds: &["modifiers"],
    ..SPEC_DEFAULTS
};

/// Shared field values for C and C++. Their definition and attribute node
/// kinds are identical — confirmed by parsing the same samples under each
/// grammar.
const fn c_family_spec(language: &'static str) -> DefinitionSpec {
    DefinitionSpec {
        language,
        function_kinds: &["function_definition"],
        name_field: "declarator",
        attribute_kinds: &["attribute"],
        attribute_container_kinds: &["attribute_declaration"],
        ..SPEC_DEFAULTS
    }
}

/// C. Verified against `tree_sitter_c`. A function's name sits several
/// `declarator` fields deep (`function_definition` names its
/// `function_declarator`, which names the plain identifier — one more
/// `pointer_declarator` level for a pointer-returning function), resolved
/// generically by [`resolve_declarator_name`] rather than a C-specific
/// special case.
static C_SPEC: DefinitionSpec = c_family_spec("c");

/// C++. Verified against `tree_sitter_cpp`. Its attribute uses the C++11
/// `[[...]]` syntax (`attribute_declaration` wrapping `attribute`, confirmed by
/// parsing `[[nodiscard]]`), the same shape C's does.
static CPP_SPEC: DefinitionSpec = c_family_spec("cpp");

/// C#. Verified against `tree_sitter_c_sharp`. Its attribute sits inside the
/// method's own `attribute_list` child (confirmed by parsing `[Test]`/`[Fact]`
/// and reading each node's exact byte span).
static CSHARP_SPEC: DefinitionSpec = DefinitionSpec {
    language: "csharp",
    function_kinds: &["method_declaration"],
    name_field: "name",
    attribute_kinds: &["attribute"],
    attribute_container_kinds: &["attribute_list"],
    ..SPEC_DEFAULTS
};

/// PHP. Verified against `tree_sitter_php` (the `LANGUAGE_PHP` grammar). Its
/// attribute is nested two container levels deep (`attributes: (attribute_list
/// (attribute_group (attribute ...)))`, confirmed on both a free function and a
/// class method) — PHPUnit's real `#[Test]` attribute marker.
static PHP_SPEC: DefinitionSpec = DefinitionSpec {
    language: "php",
    function_kinds: &["function_definition", "method_declaration"],
    name_field: "name",
    attribute_kinds: &["attribute"],
    attribute_container_kinds: &["attribute_list", "attribute_group"],
    ..SPEC_DEFAULTS
};

/// Go. Verified against `tree_sitter_go`. Go has no attribute/annotation node
/// kind at all (confirmed while mapping every other node kind — no such kind
/// appeared), so its real test convention — `func TestXxx(t *testing.T)` — is
/// name+signature based instead: [`DefinitionSpec::test_name_prefix`] `"Test"`
/// plus [`DefinitionSpec::test_param_type`] `"testing.T"` on the first
/// parameter's own `type` field (confirmed by parsing `func TestAdd(t
/// *testing.T) {...}` and reading the parameter's `type: (pointer_type
/// (qualified_type package: (package_identifier) name: (type_identifier)))`),
/// so an ordinary `TestXxx` HELPER with no such parameter is never mistaken
/// for a real `go test` entry point.
static GO_SPEC: DefinitionSpec = DefinitionSpec {
    language: "go",
    function_kinds: &["function_declaration", "method_declaration"],
    name_field: "name",
    test_name_prefix: Some("Test"),
    test_param_type: Some("testing.T"),
    parameters_field: "parameters",
    ..SPEC_DEFAULTS
};

/// Ruby. Verified against `tree_sitter_ruby`. Ruby has no attribute/annotation
/// node kind, so its real test convention — minitest's `def test_foo` — is
/// name-prefix based, with no signature check needed
/// ([`DefinitionSpec::test_param_type`] `None`, confirmed: minitest test
/// methods take no fixed parameter).
static RUBY_SPEC: DefinitionSpec = DefinitionSpec {
    language: "ruby",
    function_kinds: &["method", "singleton_method"],
    name_field: "name",
    test_name_prefix: Some("test_"),
    ..SPEC_DEFAULTS
};

/// Fortran. Verified against `tree_sitter_fortran`. Its `subroutine`/
/// `function` wrap a `subroutine_statement`/`function_statement` child that
/// owns the real `name`/`parameters` fields (confirmed via `node-types.json`:
/// the wrapping node's own field list is empty, resolved generically by
/// [`function_header`]). Fortran has no attribute/annotation node kind either;
/// its real test convention — FRUIT's `test_*` subroutine naming — is
/// name-prefix based like Ruby's, with no signature check needed, and
/// case-insensitive because Fortran identifiers are.
static FORTRAN_SPEC: DefinitionSpec = DefinitionSpec {
    language: "fortran",
    function_kinds: &["subroutine", "function"],
    name_field: "name",
    test_name_prefix: Some("test_"),
    test_name_case_insensitive: true,
    header_child_kinds: &["subroutine_statement", "function_statement"],
    ..SPEC_DEFAULTS
};

/// Swift. Verified against `tree_sitter_swift`. It DOES have a genuine,
/// current test-marking mechanism — `@Test` parses as `modifiers >
/// attribute`, matching the real Swift Testing framework (confirmed by
/// parsing `@Test\nfunc deeplyNested(...)`).
static SWIFT_SPEC: DefinitionSpec = DefinitionSpec {
    language: "swift",
    function_kinds: &["function_declaration"],
    name_field: "name",
    attribute_kinds: &["attribute"],
    attribute_container_kinds: &["modifiers"],
    ..SPEC_DEFAULTS
};

/// Elixir. Verified against `tree_sitter_elixir`. `def`/`defp`/`defmacro`/
/// `defmacrop` and ExUnit's `test` are ALL generic `call` nodes with `target:
/// (identifier)`, distinguished only by that target's text (confirmed by
/// parsing `defmodule Foo do def pick(a, b) do ... end end` and reading the
/// labelled tree) — reclassified to that text by [`effective_kind`] via
/// [`DefinitionSpec::call_target_kinds`] rather than any per-language special
/// case. `def`'s/`test`'s own name is not a field of the `call` at all; it is
/// read from `arguments` by [`call_function_name`] instead, which
/// [`function_name`] reaches through [`defining_call`] — the `call` IS the
/// definition here, so it is its own defining call. ExUnit's `test
/// "description" do ... end` is itself a `call` with target `"test"` — no
/// attribute lookup needed, being named `test` at the definition IS the marker
/// ([`DefinitionSpec::call_target_test_kinds`]).
static ELIXIR_SPEC: DefinitionSpec = DefinitionSpec {
    language: "elixir",
    function_kinds: &["def", "defp", "defmacro", "defmacrop", "test"],
    call_kind: "call",
    callee_field: "target",
    callee_kinds: &["identifier"],
    call_target_kinds: &["def", "defp", "defmacro", "defmacrop", "test"],
    call_target_test_kinds: &["test"],
    ..SPEC_DEFAULTS
};

/// Every language with a definition mapping. A language absent here is "not
/// mapped", never an empty answer.
static DEFINITION_SPECS: &[&DefinitionSpec] = &[
    &RUST_SPEC,
    &TYPESCRIPT_SPEC,
    &TSX_SPEC,
    &JAVASCRIPT_SPEC,
    &PYTHON_SPEC,
    &JAVA_SPEC,
    &C_SPEC,
    &CPP_SPEC,
    &CSHARP_SPEC,
    &PHP_SPEC,
    &GO_SPEC,
    &RUBY_SPEC,
    &FORTRAN_SPEC,
    &SWIFT_SPEC,
    &ELIXIR_SPEC,
];

/// The spec for a language id, or `None` when that language has no mapping.
///
/// Reads [`DEFINITION_SPECS`], a slice of REFERENCES to [`DefinitionSpec`],
/// which is why the body ends in `.copied()`. Three same-named siblings read
/// three other tables of three other types, two of them slices of values.
/// Sharing one body costs a trait and four impls to save four lines — see the
/// `parser::plugins::code` module doc.
pub(super) fn spec_for_language(language: &str) -> Option<&'static DefinitionSpec> {
    DEFINITION_SPECS
        .iter()
        .find(|s| s.language == language)
        .copied()
}

/// Visit every function definition at or under `node`, in source order.
///
/// The one place "what counts as a function definition here" is decided, read
/// off the [`DefinitionSpec`] row rather than by each caller walking the tree
/// its own way.
pub(super) fn for_each_function<'t>(
    node: Node<'t>,
    source: &str,
    spec: &DefinitionSpec,
    depth: u32,
    visit: &mut dyn FnMut(Node<'t>),
) {
    if depth > MAX_TRAVERSAL_DEPTH {
        // Stop descending rather than risk the native call stack. A function
        // nested this deep in surrounding structure is unreachable in any
        // real file; the walk simply never finds it.
        return;
    }
    if spec
        .function_kinds
        .contains(&effective_kind(node, spec, source))
    {
        visit(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        for_each_function(child, source, spec, depth + 1, visit);
    }
}

/// The node's classification for a [`DefinitionSpec::function_kinds`] check:
/// its own grammar KIND, unless it is a `call` node with a bare identifier
/// `target` naming one of [`DefinitionSpec::call_target_kinds`] — Elixir, whose
/// `def`/`defp`/`defmacro`/`test` are ALL generic `call` nodes. Verified by
/// parsing `defmodule Foo do def pick(a, b) do ... end end` and reading the
/// labelled tree: `defmodule` and `def` are each a `call` node with `target:
/// (identifier)`, distinguished only by that target's text — and an ordinary
/// call (`Repo.insert(a)`) has a `target` of kind `dot`, not `identifier`, so it
/// is never misclassified. A no-op for every other mapped grammar, whose
/// [`DefinitionSpec::call_target_kinds`] is empty.
fn effective_kind<'s>(node: Node<'_>, spec: &DefinitionSpec, source: &'s str) -> &'s str {
    if spec.call_target_kinds.is_empty() {
        return node.kind();
    }
    match call_target_text(node, spec, source) {
        Some(text) if spec.call_target_kinds.contains(&text) => text,
        _ => node.kind(),
    }
}

/// The callee's text of a call node, or `None` when `node` is not this
/// grammar's [`DefinitionSpec::call_kind`], or its callee is not one of the
/// [`DefinitionSpec::callee_kinds`] the grammar's row reads.
///
/// The field holding the callee is the grammar's own
/// ([`DefinitionSpec::callee_field`]: Elixir's `target`, the JS family's
/// `function`), so one lookup serves every call-based grammar.
fn call_target_text<'s>(node: Node<'_>, spec: &DefinitionSpec, source: &'s str) -> Option<&'s str> {
    if node.kind() != spec.call_kind {
        return None;
    }
    let callee = node.child_by_field_name(spec.callee_field)?;
    if !spec.callee_kinds.contains(&callee.kind()) {
        return None;
    }
    node_text(callee, source)
}

/// The call node that DEFINES the function at `node`, together with its callee
/// text, or `None` when no call defines it.
///
/// Two shapes, both read off the same [`DefinitionSpec`] row. The definition IS
/// the call — Elixir's `def`/`test`, whose whole classification comes from the
/// callee. Or the definition is the callback the call takes as an argument —
/// jest/mocha's `it("...", () => { ... })`, where
/// [`DefinitionSpec::test_callback_kinds`] names the callback kinds and the
/// callback sits directly in the call's argument list, putting the call exactly
/// two levels up (`arrow_function` inside `arguments` inside `call_expression`,
/// verified against the JS grammar).
///
/// A call whose callee names no definition is never one, so an ordinary
/// callback keeps its own identity: `arr.map((value) => value + 1)` reports
/// `None`, and its callback stays an anonymous function rather than becoming a
/// test.
pub(super) fn defining_call<'t, 's>(
    node: Node<'t>,
    spec: &DefinitionSpec,
    source: &'s str,
) -> Option<(Node<'t>, &'s str)> {
    let call = if node.kind() == spec.call_kind {
        node
    } else if spec.test_callback_kinds.contains(&node.kind()) {
        node.parent()?.parent()?
    } else {
        return None;
    };
    let target = call_target_text(call, spec, source)?;
    let defines =
        spec.call_target_kinds.contains(&target) || spec.call_target_test_kinds.contains(&target);
    defines.then_some((call, target))
}

/// The quote characters a grammar wraps a string literal's own text in. Elixir
/// writes a test description `"..."`; JavaScript accepts `'...'` for the same
/// literal, so both are stripped to leave the description itself.
const STRING_QUOTES: [char; 2] = ['"', '\''];

/// The name of a call-based function definition (Elixir's `def`/`defp`/
/// `defmacro`/`defmacrop`, ExUnit's `test`, or jest/mocha's `it`/`test`) from
/// its `arguments` field: verified as one of three shapes — the first argument
/// is a nested `call` naming a parameterized function (whose own `target` is the
/// name, e.g. `def pick(a, b)`), a bare `identifier` naming an arity-0 function
/// (`def zero`), or (`test "description" do ... end`, `it("description", ...)`)
/// a `string` naming the test directly.
fn call_function_name(node: Node<'_>, source: &str) -> Option<String> {
    let args = child_by_field_or_kind(node, "arguments")?;
    let mut cursor = args.walk();
    let first = args.named_children(&mut cursor).next();
    drop(cursor);
    match first?.kind() {
        "call" => first?
            .child_by_field_name("target")
            .and_then(|t| node_text(t, source))
            .map(String::from),
        "identifier" => node_text(first?, source).map(String::from),
        "string" => node_text(first?, source).map(|s| s.trim_matches(STRING_QUOTES).to_string()),
        _ => None,
    }
}

/// The function's declared name, or `<anonymous>`.
///
/// A definition a call makes is named by that call's arguments — Elixir's `def
/// pick(a, b)` and jest's `it("adds up", ...)` alike — because neither node
/// carries a `name` field of its own. Every other definition is named by
/// [`DefinitionSpec::name_field`].
pub(super) fn function_name(node: Node<'_>, source: &str, spec: &DefinitionSpec) -> String {
    if let Some((call, _)) = defining_call(node, spec, source) {
        if let Some(name) = call_function_name(call, source) {
            return name;
        }
    }
    let header = function_header(node, spec);
    header
        .child_by_field_name(spec.name_field)
        .and_then(|n| resolve_declarator_name(n, source))
        .unwrap_or("<anonymous>")
        .to_string()
}

/// The node that owns a function's own `name`/[`DefinitionSpec::parameters_field`]
/// fields — `node` itself, unless [`DefinitionSpec::header_child_kinds`] names
/// a child that owns them instead (Fortran's `subroutine`/`function`, which
/// wrap a `subroutine_statement`/`function_statement` child that is the real
/// field owner — verified via `node-types.json`: the wrapping node's own
/// field list is empty).
pub(super) fn function_header<'t>(node: Node<'t>, spec: &DefinitionSpec) -> Node<'t> {
    if spec.header_child_kinds.is_empty() {
        return node;
    }
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .find(|c| spec.header_child_kinds.contains(&c.kind()));
    drop(cursor);
    found.unwrap_or(node)
}

/// Resolve a name-field value down to its leaf text, unwrapping the nested
/// `declarator` field chain C/C++ use for pointer/function/array declarators
/// (`int *get_pointer()` names a `pointer_declarator`, whose own `declarator`
/// field names the `function_declarator`, whose own `declarator` field
/// finally names the plain identifier). Every other mapped language's name
/// field already points straight at a leaf, which has no `declarator` field
/// of its own, so the first lookup already terminates there.
fn resolve_declarator_name<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
    match node.child_by_field_name("declarator") {
        Some(inner) => resolve_declarator_name(inner, source),
        None => node_text(node, source),
    }
}

/// The source text a node spans, when the byte range is valid UTF-8 boundaries.
///
/// This copy answers `Option`. The text is compared against test markers, so an
/// empty string would read as "this is not a test" and would report a test
/// function as ordinary code. The `duplication` copy answers `""` on purpose,
/// because a chunk must still hash. The four contracts of this name are
/// recorded in the `parser::plugins::code` module doc.
pub(super) fn node_text<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
    source.get(node.start_byte()..node.end_byte())
}

/// Fetch `node`'s child named `name`: by FIELD first
/// (`child_by_field_name`), then — when the grammar has no such field at
/// all — by matching a named child's own KIND against `name` instead.
/// Elixir's `call` node declares only a `target` field, verified via
/// `node-types.json`: its `arguments` and `do_block` are ordinary
/// POSITIONAL named children whose KIND happens to be spelled the same as
/// the concept they hold (confirmed directly against the compiled grammar:
/// `field_name_for_named_child` returns `None` for both, even though their
/// own `kind()` reads `"arguments"`/`"do_block"`), so a caller reads them by
/// kind through this same lookup instead of needing an Elixir-specific
/// field/kind flag. A no-op fallback for every other mapped grammar, whose
/// fields genuinely exist and so are always found on the first branch.
pub(super) fn child_by_field_or_kind<'t>(node: Node<'t>, name: &str) -> Option<Node<'t>> {
    if let Some(found) = node.child_by_field_name(name) {
        return Some(found);
    }
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find(|c| c.kind() == name);
    drop(cursor);
    found
}

/// Whether the definition is marked as a test, wherever the grammar attaches
/// the marker: a contiguous run of preceding siblings (Rust's `#[attr]`,
/// Python's decorator inside the shared `decorated_definition` wrapper,
/// TypeScript's decorator as a `class_body` sibling), a direct child of the
/// definition itself (JavaScript's bare `decorator` field), or a child
/// wrapped in a container the grammar nests inside the definition (Java's
/// `modifiers`, C#'s `attribute_list`, PHP's `attribute_list`/
/// `attribute_group`, C/C++'s `attribute_declaration`) — or, where the grammar
/// spells a test as a call, the callee of the call that defines it (Elixir's
/// `test "..." do`, jest/mocha's `it("...", () => { ... })`), read through
/// [`defining_call`]. The file name is never consulted.
///
/// The `duplication` function of the same name takes a `TestSpec` and ORs four
/// `marked_by_*` readings instead. The two share a name and nothing else — see
/// the `parser::plugins::code` module doc.
pub(super) fn is_test_definition(node: Node<'_>, source: &str, spec: &DefinitionSpec) -> bool {
    if name_signature_marks_test(node, source, spec) {
        return true;
    }
    if let Some((_, target_text)) = defining_call(node, spec, source) {
        if spec.call_target_test_kinds.contains(&target_text) {
            return true;
        }
    }

    definition_attributes(node, spec)
        .into_iter()
        .any(|attribute| attribute_marks_test(attribute, source))
}

/// Every attribute/annotation/decorator node the grammar attaches to the
/// definition at `node`.
///
/// Two placements, both of which every mapped grammar uses one of: a contiguous
/// run of preceding siblings (Rust's `#[attr]`, Python's decorator inside the
/// shared `decorated_definition` wrapper, TypeScript's decorator as a
/// `class_body` sibling), and a named child of the definition itself —
/// directly (JavaScript's bare `decorator`) or through however many
/// [`DefinitionSpec::attribute_container_kinds`] wrapper levels the grammar
/// nests (PHP two deep, `attribute_list` > `attribute_group` > `attribute`;
/// Java/C#/C/C++ one).
///
/// Collecting the nodes rather than answering one question about them is what
/// lets the test marker and the [`test_census`](mod@super::test_census)'s skip
/// markers be read off the same traversal.
pub(super) fn definition_attributes<'t>(node: Node<'t>, spec: &DefinitionSpec) -> Vec<Node<'t>> {
    let mut attributes = Vec::new();
    let mut sibling = node.prev_named_sibling();
    while let Some(current) = sibling {
        if !spec.attribute_kinds.contains(&current.kind()) {
            break;
        }
        attributes.push(current);
        sibling = current.prev_named_sibling();
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_attributes(child, spec, &mut attributes);
    }
    drop(cursor);
    attributes
}

/// Push `node` onto `out` when it is an attribute, or descend into it when it
/// is one of the wrapper levels a grammar nests attributes inside.
fn collect_attributes<'t>(node: Node<'t>, spec: &DefinitionSpec, out: &mut Vec<Node<'t>>) {
    if spec.attribute_kinds.contains(&node.kind()) {
        out.push(node);
        return;
    }
    if spec.attribute_container_kinds.contains(&node.kind()) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect_attributes(child, spec, out);
        }
        drop(cursor);
    }
}

/// Whether one attribute/annotation/decorator node names the `test` marker,
/// however the grammar spells it: Rust's `#[test]`/`#[tokio::test]`, Python's
/// `@pytest.mark.test`, Java's `@Test`, C#'s `[Test]`, PHP's real PHPUnit
/// `#[Test]`, JavaScript/TypeScript's `@Test`, or C/C++'s `[[test]]`. The
/// last `.`/`::`/`\`-separated path segment must be exactly `test`
/// (case-insensitively, to cover both Rust's lowercase convention and Java/
/// C#/PHP's capitalized one), so `#[serial_test::serial]` and
/// `#[test_case(..)]` are not tests.
fn attribute_marks_test(node: Node<'_>, source: &str) -> bool {
    attribute_marker_name(node, source).is_some_and(|last| last.eq_ignore_ascii_case("test"))
}

/// The marker one attribute/annotation/decorator node names: its last
/// `.`/`::`/`\`-separated path segment, with the grammar's punctuation and any
/// argument list stripped.
///
/// `#[tokio::test]` reduces to `test`, `#[ignore]` to `ignore`, and
/// `@Disabled("flaky")` to `Disabled` — so one comparison recognizes a marker
/// however the language spells it, and `#[test_case(..)]` reduces to
/// `test_case` rather than to `test`.
pub(super) fn attribute_marker_name<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
    let text = node_text(node, source)?;
    let inner = text
        .trim()
        .trim_start_matches('#')
        .trim_start_matches('@')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let path = inner.split('(').next().unwrap_or(inner).trim();
    path.rsplit(['.', ':', '\\']).next()
}

/// Whether the definition is a name+signature test by
/// [`DefinitionSpec::test_name_prefix`]/[`DefinitionSpec::test_param_type`] —
/// the convention Go (`func TestXxx(t *testing.T)`), Ruby (minitest's `def
/// test_foo`), Fortran (FRUIT's `test_*` subroutine naming), and Python
/// (pytest's and `unittest`'s `def test_foo`) use in place of an attribute.
/// The first three grammars have no attribute/annotation node kind at all;
/// Python has one, but its frameworks do not mark a test with it, so the
/// prefix check runs BESIDE the decorator check. `false` for every grammar
/// that marks tests only through an attribute
/// ([`DefinitionSpec::test_name_prefix`] is `None`).
///
/// The prefix match itself is case-sensitive UNLESS
/// [`DefinitionSpec::test_name_case_insensitive`] is set — Fortran's, whose
/// identifiers are case-insensitive by language semantics, so
/// `TEST_DEEPLY_NESTED`/`test_deeply_nested`/`Test_Deeply_Nested` all name
/// the same subroutine. Go, Ruby, and Python leave it unset: all three are
/// case-sensitive languages where `go test`/minitest/pytest require the
/// exact-case prefix, so a case-insensitive match there would recognize
/// helpers the runner itself would never run (an unexported `testHelper` is
/// not a real `Test` entry point).
fn name_signature_marks_test(node: Node<'_>, source: &str, spec: &DefinitionSpec) -> bool {
    let Some(prefix) = spec.test_name_prefix else {
        return false;
    };
    let header = function_header(node, spec);
    let Some(name) = header
        .child_by_field_name(spec.name_field)
        .and_then(|n| resolve_declarator_name(n, source))
    else {
        return false;
    };
    let prefix_matches = if spec.test_name_case_insensitive {
        name.get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    } else {
        name.starts_with(prefix)
    };
    if !prefix_matches {
        return false;
    }
    let Some(required_type) = spec.test_param_type else {
        return true;
    };
    let Some(params) = header.child_by_field_name(spec.parameters_field) else {
        return false;
    };
    let mut cursor = params.walk();
    let first_param = params.named_children(&mut cursor).next();
    drop(cursor);
    let Some(first_param) = first_param else {
        return false;
    };
    first_param
        .child_by_field_name("type")
        .and_then(|t| node_text(t, source))
        .is_some_and(|text| text.contains(required_type))
}
