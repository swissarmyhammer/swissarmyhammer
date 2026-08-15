//! Cognitive complexity, computed from the tree-sitter parse instead of counted
//! by a language model.
//!
//! The `complexity` review validator used to ask the model to count nesting
//! depth and branches. Models count badly, so the same unchanged file produced a
//! different finding set on every run. This module replaces the counting with a
//! pure function over a parsed tree: the model is handed the numbers and only
//! compares them against a threshold.
//!
//! # The metric
//!
//! [`FunctionComplexity::cognitive_score`] is the published Sonar cognitive
//! complexity of one function:
//!
//! - **+1** for each break in linear flow (`if`, `else`, `else if`, each loop,
//!   each `match`/`switch`, each `catch`, each labelled jump).
//! - **plus the current nesting level** for a construct that also opens a
//!   nesting level. An `if` two levels deep therefore scores 3, not 1.
//! - **+1 per sequence** of like boolean operators in one condition. `a && b &&
//!   c` is one sequence; `a && b || c` is two.
//! - **no increment** for a construct that only shorthands control flow.
//!
//! Two consequences matter for this validator, and both are the point of moving
//! the count into code:
//!
//! 1. A `match`/`switch` scores **+1 for the whole construct**, never once per
//!    arm, and its arms open **no** nesting level. An arm is a branch of one
//!    decision, not a nested decision. A two-arm `Option` match is therefore
//!    score 1 at nesting depth 1 — it cannot be reported as "depth 4".
//! 2. The rule's "simple match/switch with many variants but simple bodies"
//!    exception is now arithmetic rather than prose the model has to remember:
//!    many simple arms simply do not accumulate score.
//!
//! # The gates
//!
//! Two, both explicit constants:
//!
//! - [`COGNITIVE_COMPLEXITY_THRESHOLD`] — the Sonar default, on the score.
//! - [`NESTING_DEPTH_THRESHOLD`] — the depth the validator's rule already
//!   stated ("nested more than 3 levels deep, 4+ is a flag"), kept as its own
//!   gate because the rule already quantified it.
//!
//! The remaining numbers ([`FunctionComplexity::branch_count`],
//! [`FunctionComplexity::max_boolean_operands`],
//! [`FunctionComplexity::max_loop_nesting`],
//! [`FunctionComplexity::max_else_if_chain`]) are **evidence, not gates**. They
//! let a finding say "7 branches" instead of "numerous branches". The rule never
//! set a limit for them, so this module does not invent one.
//!
//! # Language coverage
//!
//! Node kinds are per grammar, so each language needs a [`ComplexitySpec`] row.
//! A language with no row returns [`None`] — **not computed**, never a score of
//! zero. A silent zero would disable the validator on that language, which is
//! worse than the drift this module removes.
//!
//! Every row is built the same way: parse a real sample in the target grammar
//! and read the s-expression it actually produces, then transcribe the node
//! kinds and field names verbatim. Two structural shapes recur across the
//! mapped grammars' `if`/`else if`/`else` chains, and [`walk_conditional`] and
//! [`walk_alternative`] handle both from the same generic fields:
//!
//! - A **single, recursive `alternative` field**. Rust, C, C++, JavaScript and
//!   TypeScript wrap the value in a transparent [`ComplexitySpec::else_wrapper_kinds`]
//!   node (`else_clause`) holding either the next link (another
//!   [`ComplexitySpec::conditional_kinds`] node) or the terminal body. Java and
//!   C# skip the wrapper and put the next link or the terminal body directly in
//!   `alternative`.
//! - A **repeated `alternative` field flattened onto the original conditional**.
//!   Python's `elif_clause` and PHP's `else_if_clause` — both listed in
//!   [`ComplexitySpec::elif_kinds`] — carry their own `condition` and
//!   [`ComplexitySpec::consequence_field`], sitting as siblings of a trailing
//!   terminal `else_clause` rather than nested inside one another. Ruby's
//!   `elsif`/`else` and Go's bare-nested-`if_statement` shape reuse this SAME
//!   mechanism unchanged (Go's `alternative` field holds the next link or
//!   terminal body directly, matching Java's/C#'s no-wrapper case; Ruby's
//!   `elsif` carries its own `condition`/`consequence`/`alternative` fields
//!   exactly like Python's `elif_clause`).
//!
//! Three more grammars needed one further generalization each, verified the
//! same way, and none of them special-case a single language in the shared
//! walker:
//!
//! - **Swift's positional, marker-delimited alternative.** `if_statement`'s
//!   own field list holds only `condition` — no `consequence`, no
//!   `alternative` field at all. The consequence and alternative are
//!   POSITIONAL children instead, delimited by an anonymous-but-NAMED `else`
//!   marker token ([`ComplexitySpec::else_marker_kinds`]), handled by
//!   [`walk_marker_conditional`].
//! - **Fortran's fully positional conditional.** `if_statement` carries NO
//!   fields at all — condition, primary consequence statements, and
//!   `elseif_clause`/`else_clause` siblings are recovered purely by position
//!   and kind ([`ComplexitySpec::positional_conditional`],
//!   [`ComplexitySpec::statement_terminator_kinds`]), handled by
//!   [`walk_positional_conditional`].
//! - **Elixir's call-classified everything.** `def`/`defp`/`defmacro`,
//!   ExUnit's `test`, and even Elixir's OWN `if`/`unless`/`case`/`cond` are
//!   ALL generic `call` nodes, distinguished only by their `target`
//!   identifier's text ([`ComplexitySpec::call_target_kinds`], resolved by
//!   [`effective_kind`] everywhere a node's kind is checked against a spec
//!   list). Its conditional's consequence and alternative both sit inside
//!   ONE `do_block`, the alternative as a trailing child rather than a
//!   separate field ([`ComplexitySpec::alternative_nested_in_consequence`],
//!   handled by [`walk_consequence_with_nested_alternative`]), and its
//!   condition sits in the SAME `arguments` field a `def` uses for its own
//!   name+parameters ([`ComplexitySpec::condition_field`]) — read via
//!   [`child_by_field_or_kind`], since Elixir's `call` node declares only a
//!   `target` FIELD; `arguments`/`do_block` are ordinary positional children
//!   whose KIND happens to be spelled the same as the concept they hold.
//!
//! # Test marking beyond attributes
//!
//! Every grammar mapped by ^xjyb2qf marks a test with an attribute/
//! annotation/decorator node. Three more — Go, Ruby, Fortran — have no such
//! grammar construct at all; their real convention is name+signature based
//! instead (Go's `func TestXxx(t *testing.T)`, Ruby's minitest `def
//! test_foo`, Fortran's FRUIT-style `test_*` naming), checked by
//! [`ComplexitySpec::test_name_prefix`]/[`ComplexitySpec::test_param_type`]
//! via [`name_signature_marks_test`] rather than a per-language attribute
//! branch. Python has the decorator construct, but neither pytest nor
//! `unittest` marks a test with one: both read the `test_` prefix at the
//! definition, so Python carries a name prefix BESIDE its decorator kinds
//! rather than instead of them. Elixir needs neither: its ExUnit `test` block
//! is itself a `call` classified exactly like `def`
//! ([`ComplexitySpec::call_target_test_kinds`]), so being named `test` at the
//! definition IS the marker.
//!
//! The JavaScript family marks a test the same way, one level out: jest and
//! mocha spell a test `it("...", () => { ... })`, a call whose callback holds
//! the body. The callback is the definition — it is where the statements and so
//! the score are — and the enclosing call's callee is its marker, reached by
//! [`defining_call`] off the same
//! [`ComplexitySpec::call_target_test_kinds`] list Elixir uses. A `describe`
//! suite is not on that list, so only the tests themselves are exempt from the
//! gates.
//!
//! The prefix match is case-sensitive by default (Go, Python, and Ruby are all
//! case-sensitive languages, and `go test` itself requires the exact-case
//! `Test` prefix), but Fortran overrides it with
//! [`ComplexitySpec::test_name_case_insensitive`]: Fortran identifiers are
//! case-insensitive by language semantics, so `TEST_DEEPLY_NESTED` and
//! `test_deeply_nested` name the same subroutine and must both be recognized
//! as a FRUIT-style test. Fortran's own `.and.`/`.or.` boolean-operator
//! tokens need no equivalent case handling in [`ComplexitySpec::logical_operators`]:
//! `tree_sitter_fortran`'s grammar aliases each via a `caseInsensitive()`
//! regex to the SAME lowercase node kind regardless of source casing
//! (verified by reading `grammar.js` and by parsing `.AND.`/`.OR.` samples,
//! covered by `fortran_boolean_operators_are_recognized_regardless_of_case`).
//!
//! Bash has no attribute/annotation grammar construct either, and its one
//! real-world convention — bats-core's `# @test "description"` comment — is
//! unstructured free text inside a generic `comment` node, indistinguishable
//! by KIND from an ordinary doc comment or license header (verified by
//! parsing one). Treating any comment as a potential test marker would be
//! unsafe and overbroad, so Bash has no [`ComplexitySpec`] row and reports
//! not-computed like any other unmapped language.

use tree_sitter::Node;

use super::parse_code;

mod test_census;

/// The test census — the per-test measurement the `no-test-cheating` rule
/// reads. It shares this module's grammar rows, so it lives under `complexity`
/// and is re-exported here as the plugin's public entry point.
pub use test_census::{test_census, TestCensus, TestDefect};

/// The cognitive-complexity score at or above which a function is flagged. The
/// Sonar default.
pub const COGNITIVE_COMPLEXITY_THRESHOLD: u32 = 15;

/// The condition-nesting depth at or above which a function is flagged:
/// conditions nested more than 3 levels deep.
pub const NESTING_DEPTH_THRESHOLD: u32 = 4;

/// The deepest tree-sitter tree depth the walk descends before stopping and
/// reporting the function as partial rather than continuing to recurse.
///
/// This probe runs on real diffs, including third-party repository content
/// parsed directly (see `mirdan/src/git_source.rs`). A pathologically deep but
/// finite source file — generated code, a huge chained `if`/`match`, deeply
/// nested literals from a minifier — produces a parse tree deep enough to
/// exhaust the native call stack through `walk`/`walk_children`/
/// `walk_conditional`/`walk_alternative`/`walk_boolean`/`boolean_chain` (and
/// `collect_functions`'s own tree walk), which mirror the tree with one Rust
/// stack frame per level. That is a hard crash, not a graceful error, and it
/// would take down the whole review process rather than fail one probe.
///
/// The deepest pinned fixture is depth 4, and real code rarely exceeds depth
/// 10-20, so this cap is two orders of magnitude above any plausible real
/// function. It never touches legitimate code while stopping every walker
/// far short of exhausting even a small thread stack.
const MAX_TRAVERSAL_DEPTH: u32 = 256;

/// The per-grammar node kinds the scorer interprets.
///
/// Every language is one row of data. The scorer is a single traversal
/// parameterized by the row — there is no per-language branch — so the counted
/// node set for a language is reviewable in one place instead of inferred from
/// code.
struct ComplexitySpec {
    /// The language id, mirroring the `LanguageConfig` id it is keyed to.
    language: &'static str,
    /// Node kinds that define a function scored as its own unit. A function
    /// nested inside another is scored separately, never folded into its
    /// parent.
    function_kinds: &'static [&'static str],
    /// The field name holding a function's name, when the grammar names it.
    /// Resolved through [`resolve_declarator_name`], which unwraps a nested
    /// `declarator` field chain when the grammar needs one (C/C++'s
    /// `function_declarator`/`pointer_declarator`).
    name_field: &'static str,
    /// Kinds that add `1 + nesting` and open a nesting level for their body.
    nesting_kinds: &'static [&'static str],
    /// The subset of [`Self::nesting_kinds`] that are conditionals, walked via
    /// [`walk_conditional`] so an `else if`/`elif` is recognized inside the
    /// chain rather than as a separately-nested condition.
    conditional_kinds: &'static [&'static str],
    /// The field name holding a conditional's primary branch body: Sonar's own
    /// `"consequence"` convention, used by every mapped grammar except PHP's
    /// `"body"`.
    consequence_field: &'static str,
    /// Kinds that continue an else-if chain with their own `condition` and
    /// [`Self::consequence_field`], attached via a REPEATED `alternative`
    /// field on the ORIGINAL conditional rather than through nesting —
    /// Python's `elif_clause`, PHP's `else_if_clause`. Empty for grammars that
    /// nest the next link as another [`Self::conditional_kinds`] node instead.
    elif_kinds: &'static [&'static str],
    /// A transparent wrapper the grammar puts around a conditional's single
    /// `alternative` value, holding either the next chain link or the
    /// terminal body as its one child — Rust/C/C++/JavaScript/TypeScript's
    /// `else_clause`. Empty for grammars that put the next link or the
    /// terminal body directly in `alternative` with no wrapper (Java's, C#'s).
    else_wrapper_kinds: &'static [&'static str],
    /// The subset of [`Self::nesting_kinds`] that are loops, counted separately
    /// for [`FunctionComplexity::max_loop_nesting`].
    loop_kinds: &'static [&'static str],
    /// Arm/case kinds. Transparent: no increment, **no nesting level**. This is
    /// the row that keeps a `match`/`switch` arm from reading as a nested
    /// condition.
    arm_kinds: &'static [&'static str],
    /// Kinds that open a nesting level without incrementing (closures, lambdas).
    nest_only_kinds: &'static [&'static str],
    /// Kinds whose operator token is inspected for boolean sequences.
    logical_kinds: &'static [&'static str],
    /// The operator tokens that form a boolean sequence.
    logical_operators: &'static [&'static str],
    /// Jump kinds that add a flat +1 when they carry a label.
    labelled_jump_kinds: &'static [&'static str],
    /// The node kind of the label on a jump. A jump WITHOUT one of these
    /// children is ordinary control flow — `break 5` returns a value from a
    /// loop and is not a jump to a label.
    label_kinds: &'static [&'static str],
    /// The node kind of an attribute/annotation/decorator that can mark a
    /// function as a test, wherever the grammar attaches it: a preceding
    /// sibling ([`is_test_definition`]'s sibling scan) or a child of the
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
    parameters_field: &'static str,
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

    // ---- conditional shape generalizations (go/ruby reuse the existing
    // field-based mechanism unchanged; swift, fortran, and elixir each need
    // one more shape) ----
    //
    /// The field name holding a conditional's own condition expression.
    /// Every currently-mapped grammar spells this `"condition"`; Elixir's
    /// `if`/`unless` are `call` nodes with no `condition` field at all —
    /// their test expression is the SAME `arguments` field a `def` uses for
    /// its own name+parameter list, verified by parsing `if a do ... end`
    /// and reading the field name on the s-expression.
    condition_field: &'static str,
    /// A marker node with no fields or content, sitting as a direct
    /// POSITIONAL child of the conditional right before its alternative —
    /// Swift's `else` token, which the grammar makes a NAMED node despite
    /// carrying no fields at all (verified: `if_statement`'s own field list
    /// holds only `condition`; `else`/`if_statement`/`statements` are listed
    /// as unnamed-field "children" in `node-types.json`, and parsing a
    /// three-way `else if` chain confirms `else` appears as a bare marker
    /// between the consequence and the next link/terminal body). When set,
    /// [`walk_conditional`] locates the consequence and alternative by
    /// position instead of by field, via [`walk_marker_conditional`]. Empty
    /// for every grammar whose alternative is field-accessible.
    else_marker_kinds: &'static [&'static str],
    /// Whether this grammar's conditional carries NO fields at all for its
    /// condition, consequence, or alternative — Fortran's `if_statement`,
    /// verified via `node-types.json`: `if_statement`, `elseif_clause`, and
    /// `else_clause` each report an EMPTY field list, so the condition is the
    /// first named child, zero or more ordinary statements are the primary
    /// consequence, then zero or more [`Self::elif_kinds`] siblings
    /// (`elseif_clause`), then optionally one terminal body of its own
    /// (`else_clause`, walked as a plain container — no unwrap needed), then
    /// a [`Self::statement_terminator_kinds`] marker (`end_if_statement`).
    /// When set, [`walk_conditional`] dispatches to
    /// [`walk_positional_conditional`] instead of any field-based path.
    /// `false` for every other mapped grammar.
    positional_conditional: bool,
    /// Kinds that terminate a [`Self::positional_conditional`] scan —
    /// Fortran's `end_if_statement`, which sits as a trailing sibling of the
    /// primary/elseif/else content rather than under any field, and would
    /// otherwise be misread as one more terminal alternative. Only
    /// meaningful when [`Self::positional_conditional`] is set.
    statement_terminator_kinds: &'static [&'static str],
    /// Whether a conditional's alternative is nested INSIDE the fetched
    /// [`Self::consequence_field`] container as its own TRAILING child,
    /// rather than being a separate field of the conditional itself —
    /// Elixir's `do_block`: verified by parsing `if a do 1 else 2 end`, whose
    /// `do_block` field holds `1` followed by an `else_block` — both as
    /// children of the SAME `do_block`, with no separate `alternative` field
    /// on the `if` call at all. When set, [`walk_conditional`] walks every
    /// child of the fetched consequence except a trailing
    /// [`Self::else_wrapper_kinds`] child as the primary body, and feeds that
    /// trailing child to [`walk_alternative`] instead, via
    /// [`walk_consequence_with_nested_alternative`]. `false` for every other
    /// mapped grammar.
    alternative_nested_in_consequence: bool,

    // ---- call-target classification (elixir) ----
    //
    // Elixir represents `def`/`defp`/`defmacro`/`defmacrop`, ExUnit's `test`,
    // and even its OWN control-flow special forms (`if`/`unless`/`case`/
    // `cond`/`for`) as one generic `call` node — verified by parsing
    // `defmodule Foo do def pick(a, b) do if a do ... end end end` and
    // reading the labelled tree: every one of `defmodule`/`def`/`if` is a
    // `call` node with `target: (identifier)`, distinguished ONLY by that
    // target's text. This breaks `function_kinds.contains(&node.kind())` —
    // EVERY call in the file has kind `"call"`, definitions and ordinary
    // calls alike.
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
    /// is never misread as a special form. The JS family reads
    /// `member_expression` as well, because jest spells a skipped test
    /// `it.skip(...)`, whose callee is a member expression reading `it.skip`.
    callee_kinds: &'static [&'static str],
    /// The set of callee texts that reclassify a call node's EFFECTIVE kind
    /// (see [`effective_kind`]) to that text, for every [`ComplexitySpec`] list
    /// membership check the walker makes — function detection, nesting,
    /// conditionals, and arms alike. An ordinary call (`Repo.insert(a)`,
    /// `foo()`) is never affected: its callee is either not one of
    /// [`Self::callee_kinds`] or an identifier whose text is not in this list.
    /// Empty for every grammar whose special forms are real dedicated node
    /// kinds, which is every grammar but Elixir's.
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
    /// callback is the definition the scorer and the census both measure. A
    /// callback sits directly in the call's argument list (`arrow_function`
    /// inside `arguments` inside `call_expression`, verified), which is how
    /// [`defining_call`] reaches its call. Empty for Elixir, whose `test "..."
    /// do ... end` IS the definition node.
    test_callback_kinds: &'static [&'static str],
}

/// Default values for the fields ^xjyb2qf added to support Go's, Ruby's, and
/// Fortran's name+signature test marking, Swift's and Fortran's
/// marker/positional conditional shapes, and Elixir's call-target
/// classification. Every language mapped before that work needs none of
/// them, so its spec literal inherits this whole block via struct-update
/// syntax (`..EXTENDED_SPEC_DEFAULTS`) and overrides only the field(s) that
/// actually differ, instead of repeating the same dozen field values in
/// every definition. The leading fields set here are placeholders, never
/// read: every spec below sets its own function/nesting/conditional shape
/// explicitly, so this constant is never used by itself.
const EXTENDED_SPEC_DEFAULTS: ComplexitySpec = ComplexitySpec {
    language: "",
    function_kinds: &[],
    name_field: "",
    nesting_kinds: &[],
    conditional_kinds: &[],
    consequence_field: "",
    elif_kinds: &[],
    else_wrapper_kinds: &[],
    loop_kinds: &[],
    arm_kinds: &[],
    nest_only_kinds: &[],
    logical_kinds: &[],
    logical_operators: &[],
    labelled_jump_kinds: &[],
    label_kinds: &[],
    attribute_kinds: &[],
    attribute_container_kinds: &[],
    test_name_prefix: None,
    test_param_type: None,
    parameters_field: "",
    test_name_case_insensitive: false,
    header_child_kinds: &[],
    condition_field: "condition",
    else_marker_kinds: &[],
    positional_conditional: false,
    statement_terminator_kinds: &[],
    alternative_nested_in_consequence: false,
    call_kind: "",
    callee_field: "",
    callee_kinds: &[],
    call_target_kinds: &[],
    call_target_test_kinds: &[],
    test_callback_kinds: &[],
};

/// Rust. Verified against `tree_sitter_rust` by parsing samples covering every
/// listed kind — the node names below are the grammar's, not guesses.
static RUST_SPEC: ComplexitySpec = ComplexitySpec {
    language: "rust",
    function_kinds: &["function_item"],
    name_field: "name",
    nesting_kinds: &[
        "if_expression",
        "for_expression",
        "while_expression",
        "loop_expression",
        "match_expression",
    ],
    conditional_kinds: &["if_expression"],
    consequence_field: "consequence",
    elif_kinds: &[],
    else_wrapper_kinds: &["else_clause"],
    loop_kinds: &["for_expression", "while_expression", "loop_expression"],
    arm_kinds: &["match_arm"],
    nest_only_kinds: &["closure_expression"],
    logical_kinds: &["binary_expression"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &["break_expression", "continue_expression"],
    label_kinds: &["label"],
    attribute_kinds: &["attribute_item"],
    attribute_container_kinds: &[],
    ..EXTENDED_SPEC_DEFAULTS
};

/// Shared field values for TypeScript, TSX, and JavaScript. All three
/// grammars are C-like and produce identical node kinds for every field
/// except the language id itself, confirmed by parsing the same
/// control-flow, jest, and decorator samples under each grammar.
///
/// A jest/mocha test is a call — `it("...", () => { ... })` — rather than a
/// declaration, and the call is not the function: the grammar hangs the body on
/// the `arrow_function`/`function_expression` the call takes as its second
/// argument (verified). So the callback is the definition, marked a test
/// through the enclosing call's callee
/// ([`ComplexitySpec::call_target_test_kinds`] reached by [`defining_call`]) and
/// named by the call's description string. `describe` is deliberately absent
/// from that list: a suite asserts nothing itself, and marking its callback a
/// test would exempt real code from the gates.
const fn typescript_family_spec(language: &'static str) -> ComplexitySpec {
    ComplexitySpec {
        language,
        function_kinds: &[
            "function_declaration",
            "method_definition",
            "arrow_function",
            "function_expression",
        ],
        name_field: "name",
        nesting_kinds: &[
            "if_statement",
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
        ],
        conditional_kinds: &["if_statement"],
        consequence_field: "consequence",
        elif_kinds: &[],
        else_wrapper_kinds: &["else_clause"],
        loop_kinds: &[
            "for_statement",
            "for_in_statement",
            "while_statement",
            "do_statement",
        ],
        arm_kinds: &["switch_case", "switch_default"],
        nest_only_kinds: &[],
        logical_kinds: &["binary_expression"],
        logical_operators: &["&&", "||"],
        labelled_jump_kinds: &["continue_statement", "break_statement"],
        label_kinds: &["statement_identifier"],
        attribute_kinds: &["decorator"],
        attribute_container_kinds: &[],
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
        ..EXTENDED_SPEC_DEFAULTS
    }
}

/// TypeScript. Verified against `tree_sitter_typescript` (the `LANGUAGE_TYPESCRIPT`
/// grammar). Its decorator is a sibling of the `method_definition` it marks
/// inside `class_body` — unlike JavaScript's, which nests it as a field of the
/// method itself — confirmed by parsing a two-method class with only one
/// decorated.
static TYPESCRIPT_SPEC: ComplexitySpec = typescript_family_spec("typescript");

/// TSX. Verified against `tree_sitter_typescript` (the `LANGUAGE_TSX` grammar)
/// by parsing the same control-flow and decorator samples used for TypeScript
/// — the node kinds are identical; only the JSX-extended grammar differs, and
/// none of the samples used JSX syntax.
static TSX_SPEC: ComplexitySpec = typescript_family_spec("tsx");

/// JavaScript. Verified against `tree_sitter_javascript`. Its decorator is a
/// `decorator:` field of the `method_definition` itself — unlike TypeScript's
/// sibling placement — confirmed by parsing a decorated class method and
/// reading the field name on the s-expression.
static JAVASCRIPT_SPEC: ComplexitySpec = typescript_family_spec("javascript");

/// Python. Verified against `tree_sitter_python`. Its `if_statement` flattens
/// every `elif_clause`/`else_clause` onto ONE repeated `alternative` field
/// (confirmed with a three-way `elif` chain — none of them nest inside one
/// another), and its boolean operator tokens are the literal keywords `and`/
/// `or` rather than `&&`/`||` (confirmed by inspecting the operator node's own
/// `kind()`).
///
/// Both pytest and `unittest` mark a test by the `test_` prefix at the
/// definition rather than by a decorator, so the spec carries
/// [`ComplexitySpec::test_name_prefix`] `"test_"` as well — a plain name
/// prefix with no signature check, exactly as Ruby's minitest convention does.
/// The `@...test` decorator branch stays: a `decorator` is a preceding named
/// sibling inside the `decorated_definition` wrapper (confirmed by parsing
/// `@pytest.mark.skip("why")` above a `def` and reading the s-expression).
static PYTHON_SPEC: ComplexitySpec = ComplexitySpec {
    language: "python",
    function_kinds: &["function_definition"],
    name_field: "name",
    nesting_kinds: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "match_statement",
    ],
    conditional_kinds: &["if_statement"],
    consequence_field: "consequence",
    elif_kinds: &["elif_clause"],
    else_wrapper_kinds: &[],
    loop_kinds: &["for_statement", "while_statement"],
    arm_kinds: &["case_clause"],
    nest_only_kinds: &[],
    logical_kinds: &["boolean_operator"],
    logical_operators: &["and", "or"],
    labelled_jump_kinds: &[],
    label_kinds: &[],
    attribute_kinds: &["decorator"],
    attribute_container_kinds: &[],
    test_name_prefix: Some("test_"),
    ..EXTENDED_SPEC_DEFAULTS
};

/// Java. Verified against `tree_sitter_java`. Its `if_statement` puts the next
/// `else if` link or the terminal `else` body directly in `alternative` with
/// NO wrapping node at all (confirmed with a three-way `else if` chain: each
/// link is a bare nested `if_statement`, and the final `else` is a bare
/// `block`) — unlike Rust/C/C++/JavaScript/TypeScript's `else_clause` wrapper.
/// Its annotation sits inside the method's own `modifiers` child rather than
/// as a preceding sibling (confirmed by parsing `@Test`/`@Test(timeout = 100)`
/// and reading the exact byte span of each node).
static JAVA_SPEC: ComplexitySpec = ComplexitySpec {
    language: "java",
    function_kinds: &["method_declaration"],
    name_field: "name",
    nesting_kinds: &[
        "if_statement",
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
        "switch_expression",
    ],
    conditional_kinds: &["if_statement"],
    consequence_field: "consequence",
    elif_kinds: &[],
    else_wrapper_kinds: &[],
    loop_kinds: &[
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
    ],
    arm_kinds: &["switch_block_statement_group"],
    nest_only_kinds: &[],
    logical_kinds: &["binary_expression"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &["continue_statement", "break_statement"],
    label_kinds: &["identifier"],
    attribute_kinds: &["marker_annotation", "annotation"],
    attribute_container_kinds: &["modifiers"],
    ..EXTENDED_SPEC_DEFAULTS
};

/// Shared field values for C and C++. Their control-flow, loop, and
/// attribute node kinds are identical — confirmed by parsing the same
/// samples under each grammar — and [`walk_conditional`] fetches the
/// `condition` field generically, so C++'s `condition_clause` wrapper vs.
/// C's `parenthesized_expression` never needs to be modeled here.
const fn c_family_spec(language: &'static str) -> ComplexitySpec {
    ComplexitySpec {
        language,
        function_kinds: &["function_definition"],
        name_field: "declarator",
        nesting_kinds: &[
            "if_statement",
            "for_statement",
            "while_statement",
            "do_statement",
            "switch_statement",
        ],
        conditional_kinds: &["if_statement"],
        consequence_field: "consequence",
        elif_kinds: &[],
        else_wrapper_kinds: &["else_clause"],
        loop_kinds: &["for_statement", "while_statement", "do_statement"],
        arm_kinds: &["case_statement"],
        nest_only_kinds: &[],
        logical_kinds: &["binary_expression"],
        logical_operators: &["&&", "||"],
        labelled_jump_kinds: &["goto_statement"],
        label_kinds: &["statement_identifier"],
        attribute_kinds: &["attribute"],
        attribute_container_kinds: &["attribute_declaration"],
        ..EXTENDED_SPEC_DEFAULTS
    }
}

/// C. Verified against `tree_sitter_c`. A function's name sits several
/// `declarator` fields deep (`function_definition` names its
/// `function_declarator`, which names the plain identifier — one more
/// `pointer_declarator` level for a pointer-returning function), resolved
/// generically by [`resolve_declarator_name`] rather than a C-specific
/// special case. C has no labelled `break`/`continue`; its only labelled jump
/// is `goto`, confirmed by parsing a `goto` past two nested `for` loops.
static C_SPEC: ComplexitySpec = c_family_spec("c");

/// C++. Verified against `tree_sitter_cpp`. Structurally identical to C's
/// control flow (its `if_statement`'s `condition` wraps the value in a
/// `condition_clause` rather than C's `parenthesized_expression`, which is
/// irrelevant here since [`walk_conditional`] fetches the `condition` field
/// generically rather than matching its inner kind). Its attribute uses the
/// C++11 `[[...]]` syntax (`attribute_declaration` wrapping `attribute`,
/// confirmed by parsing `[[nodiscard]]`), the same shape C's does.
static CPP_SPEC: ComplexitySpec = c_family_spec("cpp");

/// C#. Verified against `tree_sitter_c_sharp`. Its `if_statement` matches
/// Java's shape exactly: `alternative` holds the next link or the terminal
/// body directly, no wrapper (confirmed with a three-way `else if` chain). C#
/// has no labelled `break`/`continue`; its only labelled jump is `goto`
/// (confirmed by parsing a `goto` past two nested `foreach` loops). Its
/// attribute sits inside the method's own `attribute_list` child (confirmed
/// by parsing `[Test]`/`[Fact]` and reading each node's exact byte span).
static CSHARP_SPEC: ComplexitySpec = ComplexitySpec {
    language: "csharp",
    function_kinds: &["method_declaration"],
    name_field: "name",
    nesting_kinds: &[
        "if_statement",
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
        "switch_statement",
    ],
    conditional_kinds: &["if_statement"],
    consequence_field: "consequence",
    elif_kinds: &[],
    else_wrapper_kinds: &[],
    loop_kinds: &[
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
    ],
    arm_kinds: &["switch_section"],
    nest_only_kinds: &[],
    logical_kinds: &["binary_expression"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &["goto_statement"],
    label_kinds: &["identifier"],
    attribute_kinds: &["attribute"],
    attribute_container_kinds: &["attribute_list"],
    ..EXTENDED_SPEC_DEFAULTS
};

/// PHP. Verified against `tree_sitter_php` (the `LANGUAGE_PHP` grammar). Its
/// `if_statement` flattens `else_if_clause`/`else_clause` onto one repeated
/// `alternative` field exactly like Python's (confirmed with a three-way
/// `elseif` chain), and it names the primary branch body `body` rather than
/// `consequence`. Its attribute is nested two container levels deep
/// (`attributes: (attribute_list (attribute_group (attribute ...)))`,
/// confirmed on both a free function and a class method) — PHPUnit's real
/// `#[Test]` attribute marker.
static PHP_SPEC: ComplexitySpec = ComplexitySpec {
    language: "php",
    function_kinds: &["function_definition", "method_declaration"],
    name_field: "name",
    nesting_kinds: &[
        "if_statement",
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
        "switch_statement",
    ],
    conditional_kinds: &["if_statement"],
    consequence_field: "body",
    elif_kinds: &["else_if_clause"],
    else_wrapper_kinds: &[],
    loop_kinds: &[
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
    ],
    arm_kinds: &["case_statement", "default_statement"],
    nest_only_kinds: &[],
    logical_kinds: &["binary_expression"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &[],
    label_kinds: &[],
    attribute_kinds: &["attribute"],
    attribute_container_kinds: &["attribute_list", "attribute_group"],
    ..EXTENDED_SPEC_DEFAULTS
};

/// Go. Verified against `tree_sitter_go`. Its `if_statement` matches Java's/
/// C#'s shape: `alternative` holds the next link or the terminal body
/// directly, no wrapper (confirmed with a three-way `else if` chain). Go has
/// no attribute/annotation node kind at all (confirmed while mapping every
/// other node kind below — no such kind appeared), so its real test
/// convention — `func TestXxx(t *testing.T)` — is name+signature based
/// instead: [`ComplexitySpec::test_name_prefix`] `"Test"` plus
/// [`ComplexitySpec::test_param_type`] `"testing.T"` on the first parameter's
/// own `type` field (confirmed by parsing `func TestAdd(t *testing.T) {...}`
/// and reading the parameter's `type: (pointer_type (qualified_type package:
/// (package_identifier) name: (type_identifier)))`), so an ordinary
/// `TestXxx` HELPER with no such parameter is never mistaken for a real `go
/// test` entry point. Its labelled `break`/`continue` names the label via a
/// `label_name` child (confirmed by parsing a labelled `continue` past two
/// nested `for` loops) — unlike Rust's/Java's/TypeScript's `label`/
/// `statement_identifier`.
static GO_SPEC: ComplexitySpec = ComplexitySpec {
    language: "go",
    function_kinds: &["function_declaration", "method_declaration"],
    name_field: "name",
    nesting_kinds: &[
        "if_statement",
        "for_statement",
        "expression_switch_statement",
    ],
    conditional_kinds: &["if_statement"],
    consequence_field: "consequence",
    elif_kinds: &[],
    else_wrapper_kinds: &[],
    loop_kinds: &["for_statement"],
    arm_kinds: &["expression_case", "default_case"],
    nest_only_kinds: &["func_literal"],
    logical_kinds: &["binary_expression"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &["break_statement", "continue_statement"],
    label_kinds: &["label_name"],
    attribute_kinds: &[],
    attribute_container_kinds: &[],
    test_name_prefix: Some("Test"),
    test_param_type: Some("testing.T"),
    parameters_field: "parameters",
    ..EXTENDED_SPEC_DEFAULTS
};

/// Ruby. Verified against `tree_sitter_ruby`. Its `if` matches Java's/C#'s
/// shape too, but through an intermediate chain KIND rather than a bare
/// nested `if`: `alternative` holds an `elsif` node (itself carrying its own
/// `condition`/`consequence`/`alternative` fields, confirmed with a
/// three-way `elsif` chain), or a terminal `else` directly. Ruby has no
/// attribute/annotation node kind either, so its real test convention —
/// minitest's `def test_foo` — is name-prefix based, with no signature check
/// needed ([`ComplexitySpec::test_param_type`] `None`, confirmed: minitest
/// test methods take no fixed parameter). Its boolean/comparison operator
/// node kind is `binary` (not `binary_expression`), confirmed by parsing
/// `a && b && c` and reading each nested node's own kind.
static RUBY_SPEC: ComplexitySpec = ComplexitySpec {
    language: "ruby",
    function_kinds: &["method", "singleton_method"],
    name_field: "name",
    nesting_kinds: &["if", "for", "while", "case"],
    conditional_kinds: &["if"],
    consequence_field: "consequence",
    elif_kinds: &["elsif"],
    else_wrapper_kinds: &[],
    loop_kinds: &["for", "while"],
    arm_kinds: &["when", "else"],
    nest_only_kinds: &[],
    logical_kinds: &["binary"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &[],
    label_kinds: &[],
    attribute_kinds: &[],
    attribute_container_kinds: &[],
    test_name_prefix: Some("test_"),
    ..EXTENDED_SPEC_DEFAULTS
};

/// Fortran. Verified against `tree_sitter_fortran`. Its `subroutine`/
/// `function` wrap a `subroutine_statement`/`function_statement` child that
/// owns the real `name`/`parameters` fields (confirmed via `node-types.json`:
/// the wrapping node's own field list is empty, resolved generically by
/// [`function_header`]). Its `if_statement` carries NO fields at all —
/// condition, consequence, and the `elseif_clause`/`else_clause` chain are
/// all POSITIONAL siblings (confirmed via `node-types.json` and by parsing a
/// three-way `else if` chain), handled by [`walk_positional_conditional`]
/// rather than any field-based path. Fortran has no attribute/annotation
/// node kind either; its real test convention — FRUIT's `test_*` subroutine
/// naming — is name-prefix based like Ruby's, with no signature check
/// needed. Its boolean operator node kind is `logical_expression` with the
/// literal `.and.`/`.or.` tokens (confirmed by parsing `a .and. b .and. c`).
static FORTRAN_SPEC: ComplexitySpec = ComplexitySpec {
    language: "fortran",
    function_kinds: &["subroutine", "function"],
    name_field: "name",
    nesting_kinds: &["if_statement", "do_loop", "select_case_statement"],
    conditional_kinds: &["if_statement"],
    consequence_field: "",
    elif_kinds: &["elseif_clause"],
    else_wrapper_kinds: &[],
    loop_kinds: &["do_loop"],
    arm_kinds: &["case_statement"],
    nest_only_kinds: &[],
    logical_kinds: &["logical_expression"],
    logical_operators: &[".and.", ".or."],
    labelled_jump_kinds: &[],
    label_kinds: &[],
    attribute_kinds: &[],
    attribute_container_kinds: &[],
    test_name_prefix: Some("test_"),
    test_name_case_insensitive: true,
    header_child_kinds: &["subroutine_statement", "function_statement"],
    positional_conditional: true,
    statement_terminator_kinds: &["end_if_statement"],
    ..EXTENDED_SPEC_DEFAULTS
};

/// Swift. Verified against `tree_sitter_swift`. It DOES have a genuine,
/// current test-marking mechanism — `@Test` parses as `modifiers >
/// attribute`, matching the real Swift Testing framework (confirmed by
/// parsing `@Test\nfunc deeplyNested(...)`). Its `if_statement`'s own field
/// list holds only `condition` (confirmed via `node-types.json`); the
/// consequence and alternative are POSITIONAL children with no field at
/// all — a `statements` consequence, then an `else` MARKER node (itself
/// named despite carrying no fields), then either the next `if_statement`
/// link or a terminal `statements` body (confirmed by parsing a three-way
/// `else if` chain and reading the labelled tree) — handled by
/// [`walk_marker_conditional`]. Its boolean operators are dedicated node
/// kinds (`conjunction_expression` for `&&`, right-recursive rather than
/// left-, confirmed by parsing `a && b && c`), not one generic
/// `binary_expression`, but [`logical_operator`]'s token scan finds the
/// literal `&&`/`||` child either way.
static SWIFT_SPEC: ComplexitySpec = ComplexitySpec {
    language: "swift",
    function_kinds: &["function_declaration"],
    name_field: "name",
    nesting_kinds: &[
        "if_statement",
        "for_statement",
        "while_statement",
        "switch_statement",
    ],
    conditional_kinds: &["if_statement"],
    consequence_field: "",
    elif_kinds: &[],
    else_wrapper_kinds: &[],
    loop_kinds: &["for_statement", "while_statement"],
    arm_kinds: &["switch_entry"],
    nest_only_kinds: &[],
    logical_kinds: &["conjunction_expression", "disjunction_expression"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &[],
    label_kinds: &[],
    attribute_kinds: &["attribute"],
    attribute_container_kinds: &["modifiers"],
    else_marker_kinds: &["else"],
    ..EXTENDED_SPEC_DEFAULTS
};

/// Elixir. Verified against `tree_sitter_elixir`. `def`/`defp`/`defmacro`/
/// `defmacrop`, ExUnit's `test`, and Elixir's OWN control-flow special forms
/// (`if`/`unless`/`case`/`cond`) are ALL generic `call` nodes with `target:
/// (identifier)`, distinguished only by that target's text (confirmed by
/// parsing `defmodule Foo do def pick(a, b) do if a do ... end end end` and
/// reading the labelled tree) — reclassified to that text by
/// [`effective_kind`] via [`ComplexitySpec::call_target_kinds`] rather than
/// any per-language special case. `def`'s/`test`'s own name is not a field of
/// the `call` at all; it is read from `arguments` by [`call_function_name`]
/// instead, which [`function_name`] reaches through [`defining_call`] — the
/// `call` IS the definition here, so it is its own defining call. `if`/`unless`
/// carry their condition in the SAME `arguments` field a `def` uses for its
/// name+parameters — [`ComplexitySpec::condition_field`] `"arguments"` — and
/// their consequence+alternative both sit inside ONE `do_block` field, the
/// alternative as its own TRAILING `else_block` child rather than a separate
/// field (confirmed by parsing `if a do 1 else 2 end`), handled by
/// [`walk_consequence_with_nested_alternative`]
/// ([`ComplexitySpec::alternative_nested_in_consequence`]). `case`/`cond`'s
/// arms are `stab_clause` nodes inside their own `do_block` (confirmed by
/// parsing a three-arm `case`). ExUnit's `test "description" do ... end` is
/// itself a `call` with target `"test"` — no attribute lookup needed, being
/// named `test` at the definition IS the marker
/// ([`ComplexitySpec::call_target_test_kinds`]). Elixir has no imperative
/// loop construct at all — `for` is a functional comprehension (itself a
/// `call`, confirmed) and recursion is idiomatic instead — so
/// [`ComplexitySpec::loop_kinds`] is intentionally empty; its 6-test suite's
/// "nesting deepens" case uses nested conditionals rather than loops.
/// `and`/`or` are real `binary_operator` nodes (confirmed by parsing `a and b
/// and c`), never calls, so they need no reclassification.
static ELIXIR_SPEC: ComplexitySpec = ComplexitySpec {
    language: "elixir",
    function_kinds: &["def", "defp", "defmacro", "defmacrop", "test"],
    name_field: "",
    nesting_kinds: &["if", "unless", "case", "cond"],
    conditional_kinds: &["if", "unless"],
    consequence_field: "do_block",
    elif_kinds: &[],
    else_wrapper_kinds: &["else_block"],
    loop_kinds: &[],
    arm_kinds: &["stab_clause"],
    nest_only_kinds: &["anonymous_function"],
    logical_kinds: &["binary_operator"],
    logical_operators: &["and", "or"],
    labelled_jump_kinds: &[],
    label_kinds: &[],
    attribute_kinds: &[],
    attribute_container_kinds: &[],
    condition_field: "arguments",
    alternative_nested_in_consequence: true,
    call_kind: "call",
    callee_field: "target",
    callee_kinds: &["identifier"],
    call_target_kinds: &[
        "def",
        "defp",
        "defmacro",
        "defmacrop",
        "test",
        "if",
        "unless",
        "case",
        "cond",
    ],
    call_target_test_kinds: &["test"],
    ..EXTENDED_SPEC_DEFAULTS
};

/// Every language with a scorer mapping. A language absent here is "not
/// computed", never zero.
static ALL_SPECS: &[&ComplexitySpec] = &[
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
/// Reads [`ALL_SPECS`], a slice of REFERENCES to [`ComplexitySpec`], which is
/// why the body ends in `.copied()`. Three same-named siblings read three other
/// tables of three other types, two of them slices of values. Sharing one body
/// costs a trait and four impls to save four lines — see the
/// `parser::plugins::code` module doc.
fn spec_for_language(language: &str) -> Option<&'static ComplexitySpec> {
    ALL_SPECS.iter().find(|s| s.language == language).copied()
}

/// One function's computed complexity.
///
/// Every field is a number the review prompt hands the model, so the rule text
/// is a comparison rather than a count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionComplexity {
    /// The function's name, or `<anonymous>` when the grammar gives none.
    pub name: String,
    /// The 1-based line the function starts on.
    pub start_line: usize,
    /// The 1-based line the function ends on.
    pub end_line: usize,
    /// The Sonar cognitive-complexity score. Gated by
    /// [`COGNITIVE_COMPLEXITY_THRESHOLD`].
    pub cognitive_score: u32,
    /// The deepest nesting of counted conditions, 1-based: a construct with no
    /// enclosing counted construct is depth 1. Gated by
    /// [`NESTING_DEPTH_THRESHOLD`]. Match/switch arms are not levels.
    pub max_nesting_depth: u32,
    /// Decision points: each `if`, each `else if`, each `else`, and each
    /// match/switch arm. Evidence, not a gate.
    pub branch_count: u32,
    /// The most operands joined by `&&`/`||` inside a single condition.
    /// Evidence, not a gate.
    pub max_boolean_operands: u32,
    /// The deepest nesting of loops within the function. Evidence, not a gate.
    pub max_loop_nesting: u32,
    /// The longest `else if` chain, counted in `else if` links. Evidence, not a
    /// gate.
    pub max_else_if_chain: u32,
    /// Whether the definition carries a test attribute or framework test naming.
    /// Read at the definition, never from the file name — a complex helper in a
    /// test file is still a complex function.
    pub is_test: bool,
    /// True when the walk hit [`MAX_TRAVERSAL_DEPTH`] before finishing this
    /// function. The numbers above are real for everything the walk reached,
    /// but nothing past the cut was walked, so they are a lower bound — never
    /// treat a partial result as "verified simple".
    pub is_partial: bool,
}

impl FunctionComplexity {
    /// Whether this function trips either gate.
    ///
    /// A test function never trips one: the validator's rule exempts tests
    /// whose complexity is sequential assertions, and that exemption is now
    /// computed from the definition rather than recalled by the model.
    ///
    /// A partial function always trips this, test or not: [`Self::is_partial`]
    /// means the numbers are a lower bound, so reporting it as "under the
    /// gates" would be exactly the silent wrong number this module exists to
    /// avoid.
    pub fn exceeds_gates(&self) -> bool {
        self.is_partial
            || (!self.is_test
                && (self.cognitive_score >= COGNITIVE_COMPLEXITY_THRESHOLD
                    || self.max_nesting_depth >= NESTING_DEPTH_THRESHOLD))
    }
}

/// Every scored function in one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileComplexity {
    /// The language id the scorer used.
    pub language: &'static str,
    /// One entry per function, in source order.
    pub functions: Vec<FunctionComplexity>,
}

/// Compute the cognitive complexity of every function in `source`.
///
/// `path` selects the grammar by extension. Returns `None` — meaning **not
/// computed** — when the path has no language mapping, when the language has no
/// [`ComplexitySpec`], or when the source does not parse. A caller must report
/// "not computed" for `None` and never substitute a zero score, which would
/// silently disable the validator for that file.
pub fn cognitive_complexity(path: &str, source: &str) -> Option<FileComplexity> {
    // The parse comes from the shared roster entry point, never from a second
    // parser built here: one grammar table, one parser cache, one place a new
    // language is added.
    let parsed = parse_code(path, source)?;
    let spec = spec_for_language(parsed.language())?;

    let mut functions = Vec::new();
    collect_functions(parsed.tree().root_node(), source, spec, 0, &mut functions);
    Some(FileComplexity {
        language: spec.language,
        functions,
    })
}

/// Walk the whole tree and score every function node, in source order.
///
/// A function nested in another still gets its own entry; the outer function's
/// walk skips it, so no construct is counted twice.
fn collect_functions(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    depth: u32,
    out: &mut Vec<FunctionComplexity>,
) {
    for_each_function(node, source, spec, depth, &mut |function| {
        out.push(score_function(function, source, spec));
    });
}

/// Visit every function definition at or under `node`, in source order.
///
/// The one place "what counts as a function definition here" is decided, so the
/// scorer and the [`test_census`] read the same set of definitions off the same
/// [`ComplexitySpec`] row rather than each walking the tree its own way.
fn for_each_function<'t>(
    node: Node<'t>,
    source: &str,
    spec: &ComplexitySpec,
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

/// The node's classification for every [`ComplexitySpec`] list membership
/// check: its own grammar KIND, unless it is a `call` node with a bare
/// identifier `target` naming one of [`ComplexitySpec::call_target_kinds`] —
/// Elixir, whose `def`/`defp`/`defmacro`/`test`, and even its OWN
/// control-flow special forms (`if`/`unless`/`case`/`cond`/`for`), are ALL
/// generic `call` nodes. Verified by parsing `defmodule Foo do def pick(a, b)
/// do if a do ... end end end` and reading the labelled tree: `defmodule`,
/// `def`, and `if` are each a `call` node with `target: (identifier)`,
/// distinguished only by that target's text — and an ordinary call
/// (`Repo.insert(a)`) has a `target` of kind `dot`, not `identifier`, so it
/// is never misclassified. A no-op for every other mapped grammar, whose
/// [`ComplexitySpec::call_target_kinds`] is empty.
fn effective_kind<'s>(node: Node<'_>, spec: &ComplexitySpec, source: &'s str) -> &'s str {
    if spec.call_target_kinds.is_empty() {
        return node.kind();
    }
    match call_target_text(node, spec, source) {
        Some(text) if spec.call_target_kinds.contains(&text) => text,
        _ => node.kind(),
    }
}

/// The callee's text of a call node, or `None` when `node` is not this
/// grammar's [`ComplexitySpec::call_kind`], or its callee is not one of the
/// [`ComplexitySpec::callee_kinds`] the grammar's row reads.
///
/// The field holding the callee is the grammar's own
/// ([`ComplexitySpec::callee_field`]: Elixir's `target`, the JS family's
/// `function`), so one lookup serves every call-based grammar.
fn call_target_text<'s>(node: Node<'_>, spec: &ComplexitySpec, source: &'s str) -> Option<&'s str> {
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
/// Two shapes, both read off the same [`ComplexitySpec`] row. The definition IS
/// the call — Elixir's `def`/`test`, whose whole classification comes from the
/// callee. Or the definition is the callback the call takes as an argument —
/// jest/mocha's `it("...", () => { ... })`, where
/// [`ComplexitySpec::test_callback_kinds`] names the callback kinds and the
/// callback sits directly in the call's argument list, putting the call exactly
/// two levels up (`arrow_function` inside `arguments` inside `call_expression`,
/// verified against the JS grammar).
///
/// A call whose callee names no definition is never one, so an ordinary
/// callback keeps its own identity: `arr.map((value) => value + 1)` reports
/// `None`, and its callback stays an anonymous function rather than becoming a
/// test.
fn defining_call<'t, 's>(
    node: Node<'t>,
    spec: &ComplexitySpec,
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

/// The running totals one function's traversal accumulates.
#[derive(Default)]
struct Tally {
    cognitive_score: u32,
    max_nesting_depth: u32,
    branch_count: u32,
    max_boolean_operands: u32,
    max_loop_nesting: u32,
    max_else_if_chain: u32,
    is_partial: bool,
}

impl Tally {
    /// Record a construct that increments by `1 + nesting` and sits at
    /// `nesting + 1` levels deep.
    fn nesting_increment(&mut self, nesting: u32) {
        self.cognitive_score += 1 + nesting;
        self.max_nesting_depth = self.max_nesting_depth.max(nesting + 1);
    }

    /// Record a construct that increments by 1 and opens no nesting level.
    fn flat_increment(&mut self) {
        self.cognitive_score += 1;
    }

    /// Record that a walker stopped at [`MAX_TRAVERSAL_DEPTH`] rather than
    /// finishing. Everything counted up to this point is real; nothing past
    /// the cut was walked, so the totals become a lower bound.
    fn depth_capped(&mut self) {
        self.is_partial = true;
    }
}

/// Score one function node.
fn score_function(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> FunctionComplexity {
    let mut tally = Tally::default();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, source, spec, 0, 0, 1, &mut tally);
    }

    FunctionComplexity {
        name: function_name(node, source, spec),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        cognitive_score: tally.cognitive_score,
        max_nesting_depth: tally.max_nesting_depth,
        branch_count: tally.branch_count,
        max_boolean_operands: tally.max_boolean_operands,
        max_loop_nesting: tally.max_loop_nesting,
        max_else_if_chain: tally.max_else_if_chain,
        is_test: is_test_definition(node, source, spec),
        is_partial: tally.is_partial,
    }
}

/// The function's declared name, or `<anonymous>`.
///
/// A definition a call makes is named by that call's arguments — Elixir's `def
/// pick(a, b)` and jest's `it("adds up", ...)` alike — because neither node
/// carries a `name` field of its own. Every other definition is named by
/// [`ComplexitySpec::name_field`].
fn function_name(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> String {
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

/// The node that owns a function's own `name`/[`ComplexitySpec::parameters_field`]
/// fields — `node` itself, unless [`ComplexitySpec::header_child_kinds`] names
/// a child that owns them instead (Fortran's `subroutine`/`function`, which
/// wrap a `subroutine_statement`/`function_statement` child that is the real
/// field owner — verified via `node-types.json`: the wrapping node's own
/// field list is empty).
fn function_header<'t>(node: Node<'t>, spec: &ComplexitySpec) -> Node<'t> {
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
/// empty string would read as "this is not a test" and would score a test
/// function as complex code. The `duplication` copy answers `""` on purpose,
/// because a chunk must still hash. The four contracts of this name are
/// recorded in the `parser::plugins::code` module doc.
fn node_text<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
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
/// own `kind()` reads `"arguments"`/`"do_block"`), so
/// [`ComplexitySpec::condition_field`] and
/// [`ComplexitySpec::consequence_field`] read them by kind through this same
/// lookup instead of needing an Elixir-specific field/kind flag. A no-op
/// fallback for every other mapped grammar, whose fields genuinely exist and
/// so are always found on the first branch.
fn child_by_field_or_kind<'t>(node: Node<'t>, name: &str) -> Option<Node<'t>> {
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
fn is_test_definition(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> bool {
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
/// [`ComplexitySpec::attribute_container_kinds`] wrapper levels the grammar
/// nests (PHP two deep, `attribute_list` > `attribute_group` > `attribute`;
/// Java/C#/C/C++ one).
///
/// Collecting the nodes rather than answering one question about them is what
/// lets the test marker and the [`test_census`]'s skip markers be read off the
/// same traversal.
fn definition_attributes<'t>(node: Node<'t>, spec: &ComplexitySpec) -> Vec<Node<'t>> {
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
fn collect_attributes<'t>(node: Node<'t>, spec: &ComplexitySpec, out: &mut Vec<Node<'t>>) {
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
fn attribute_marker_name<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
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
/// [`ComplexitySpec::test_name_prefix`]/[`ComplexitySpec::test_param_type`] —
/// the convention Go (`func TestXxx(t *testing.T)`), Ruby (minitest's `def
/// test_foo`), Fortran (FRUIT's `test_*` subroutine naming), and Python
/// (pytest's and `unittest`'s `def test_foo`) use in place of an attribute.
/// The first three grammars have no attribute/annotation node kind at all;
/// Python has one, but its frameworks do not mark a test with it, so the
/// prefix check runs BESIDE the decorator check. `false` for every grammar
/// that marks tests only through an attribute
/// ([`ComplexitySpec::test_name_prefix`] is `None`).
///
/// The prefix match itself is case-sensitive UNLESS
/// [`ComplexitySpec::test_name_case_insensitive`] is set — Fortran's, whose
/// identifiers are case-insensitive by language semantics, so
/// `TEST_DEEPLY_NESTED`/`test_deeply_nested`/`Test_Deeply_Nested` all name
/// the same subroutine. Go, Ruby, and Python leave it unset: all three are
/// case-sensitive languages where `go test`/minitest/pytest require the
/// exact-case prefix, so a case-insensitive match there would recognize
/// helpers the runner itself would never run (an unexported `testHelper` is
/// not a real `Test` entry point).
fn name_signature_marks_test(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> bool {
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

/// Walk one node, accumulating into `tally`.
///
/// `nesting` is the count of enclosing constructs that opened a nesting level.
/// `loop_nesting` is the same count restricted to loops. `depth` is the raw
/// tree depth of `node` below the function's own root (its children, if any,
/// are walked at `depth + 1`) — a plain recursion-depth counter, unrelated to
/// `nesting`, that exists solely to bound the native call stack. See
/// [`MAX_TRAVERSAL_DEPTH`].
fn walk(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
) {
    if depth > MAX_TRAVERSAL_DEPTH {
        tally.depth_capped();
        return;
    }

    let kind = effective_kind(node, spec, source);

    // A nested function is its own unit — `collect_functions` scores it.
    if spec.function_kinds.contains(&kind) {
        return;
    }

    if spec.nesting_kinds.contains(&kind) {
        tally.nesting_increment(nesting);
        let inner_loop_nesting = if spec.loop_kinds.contains(&kind) {
            tally.max_loop_nesting = tally.max_loop_nesting.max(loop_nesting + 1);
            loop_nesting + 1
        } else {
            loop_nesting
        };
        if spec.conditional_kinds.contains(&kind) {
            tally.branch_count += 1;
            walk_conditional(
                node,
                source,
                spec,
                nesting,
                inner_loop_nesting,
                depth,
                tally,
                1,
            );
        } else {
            walk_children(
                node,
                source,
                spec,
                nesting + 1,
                inner_loop_nesting,
                depth + 1,
                tally,
            );
        }
        return;
    }

    if spec.arm_kinds.contains(&kind) {
        // Transparent: an arm is a branch of one decision, not a nested
        // decision. No increment and no nesting level.
        tally.branch_count += 1;
        walk_children(node, source, spec, nesting, loop_nesting, depth + 1, tally);
        return;
    }

    if spec.nest_only_kinds.contains(&kind) {
        walk_children(
            node,
            source,
            spec,
            nesting + 1,
            loop_nesting,
            depth + 1,
            tally,
        );
        return;
    }

    if spec.labelled_jump_kinds.contains(&kind) && carries_label(node, spec) {
        tally.flat_increment();
        return;
    }

    if is_boolean_root(node, spec) {
        walk_boolean(node, source, spec, nesting, loop_nesting, depth, tally);
        return;
    }

    walk_children(node, source, spec, nesting, loop_nesting, depth + 1, tally);
}

/// Walk a node's named children at the given levels. `depth` is already the
/// depth at which each child is walked — the caller adds the one level
/// crossing from `node` to its children before calling in.
fn walk_children(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, source, spec, nesting, loop_nesting, depth, tally);
    }
}

/// Walk one conditional's primary body and its `alternative` chain, whichever
/// shape the grammar uses — see the module-level doc for the two shapes
/// [`walk_alternative`] handles.
///
/// `chain` is the 1-based position of `node` in the else-if chain, so
/// [`FunctionComplexity::max_else_if_chain`] can record the longest one.
///
/// `depth` is `node`'s own raw tree depth — see [`walk`]'s doc. This function
/// is a recursive-walk entry point in its own right (an else-if chain link
/// re-enters it directly from [`walk_alternative`] without going back through
/// [`walk`]'s guard), so it re-checks [`MAX_TRAVERSAL_DEPTH`] itself.
#[allow(clippy::too_many_arguments)]
fn walk_conditional(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
    chain: u32,
) {
    if depth > MAX_TRAVERSAL_DEPTH {
        tally.depth_capped();
        return;
    }

    if spec.positional_conditional {
        walk_positional_conditional(
            node,
            source,
            spec,
            nesting,
            loop_nesting,
            depth,
            tally,
            chain,
        );
        return;
    }

    let condition = child_by_field_or_kind(node, spec.condition_field);
    if let Some(condition) = condition {
        walk(
            condition,
            source,
            spec,
            nesting + 1,
            loop_nesting,
            depth + 1,
            tally,
        );
    }

    if !spec.else_marker_kinds.is_empty() {
        walk_marker_conditional(
            node,
            source,
            spec,
            nesting,
            loop_nesting,
            depth,
            tally,
            chain,
            condition,
        );
        return;
    }

    if let Some(consequence) = child_by_field_or_kind(node, spec.consequence_field) {
        if spec.alternative_nested_in_consequence {
            walk_consequence_with_nested_alternative(
                consequence,
                source,
                spec,
                nesting,
                loop_nesting,
                depth,
                tally,
                chain,
            );
        } else {
            walk(
                consequence,
                source,
                spec,
                nesting + 1,
                loop_nesting,
                depth + 1,
                tally,
            );
        }
    }

    let mut cursor = node.walk();
    let mut chain = chain;
    for alt in node.children_by_field_name("alternative", &mut cursor) {
        chain = walk_alternative(
            alt,
            source,
            spec,
            nesting,
            loop_nesting,
            depth + 1,
            tally,
            chain,
        );
    }
}

/// Walk a conditional whose alternative sits as a POSITIONAL sibling marked
/// by an [`ComplexitySpec::else_marker_kinds`] token rather than any field —
/// Swift's `if_statement`, verified via `node-types.json`: its own field list
/// holds only `condition`; `else`/`if_statement`/`statements` are unnamed
/// "children". `condition` is passed in already resolved (and already
/// walked by the caller) so this only needs to identify it by identity when
/// scanning `node`'s children.
#[allow(clippy::too_many_arguments)]
fn walk_marker_conditional(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
    chain: u32,
    condition: Option<Node<'_>>,
) {
    let condition_id = condition.map(|c| c.id());
    let mut cursor = node.walk();
    let children: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
    drop(cursor);

    let mut saw_marker = false;
    let mut chain = chain;
    for child in children {
        if condition_id == Some(child.id()) {
            continue;
        }
        if spec
            .else_marker_kinds
            .contains(&effective_kind(child, spec, source))
        {
            saw_marker = true;
            continue;
        }
        if saw_marker {
            chain = walk_alternative(
                child,
                source,
                spec,
                nesting,
                loop_nesting,
                depth + 1,
                tally,
                chain,
            );
            saw_marker = false;
        } else {
            walk(
                child,
                source,
                spec,
                nesting + 1,
                loop_nesting,
                depth + 1,
                tally,
            );
        }
    }
}

/// Walk a conditional that carries NO fields at all — Fortran's
/// `if_statement`, verified via `node-types.json`: `if_statement`,
/// `elseif_clause`, and `else_clause` each report an empty field list, so the
/// condition is the first named child, zero or more ordinary statements are
/// the primary consequence, then zero or more [`ComplexitySpec::elif_kinds`]
/// siblings, then optionally one terminal body of its own, then a
/// [`ComplexitySpec::statement_terminator_kinds`] marker (`end_if_statement`)
/// that stops the scan before it is misread as one more terminal
/// alternative. A recursive re-entry for a chain link (`elseif_clause`) sees
/// only its own condition+body children — Fortran attaches every
/// `elseif_clause`/`else_clause` as a FLAT sibling of the ORIGINAL
/// `if_statement`, never nested inside the previous link — so this same
/// scan naturally finds no further chain kinds and treats them all as plain
/// consequence statements.
#[allow(clippy::too_many_arguments)]
fn walk_positional_conditional(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
    chain: u32,
) {
    let mut cursor = node.walk();
    let children: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
    drop(cursor);

    let mut iter = children.into_iter();
    let Some(condition) = iter.next() else {
        return;
    };
    walk(
        condition,
        source,
        spec,
        nesting + 1,
        loop_nesting,
        depth + 1,
        tally,
    );

    let mut in_chain = false;
    let mut chain = chain;
    for child in iter {
        let kind = child.kind();
        if spec.statement_terminator_kinds.contains(&kind) {
            break;
        }
        if !in_chain && (spec.conditional_kinds.contains(&kind) || spec.elif_kinds.contains(&kind))
        {
            in_chain = true;
        }
        if in_chain {
            chain = walk_alternative(
                child,
                source,
                spec,
                nesting,
                loop_nesting,
                depth + 1,
                tally,
                chain,
            );
        } else {
            walk(
                child,
                source,
                spec,
                nesting + 1,
                loop_nesting,
                depth + 1,
                tally,
            );
        }
    }
}

/// Walk a conditional's consequence when the grammar nests the alternative
/// INSIDE that same consequence container, as its own trailing child, rather
/// than as a separate field of the conditional itself — Elixir's `do_block`,
/// verified by parsing `if a do 1 else 2 end`: its `do_block` field holds `1`
/// followed by an `else_block`, both as children of the SAME `do_block`, with
/// no separate `alternative` field on the `if` call at all. Every child
/// except a trailing [`ComplexitySpec::else_wrapper_kinds`] one is the
/// primary body; that trailing child, if present, is the alternative.
#[allow(clippy::too_many_arguments)]
fn walk_consequence_with_nested_alternative(
    consequence: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
    chain: u32,
) {
    let mut cursor = consequence.walk();
    let children: Vec<Node<'_>> = consequence.named_children(&mut cursor).collect();
    drop(cursor);

    let alt_index = children
        .last()
        .filter(|c| {
            spec.else_wrapper_kinds
                .contains(&effective_kind(**c, spec, source))
        })
        .map(|_| children.len() - 1);

    for (i, child) in children.iter().enumerate() {
        if Some(i) == alt_index {
            continue;
        }
        walk(
            *child,
            source,
            spec,
            nesting + 1,
            loop_nesting,
            depth + 1,
            tally,
        );
    }
    if let Some(i) = alt_index {
        walk_alternative(
            children[i],
            source,
            spec,
            nesting,
            loop_nesting,
            depth + 1,
            tally,
            chain,
        );
    }
}

/// Score one value of a conditional's `alternative` field and return the
/// chain position the NEXT link should use.
///
/// Unwraps a transparent [`ComplexitySpec::else_wrapper_kinds`] node first, if
/// the grammar uses one (Rust/C/C++/JavaScript/TypeScript's `else_clause`).
/// What remains is then either:
///
/// - a **chain link** — [`ComplexitySpec::conditional_kinds`] (a bare nested
///   `if`, Java's/C#'s shape once unwrapped, or Rust/C/C++/JS/TS's shape
///   after the `else_clause` unwrap) or [`ComplexitySpec::elif_kinds`]
///   (Python's `elif_clause`, PHP's `else_if_clause`, which carry their own
///   `condition` and never need unwrapping) — scored with a flat +1 at the
///   SAME nesting level, then walked recursively via [`walk_conditional`] for
///   its own body and any FURTHER alternatives; or
/// - a **terminal else** — a bare body (Java's/C#'s direct `block`, or the
///   single child left after unwrapping an `else_clause`) — scored with a
///   flat +1 and its own body walked one level deeper.
///
/// `depth` is `node`'s own raw tree depth — see [`walk`]'s doc. An else-if
/// wrapper unwrap re-enters this function on the wrapper's child without
/// going through [`walk`]'s guard, so it re-checks [`MAX_TRAVERSAL_DEPTH`]
/// itself.
#[allow(clippy::too_many_arguments)]
fn walk_alternative(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
    chain: u32,
) -> u32 {
    if depth > MAX_TRAVERSAL_DEPTH {
        tally.depth_capped();
        return chain;
    }

    let kind = effective_kind(node, spec, source);

    if spec.else_wrapper_kinds.contains(&kind) {
        let mut result = chain;
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            result = walk_alternative(
                child,
                source,
                spec,
                nesting,
                loop_nesting,
                depth + 1,
                tally,
                result,
            );
        }
        return result;
    }

    if spec.conditional_kinds.contains(&kind) || spec.elif_kinds.contains(&kind) {
        tally.flat_increment();
        tally.branch_count += 1;
        tally.max_else_if_chain = tally.max_else_if_chain.max(chain);
        tally.max_nesting_depth = tally.max_nesting_depth.max(nesting + 1);
        walk_conditional(
            node,
            source,
            spec,
            nesting,
            loop_nesting,
            depth,
            tally,
            chain + 1,
        );
        return chain + 1;
    }

    tally.flat_increment();
    tally.branch_count += 1;
    walk_children(
        node,
        source,
        spec,
        nesting + 1,
        loop_nesting,
        depth + 1,
        tally,
    );
    chain
}

/// Whether a jump names a label, which is what makes it a jump rather than
/// ordinary control flow.
fn carries_label(node: Node<'_>, spec: &ComplexitySpec) -> bool {
    let mut cursor = node.walk();
    let labelled = node
        .named_children(&mut cursor)
        .any(|child| spec.label_kinds.contains(&child.kind()));
    drop(cursor);
    labelled
}

/// The logical operator token of `node`, when it has one.
fn logical_operator<'s>(node: Node<'_>, spec: &'s ComplexitySpec) -> Option<&'s str> {
    if !spec.logical_kinds.contains(&node.kind()) {
        return None;
    }
    let mut cursor = node.walk();
    // An operator token is an anonymous node whose kind IS its text. The cursor
    // must outlive the iterator, so the search is bound before returning.
    let operator = node
        .children(&mut cursor)
        .find_map(|child| {
            spec.logical_operators
                .iter()
                .find(|op| **op == child.kind())
        })
        .copied();
    drop(cursor);
    operator
}

/// Whether `node` is the top of a boolean-operator chain.
fn is_boolean_root(node: Node<'_>, spec: &ComplexitySpec) -> bool {
    logical_operator(node, spec).is_some()
        && node
            .parent()
            .is_none_or(|parent| logical_operator(parent, spec).is_none())
}

/// Score one boolean-operator chain: +1 per sequence of like operators, and the
/// chain's operand count as evidence.
fn walk_boolean(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
) {
    let mut sequences = 0u32;
    let mut operators = 0u32;
    boolean_chain(
        node,
        source,
        spec,
        nesting,
        loop_nesting,
        depth,
        tally,
        None,
        &mut sequences,
        &mut operators,
    );
    tally.cognitive_score += sequences;
    tally.max_boolean_operands = tally.max_boolean_operands.max(operators + 1);
}

/// Recurse a boolean chain, counting operator-run changes and walking every
/// non-boolean operand normally. `depth` is the current node's own raw tree
/// depth — see [`walk`]'s doc; each recursive step into an operand crosses one
/// tree level, so it re-checks [`MAX_TRAVERSAL_DEPTH`] itself rather than
/// relying on [`walk`]'s guard, which only sees the operands, not the operator
/// nodes between them.
#[allow(clippy::too_many_arguments)]
fn boolean_chain(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    depth: u32,
    tally: &mut Tally,
    parent_operator: Option<&str>,
    sequences: &mut u32,
    operators: &mut u32,
) {
    if depth > MAX_TRAVERSAL_DEPTH {
        tally.depth_capped();
        return;
    }

    let Some(operator) = logical_operator(node, spec) else {
        // A plain operand: it may still hold conditions of its own.
        walk(node, source, spec, nesting, loop_nesting, depth, tally);
        return;
    };

    *operators += 1;
    if parent_operator != Some(operator) {
        *sequences += 1;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        boolean_chain(
            child,
            source,
            spec,
            nesting,
            loop_nesting,
            depth + 1,
            tally,
            Some(operator),
            sequences,
            operators,
        );
    }
}

#[cfg(test)]
mod tests;
