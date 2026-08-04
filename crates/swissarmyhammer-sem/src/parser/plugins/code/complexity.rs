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
//!   terminal `else_clause` rather than nested inside one another.

use tree_sitter::Node;

use super::languages::{dotted_lowercase_extension, get_language_config};

/// The cognitive-complexity score at or above which a function is flagged. The
/// Sonar default.
pub const COGNITIVE_COMPLEXITY_THRESHOLD: u32 = 15;

/// The condition-nesting depth at or above which a function is flagged. The
/// depth `cognitive-complexity.md` already stated: "conditions nested more than
/// 3 levels deep (4+ is a flag)".
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
}

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
};

/// Shared field values for TypeScript, TSX, and JavaScript. All three
/// grammars are C-like and produce identical node kinds for every field
/// except the language id itself, confirmed by parsing the same
/// control-flow and decorator samples under each grammar.
const fn typescript_family_spec(language: &'static str) -> ComplexitySpec {
    ComplexitySpec {
        language,
        function_kinds: &[
            "function_declaration",
            "method_definition",
            "arrow_function",
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
];

/// The spec for a language id, or `None` when that language has no mapping.
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
    let extension = dotted_lowercase_extension(path)?;
    let config = get_language_config(&extension)?;
    let spec = spec_for_language(config.id)?;
    let language = (config.get_language)()?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(source, None)?;

    let mut functions = Vec::new();
    collect_functions(tree.root_node(), source, spec, 0, &mut functions);
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
    if depth > MAX_TRAVERSAL_DEPTH {
        // Stop descending rather than risk the native call stack. A function
        // nested this deep in surrounding structure is unreachable in any
        // real file; the walk simply never finds it.
        return;
    }
    if spec.function_kinds.contains(&node.kind()) {
        out.push(score_function(node, source, spec));
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_functions(child, source, spec, depth + 1, out);
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
fn function_name(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> String {
    node.child_by_field_name(spec.name_field)
        .and_then(|n| resolve_declarator_name(n, source))
        .unwrap_or("<anonymous>")
        .to_string()
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
fn node_text<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
    source.get(node.start_byte()..node.end_byte())
}

/// Whether the definition is marked as a test, wherever the grammar attaches
/// the marker: a contiguous run of preceding siblings (Rust's `#[attr]`,
/// Python's decorator inside the shared `decorated_definition` wrapper,
/// TypeScript's decorator as a `class_body` sibling), a direct child of the
/// definition itself (JavaScript's bare `decorator` field), or a child
/// wrapped in a container the grammar nests inside the definition (Java's
/// `modifiers`, C#'s `attribute_list`, PHP's `attribute_list`/
/// `attribute_group`, C/C++'s `attribute_declaration`). The file name is
/// never consulted.
fn is_test_definition(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> bool {
    let mut sibling = node.prev_named_sibling();
    while let Some(current) = sibling {
        if !spec.attribute_kinds.contains(&current.kind()) {
            break;
        }
        if attribute_marks_test(current, source) {
            return true;
        }
        sibling = current.prev_named_sibling();
    }

    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .any(|child| node_carries_test_attribute(child, spec, source));
    drop(cursor);
    found
}

/// Whether `node` is a test-marking attribute itself, or (through however
/// many [`ComplexitySpec::attribute_container_kinds`] wrapper levels the
/// grammar nests) contains one. Recurses through consecutive container
/// levels generically — PHP nests two deep (`attribute_list` >
/// `attribute_group` > `attribute`), Java/C#/C/C++ nest one.
fn node_carries_test_attribute(node: Node<'_>, spec: &ComplexitySpec, source: &str) -> bool {
    if spec.attribute_kinds.contains(&node.kind()) {
        return attribute_marks_test(node, source);
    }
    if spec.attribute_container_kinds.contains(&node.kind()) {
        let mut cursor = node.walk();
        let found = node
            .named_children(&mut cursor)
            .any(|child| node_carries_test_attribute(child, spec, source));
        drop(cursor);
        return found;
    }
    false
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
    let Some(text) = node_text(node, source) else {
        return false;
    };
    let inner = text
        .trim()
        .trim_start_matches('#')
        .trim_start_matches('@')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let path = inner.split('(').next().unwrap_or(inner).trim();
    path.rsplit(['.', ':', '\\'])
        .next()
        .is_some_and(|last| last.eq_ignore_ascii_case("test"))
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

    let kind = node.kind();

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

    if let Some(condition) = node.child_by_field_name("condition") {
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
    if let Some(consequence) = node.child_by_field_name(spec.consequence_field) {
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

    if spec.else_wrapper_kinds.contains(&node.kind()) {
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

    if spec.conditional_kinds.contains(&node.kind()) || spec.elif_kinds.contains(&node.kind()) {
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
mod tests {
    use super::*;

    /// How many times a determinism test re-scores the same source. One run
    /// proves nothing about drift; the point of this module is that N runs agree.
    const DETERMINISM_RUNS: usize = 25;

    /// Score `source` as a Rust file and return its only function.
    fn only_function(source: &str) -> FunctionComplexity {
        let file = cognitive_complexity("src/lib.rs", source).expect("rust is a mapped language");
        assert_eq!(
            file.functions.len(),
            1,
            "fixture should hold exactly one function, got {:?}",
            file.functions
        );
        file.functions.into_iter().next().expect("one function")
    }

    /// Score `source` as `file` and return its only function. Shared by
    /// every per-language test below — only the file path (whose extension
    /// selects the language) differs between languages.
    fn only_function_for(file: &str, source: &str) -> FunctionComplexity {
        let scored = cognitive_complexity(file, source)
            .unwrap_or_else(|| panic!("{file} should be a mapped language"));
        assert_eq!(scored.functions.len(), 1, "got {:?}", scored.functions);
        scored.functions.into_iter().next().expect("one function")
    }

    /// Prefix a PHP fixture body with the `<?php` opening tag every
    /// `only_function_for("src/lib.php", ...)` call needs.
    fn php_source(body: &str) -> String {
        format!("<?php\n{body}")
    }

    /// Look up `name` in the class(es) parsed from `source` as `file` and
    /// return its complexity. Shared by the Java and C# tests below — only
    /// the file path (whose extension selects the language) differs.
    fn method_in_class(file: &str, source: &str, name: &str) -> FunctionComplexity {
        let parsed = cognitive_complexity(file, source)
            .unwrap_or_else(|| panic!("{file} should be a mapped language"));
        parsed
            .functions
            .into_iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("{name} is scored"))
    }

    /// `collect_line_tags` exactly as it stood when the review flagged it for
    /// "match arms contain code at depth 4". It is a two-arm `Option` match
    /// inside one `if` inside one `while`.
    const COLLECT_LINE_TAGS: &str = r#"
fn collect_line_tags(line: &str, tags: &mut BTreeSet<String>) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            i = skip_inline_code(bytes, i);
            continue;
        }
        match tag_slug_at(bytes, i) {
            Some(slug) => {
                tags.insert(line[slug.clone()].to_string());
                i = slug.end;
            }
            None => i += 1,
        }
    }
}
"#;

    /// `edit_line_markers` exactly as it stood when the review flagged it, the
    /// second false positive the card records.
    const EDIT_LINE_MARKERS: &str = r#"
fn edit_line_markers(line: &str, slug: &str, replacement: Option<&str>, out: &mut String) -> bool {
    let bytes = line.as_bytes();
    let line_start = out.len();
    let mut i = 0;
    let mut edited = false;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let end = skip_inline_code(bytes, i);
            out.push_str(&line[i..end]);
            i = end;
            continue;
        }
        match tag_slug_at(bytes, i).filter(|found| line[found.clone()] == *slug) {
            Some(found) => {
                edited = true;
                i = found.end;
                if let Some(text) = replacement {
                    out.push_str(text);
                } else if i < bytes.len() && bytes[i] == b' ' {
                    i += 1;
                } else if out.len() > line_start && out.ends_with(' ') {
                    out.pop();
                }
            }
            None => {
                let ch = line[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    edited
}
"#;

    #[test]
    fn collect_line_tags_sits_far_below_both_gates() {
        let scored = only_function(COLLECT_LINE_TAGS);

        assert_eq!(scored.cognitive_score, 5, "while + if + match, nested once");
        assert_eq!(
            scored.max_nesting_depth, 2,
            "the match sits inside one while, so depth is 2 — never the 4 the review reported"
        );
        assert!(
            !scored.exceeds_gates(),
            "the function the review flagged must not trip either gate: {scored:?}"
        );
    }

    #[test]
    fn edit_line_markers_sits_below_both_gates() {
        let scored = only_function(EDIT_LINE_MARKERS);

        assert_eq!(
            scored.max_nesting_depth, 3,
            "while > match > if let is depth 3, one short of the gate"
        );
        assert!(
            scored.cognitive_score < COGNITIVE_COMPLEXITY_THRESHOLD,
            "score {} should stay under the gate",
            scored.cognitive_score
        );
        assert!(
            !scored.exceeds_gates(),
            "the second function the review flagged must not trip either gate: {scored:?}"
        );
    }

    #[test]
    fn match_arms_are_one_decision_not_one_nesting_level_each() {
        let scored = only_function(
            r#"
fn classify(value: u8) -> u8 {
    match value {
        0 => 10,
        1 => 11,
        2 => 12,
        3 => 13,
        4 => 14,
        _ => 15,
    }
}
"#,
        );

        assert_eq!(
            scored.cognitive_score, 1,
            "a match scores once for the whole construct, never once per arm"
        );
        assert_eq!(
            scored.max_nesting_depth, 1,
            "arms are branches of one decision, so they open no nesting level"
        );
        assert_eq!(scored.branch_count, 6, "six arms are six decision points");
        assert!(
            !scored.exceeds_gates(),
            "many simple arms must stay unflagged: {scored:?}"
        );
    }

    #[test]
    fn an_if_else_if_else_chain_scores_one_per_branch_with_no_nesting() {
        let scored = only_function(
            r#"
fn pick(a: bool, b: bool) -> u8 {
    if a {
        1
    } else if b {
        2
    } else {
        3
    }
}
"#,
        );

        assert_eq!(
            scored.cognitive_score, 3,
            "if + else if + else is 1 + 1 + 1"
        );
        assert_eq!(
            scored.max_nesting_depth, 1,
            "an else-if chain is a flat set of branches, not a staircase"
        );
        assert_eq!(scored.max_else_if_chain, 1, "one else-if link");
    }

    #[test]
    fn nesting_deepens_the_score_and_trips_the_depth_gate() {
        let scored = only_function(
            r#"
fn deep(a: bool, b: bool, items: &[u8]) -> u8 {
    if a {
        for item in items {
            while b {
                if *item > 0 {
                    return 1;
                }
            }
        }
    }
    0
}
"#,
        );

        assert_eq!(
            scored.cognitive_score, 10,
            "1 + 2 + 3 + 4 as each construct nests one deeper"
        );
        assert_eq!(
            scored.max_nesting_depth, 4,
            "if > for > while > if is depth 4"
        );
        assert_eq!(scored.max_loop_nesting, 2, "for > while is two loops deep");
        assert!(
            scored.exceeds_gates(),
            "genuinely deep nesting must still be flagged: {scored:?}"
        );
    }

    #[test]
    fn one_run_of_like_boolean_operators_scores_once() {
        let scored = only_function(
            r#"
fn all_three(a: bool, b: bool, c: bool) -> u8 {
    if a && b && c {
        return 1;
    }
    0
}
"#,
        );

        assert_eq!(
            scored.cognitive_score, 2,
            "the if scores 1, the one && run 1"
        );
        assert_eq!(scored.max_boolean_operands, 3, "three operands in the run");
    }

    #[test]
    fn mixed_boolean_operators_score_once_per_run() {
        let scored = only_function(
            r#"
fn mixed(a: bool, b: bool, c: bool) -> u8 {
    if a && b || c {
        return 1;
    }
    0
}
"#,
        );

        assert_eq!(
            scored.cognitive_score, 3,
            "the if scores 1, and && then || are two runs"
        );
    }

    #[test]
    fn a_labelled_jump_adds_a_flat_increment() {
        let scored = only_function(
            r#"
fn scan(rows: &[u8], cols: &[u8]) -> u8 {
    'outer: for row in rows {
        for col in cols {
            if row == col {
                continue 'outer;
            }
        }
    }
    0
}
"#,
        );

        assert_eq!(
            scored.cognitive_score, 7,
            "for 1 + for 2 + if 3 + labelled continue 1"
        );
    }

    #[test]
    fn a_break_carrying_a_value_is_not_a_labelled_jump() {
        // `break 5` returns a value from a loop. It is ordinary control flow, not
        // a jump to a label, so it must not add an increment.
        let scored = only_function(
            r#"
fn first(items: &[u8]) -> u8 {
    loop {
        break 5;
    }
}
"#,
        );

        assert_eq!(
            scored.cognitive_score, 1,
            "only the loop scores; a valued break is not a labelled jump"
        );
    }

    #[test]
    fn a_test_attribute_at_the_definition_exempts_the_function() {
        let file = cognitive_complexity(
            "src/lib.rs",
            r#"
#[test]
fn deeply_nested_test(a: bool, b: bool, items: &[u8]) {
    if a {
        for item in items {
            while b {
                if *item > 0 {
                    assert!(true);
                }
            }
        }
    }
}
"#,
        )
        .expect("rust is a mapped language");
        let scored = &file.functions[0];

        assert!(scored.is_test, "#[test] marks the definition as a test");
        assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
        assert!(
            !scored.exceeds_gates(),
            "a test is exempt even at depth 4: {scored:?}"
        );
    }

    #[test]
    fn a_namespaced_test_attribute_marks_a_test() {
        let file = cognitive_complexity("src/lib.rs", "#[tokio::test]\nasync fn spawns() {}\n")
            .expect("rust is a mapped language");

        assert!(
            file.functions[0].is_test,
            "#[tokio::test] is a test attribute"
        );
    }

    #[test]
    fn a_helper_beside_tests_is_not_a_test() {
        let file = cognitive_complexity(
            "src/tag_parser_test.rs",
            r#"
#[test]
fn a_real_test() {}

fn build_request(a: bool) -> u8 {
    if a { 1 } else { 0 }
}
"#,
        )
        .expect("rust is a mapped language");
        let helper = file
            .functions
            .iter()
            .find(|f| f.name == "build_request")
            .expect("the helper is scored");

        assert!(
            !helper.is_test,
            "a helper is judged at its own definition, never by the file name"
        );
    }

    #[test]
    fn an_unmapped_language_is_not_computed_rather_than_zero() {
        assert!(
            cognitive_complexity("src/app.rb", "def f\nend\n").is_none(),
            "a language with no spec must report not-computed, never a zero score"
        );
    }

    #[test]
    fn a_non_code_path_is_not_computed() {
        assert!(
            cognitive_complexity("README.md", "# Title\n").is_none(),
            "a path with no language mapping must report not-computed"
        );
    }

    #[test]
    fn methods_in_an_impl_block_are_scored_individually() {
        let file = cognitive_complexity(
            "src/lib.rs",
            r#"
impl Parser {
    fn simple(&self) -> u8 {
        0
    }

    fn branchy(&self, a: bool) -> u8 {
        if a { 1 } else { 0 }
    }
}
"#,
        )
        .expect("rust is a mapped language");

        let names: Vec<&str> = file.functions.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["simple", "branchy"], "both methods are scored");
        assert_eq!(file.functions[0].cognitive_score, 0);
        assert_eq!(file.functions[1].cognitive_score, 2, "if plus else");
    }

    #[test]
    fn repeated_scoring_of_one_source_never_drifts() {
        // The whole point of the probe: the number handed to the model is a pure
        // function of the source, so N runs agree exactly. No model is involved,
        // so this belongs in CI rather than in a manual N-run review harness.
        let source = format!("{COLLECT_LINE_TAGS}\n{EDIT_LINE_MARKERS}");
        let first =
            cognitive_complexity("src/tag_parser.rs", &source).expect("rust is a mapped language");

        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/tag_parser.rs", &source)
                .expect("rust is a mapped language");
            assert_eq!(
                again, first,
                "run {run} produced a different score set than run 0"
            );
        }
    }

    #[test]
    fn the_scorer_is_insensitive_to_the_order_functions_are_declared_in() {
        // Two files with the same functions in either order must score each
        // function the same. A scorer that leaked state between functions would
        // fail here even though every single-function test passed.
        let forward = cognitive_complexity(
            "src/a.rs",
            &format!("{COLLECT_LINE_TAGS}\n{EDIT_LINE_MARKERS}"),
        )
        .expect("rust is a mapped language");
        let reversed = cognitive_complexity(
            "src/a.rs",
            &format!("{EDIT_LINE_MARKERS}\n{COLLECT_LINE_TAGS}"),
        )
        .expect("rust is a mapped language");

        let score_of = |file: &FileComplexity, name: &str| -> u32 {
            file.functions
                .iter()
                .find(|f| f.name == name)
                .unwrap_or_else(|| panic!("{name} is scored"))
                .cognitive_score
        };

        assert_eq!(
            score_of(&forward, "collect_line_tags"),
            score_of(&reversed, "collect_line_tags")
        );
        assert_eq!(
            score_of(&forward, "edit_line_markers"),
            score_of(&reversed, "edit_line_markers")
        );
    }

    // -----------------------------------------------------------------
    // TypeScript
    // -----------------------------------------------------------------

    #[test]
    fn typescript_switch_scores_once_and_arms_open_no_nesting() {
        let scored = only_function_for(
            "src/lib.ts",
            r#"
function classify(value: number): number {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
        );
        assert_eq!(
            scored.cognitive_score, 1,
            "a switch scores once for the whole construct"
        );
        assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
        assert_eq!(
            scored.branch_count, 3,
            "three arms are three decision points"
        );
        assert!(!scored.exceeds_gates());
    }

    #[test]
    fn typescript_if_else_if_else_chain_is_flat() {
        let scored = only_function_for(
            "src/lib.ts",
            r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
        );
        assert_eq!(
            scored.cognitive_score, 3,
            "if + else if + else is 1 + 1 + 1"
        );
        assert_eq!(
            scored.max_nesting_depth, 1,
            "an else-if chain is flat, not a staircase"
        );
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn typescript_nested_loops_deepen_the_score() {
        let scored = only_function_for(
            "src/lib.ts",
            r#"
function deep(a: boolean, b: boolean, items: number[]): number {
    if (a) {
        for (const item of items) {
            while (b) {
                if (item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        );
        assert_eq!(
            scored.cognitive_score, 10,
            "1 + 2 + 3 + 4 as each construct nests one deeper"
        );
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(
            scored.max_loop_nesting, 2,
            "for-of > while is two loops deep"
        );
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn typescript_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = only_function_for(
            "src/lib.ts",
            r#"
function allThree(a: boolean, b: boolean, c: boolean): number {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(
            and_only.cognitive_score, 2,
            "the if scores 1, the one && run 1"
        );
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = only_function_for(
            "src/lib.ts",
            r#"
function mixed(a: boolean, b: boolean, c: boolean): number {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(
            mixed.cognitive_score, 3,
            "the if scores 1, and && then || are two runs"
        );
    }

    #[test]
    fn typescript_test_decorator_at_the_definition_exempts_the_method() {
        let file = cognitive_complexity(
            "src/lib.ts",
            r#"
class Foo {
    @Test
    deeplyNested(a: boolean, b: boolean, items: number[]): number {
        if (a) {
            for (const item of items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
        )
        .expect("typescript is a mapped language");
        let scored = &file.functions[0];

        assert!(scored.is_test, "@Test marks the method as a test");
        assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
        assert!(
            !scored.exceeds_gates(),
            "a test is exempt even at depth 4: {scored:?}"
        );
    }

    #[test]
    fn typescript_repeated_scoring_never_drifts() {
        let source = r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
        let first = cognitive_complexity("src/lib.ts", source).expect("typescript is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/lib.ts", source).expect("typescript is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // TSX
    // -----------------------------------------------------------------

    #[test]
    fn tsx_switch_scores_once_and_arms_open_no_nesting() {
        let scored = only_function_for(
            "src/App.tsx",
            r#"
function classify(value: number): number {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 1);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.branch_count, 3);
    }

    #[test]
    fn tsx_if_else_if_else_chain_is_flat() {
        let scored = only_function_for(
            "src/App.tsx",
            r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 3);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn tsx_nested_loops_deepen_the_score() {
        let scored = only_function_for(
            "src/App.tsx",
            r#"
function deep(a: boolean, b: boolean, items: number[]): number {
    if (a) {
        for (const item of items) {
            while (b) {
                if (item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(scored.max_loop_nesting, 2);
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn tsx_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = only_function_for(
            "src/App.tsx",
            r#"
function allThree(a: boolean, b: boolean, c: boolean): number {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(and_only.cognitive_score, 2);
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = only_function_for(
            "src/App.tsx",
            r#"
function mixed(a: boolean, b: boolean, c: boolean): number {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(mixed.cognitive_score, 3);
    }

    #[test]
    fn tsx_test_decorator_at_the_definition_exempts_the_method() {
        let file = cognitive_complexity(
            "src/App.tsx",
            r#"
class Foo {
    @Test
    deeplyNested(a: boolean, b: boolean, items: number[]): number {
        if (a) {
            for (const item of items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
        )
        .expect("tsx is a mapped language");
        let scored = &file.functions[0];

        assert!(scored.is_test);
        assert_eq!(scored.max_nesting_depth, 4);
        assert!(!scored.exceeds_gates());
    }

    #[test]
    fn tsx_repeated_scoring_never_drifts() {
        let source = r#"
function pick(a: boolean, b: boolean): number {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
        let first = cognitive_complexity("src/App.tsx", source).expect("tsx is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/App.tsx", source).expect("tsx is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // JavaScript
    // -----------------------------------------------------------------

    #[test]
    fn javascript_switch_scores_once_and_arms_open_no_nesting() {
        let scored = only_function_for(
            "src/lib.js",
            r#"
function classify(value) {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 1);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.branch_count, 3);
    }

    #[test]
    fn javascript_if_else_if_else_chain_is_flat() {
        let scored = only_function_for(
            "src/lib.js",
            r#"
function pick(a, b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 3);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn javascript_nested_loops_deepen_the_score() {
        let scored = only_function_for(
            "src/lib.js",
            r#"
function deep(a, b, items) {
    if (a) {
        for (const item of items) {
            while (b) {
                if (item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(scored.max_loop_nesting, 2);
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn javascript_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = only_function_for(
            "src/lib.js",
            r#"
function allThree(a, b, c) {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(and_only.cognitive_score, 2);
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = only_function_for(
            "src/lib.js",
            r#"
function mixed(a, b, c) {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(mixed.cognitive_score, 3);
    }

    #[test]
    fn javascript_test_decorator_at_the_definition_exempts_the_method() {
        let file = cognitive_complexity(
            "src/lib.js",
            r#"
class Foo {
    @Test
    deeplyNested(a, b, items) {
        if (a) {
            for (const item of items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
        )
        .expect("javascript is a mapped language");
        let scored = &file.functions[0];

        assert!(scored.is_test, "@Test marks the method as a test");
        assert_eq!(scored.max_nesting_depth, 4);
        assert!(!scored.exceeds_gates());
    }

    #[test]
    fn javascript_repeated_scoring_never_drifts() {
        let source = r#"
function pick(a, b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
        let first = cognitive_complexity("src/lib.js", source).expect("javascript is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/lib.js", source).expect("javascript is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // Python
    // -----------------------------------------------------------------

    #[test]
    fn python_match_scores_once_and_cases_open_no_nesting() {
        let scored = only_function_for(
            "src/lib.py",
            r#"
def classify(value):
    match value:
        case 0:
            return 10
        case 1:
            return 11
        case _:
            return 15
"#,
        );
        assert_eq!(
            scored.cognitive_score, 1,
            "a match scores once for the whole construct"
        );
        assert_eq!(scored.max_nesting_depth, 1, "cases open no nesting level");
        assert_eq!(
            scored.branch_count, 3,
            "three cases are three decision points"
        );
    }

    #[test]
    fn python_if_elif_else_chain_is_flat() {
        let scored = only_function_for(
            "src/lib.py",
            r#"
def pick(a, b):
    if a:
        return 1
    elif b:
        return 2
    else:
        return 3
"#,
        );
        assert_eq!(scored.cognitive_score, 3, "if + elif + else is 1 + 1 + 1");
        assert_eq!(
            scored.max_nesting_depth, 1,
            "an elif chain is flat, not a staircase"
        );
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn python_three_way_elif_chain_reports_the_longest_link() {
        let scored = only_function_for(
            "src/lib.py",
            r#"
def pick(a, b, c):
    if a:
        return 1
    elif b:
        return 2
    elif c:
        return 3
    else:
        return 4
"#,
        );
        assert_eq!(
            scored.cognitive_score, 4,
            "if + elif + elif + else is 1 + 1 + 1 + 1"
        );
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 2, "two elif links");
    }

    #[test]
    fn python_nested_loops_deepen_the_score() {
        let scored = only_function_for(
            "src/lib.py",
            r#"
def deep(a, b, items):
    if a:
        for item in items:
            while b:
                if item > 0:
                    return 1
    return 0
"#,
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(scored.max_loop_nesting, 2, "for > while is two loops deep");
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn python_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = only_function_for(
            "src/lib.py",
            r#"
def all_three(a, b, c):
    if a and b and c:
        return 1
    return 0
"#,
        );
        assert_eq!(
            and_only.cognitive_score, 2,
            "the if scores 1, the one `and` run 1"
        );
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = only_function_for(
            "src/lib.py",
            r#"
def mixed(a, b, c):
    if a and b or c:
        return 1
    return 0
"#,
        );
        assert_eq!(
            mixed.cognitive_score, 3,
            "the if scores 1, and `and` then `or` are two runs"
        );
    }

    #[test]
    fn python_test_decorator_at_the_definition_exempts_the_function() {
        let file = cognitive_complexity(
            "src/lib.py",
            r#"
@pytest.mark.test
def deeply_nested_test(a, b, items):
    if a:
        for item in items:
            while b:
                if item > 0:
                    return 1
    return 0
"#,
        )
        .expect("python is a mapped language");
        let scored = &file.functions[0];

        assert!(
            scored.is_test,
            "@pytest.mark.test marks the definition as a test"
        );
        assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
        assert!(
            !scored.exceeds_gates(),
            "a test is exempt even at depth 4: {scored:?}"
        );
    }

    #[test]
    fn python_repeated_scoring_never_drifts() {
        let source = r#"
def pick(a, b):
    if a:
        return 1
    elif b:
        return 2
    return 0
"#;
        let first = cognitive_complexity("src/lib.py", source).expect("python is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/lib.py", source).expect("python is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // Java
    // -----------------------------------------------------------------

    #[test]
    fn java_switch_scores_once_and_arms_open_no_nesting() {
        let scored = method_in_class(
            "src/Foo.java",
            r#"
class Foo {
    int classify(int value) {
        switch (value) {
            case 0:
                return 10;
            case 1:
                return 11;
            default:
                return 15;
        }
    }
}
"#,
            "classify",
        );
        assert_eq!(
            scored.cognitive_score, 1,
            "a switch scores once for the whole construct"
        );
        assert_eq!(scored.max_nesting_depth, 1, "arms open no nesting level");
        assert_eq!(scored.branch_count, 3);
    }

    #[test]
    fn java_if_else_if_else_chain_is_flat() {
        let scored = method_in_class(
            "src/Foo.java",
            r#"
class Foo {
    int pick(boolean a, boolean b) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        } else {
            return 3;
        }
    }
}
"#,
            "pick",
        );
        assert_eq!(scored.cognitive_score, 3);
        assert_eq!(
            scored.max_nesting_depth, 1,
            "an else-if chain is flat, not a staircase"
        );
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn java_three_way_else_if_chain_reports_the_longest_link() {
        let scored = method_in_class(
            "src/Foo.java",
            r#"
class Foo {
    int pick(boolean a, boolean b, boolean c) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        } else if (c) {
            return 3;
        } else {
            return 4;
        }
    }
}
"#,
            "pick",
        );
        assert_eq!(scored.cognitive_score, 4);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 2, "two else-if links");
    }

    #[test]
    fn java_nested_loops_deepen_the_score() {
        let scored = method_in_class(
            "src/Foo.java",
            r#"
class Foo {
    int deep(boolean a, boolean b, int[] items) {
        if (a) {
            for (int item : items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
            "deep",
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(
            scored.max_loop_nesting, 2,
            "enhanced-for > while is two loops deep"
        );
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn java_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = method_in_class(
            "src/Foo.java",
            r#"
class Foo {
    int allThree(boolean a, boolean b, boolean c) {
        if (a && b && c) {
            return 1;
        }
        return 0;
    }
}
"#,
            "allThree",
        );
        assert_eq!(and_only.cognitive_score, 2);
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = method_in_class(
            "src/Foo.java",
            r#"
class Foo {
    int mixed(boolean a, boolean b, boolean c) {
        if (a && b || c) {
            return 1;
        }
        return 0;
    }
}
"#,
            "mixed",
        );
        assert_eq!(mixed.cognitive_score, 3);
    }

    #[test]
    fn java_test_annotation_at_the_definition_exempts_the_method() {
        let scored = method_in_class(
            "src/Foo.java",
            r#"
class Foo {
    @Test
    void deeplyNested() {
        boolean a = true, b = true;
        int[] items = {1};
        if (a) {
            for (int item : items) {
                while (b) {
                    if (item > 0) {
                        return;
                    }
                }
            }
        }
    }
}
"#,
            "deeplyNested",
        );

        assert!(scored.is_test, "@Test marks the method as a test");
        assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
        assert!(
            !scored.exceeds_gates(),
            "a test is exempt even at depth 4: {scored:?}"
        );
    }

    #[test]
    fn java_repeated_scoring_never_drifts() {
        let source = r#"
class Foo {
    int pick(boolean a, boolean b) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        }
        return 0;
    }
}
"#;
        let first = cognitive_complexity("src/Foo.java", source).expect("java is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/Foo.java", source).expect("java is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // C
    // -----------------------------------------------------------------

    #[test]
    fn c_switch_scores_once_and_arms_open_no_nesting() {
        let scored = only_function_for(
            "src/lib.c",
            r#"
int classify(int value) {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 1);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.branch_count, 3);
        assert_eq!(
            scored.name, "classify",
            "the name resolves through the declarator chain"
        );
    }

    #[test]
    fn c_if_else_if_else_chain_is_flat() {
        let scored = only_function_for(
            "src/lib.c",
            r#"
int pick(int a, int b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 3);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn c_nested_loops_deepen_the_score() {
        let scored = only_function_for(
            "src/lib.c",
            r#"
int deep(int a, int b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(scored.max_loop_nesting, 2);
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn c_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = only_function_for(
            "src/lib.c",
            r#"
int all_three(int a, int b, int c) {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(and_only.cognitive_score, 2);
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = only_function_for(
            "src/lib.c",
            r#"
int mixed(int a, int b, int c) {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(mixed.cognitive_score, 3);
    }

    #[test]
    fn c_test_attribute_at_the_definition_exempts_the_function() {
        let file = cognitive_complexity(
            "src/lib.c",
            r#"
[[test]]
int deeply_nested_test(int a, int b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        )
        .expect("c is a mapped language");
        let scored = &file.functions[0];

        assert!(scored.is_test, "[[test]] marks the definition as a test");
        assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
        assert!(
            !scored.exceeds_gates(),
            "a test is exempt even at depth 4: {scored:?}"
        );
    }

    #[test]
    fn c_repeated_scoring_never_drifts() {
        let source = r#"
int pick(int a, int b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
        let first = cognitive_complexity("src/lib.c", source).expect("c is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/lib.c", source).expect("c is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // C++
    // -----------------------------------------------------------------

    #[test]
    fn cpp_switch_scores_once_and_arms_open_no_nesting() {
        let scored = only_function_for(
            "src/lib.cpp",
            r#"
int classify(int value) {
    switch (value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 1);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.branch_count, 3);
    }

    #[test]
    fn cpp_if_else_if_else_chain_is_flat() {
        let scored = only_function_for(
            "src/lib.cpp",
            r#"
int pick(bool a, bool b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
        );
        assert_eq!(scored.cognitive_score, 3);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn cpp_nested_loops_deepen_the_score() {
        let scored = only_function_for(
            "src/lib.cpp",
            r#"
int deep(bool a, bool b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(scored.max_loop_nesting, 2);
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn cpp_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = only_function_for(
            "src/lib.cpp",
            r#"
int all_three(bool a, bool b, bool c) {
    if (a && b && c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(and_only.cognitive_score, 2);
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = only_function_for(
            "src/lib.cpp",
            r#"
int mixed(bool a, bool b, bool c) {
    if (a && b || c) {
        return 1;
    }
    return 0;
}
"#,
        );
        assert_eq!(mixed.cognitive_score, 3);
    }

    #[test]
    fn cpp_test_attribute_at_the_definition_exempts_the_function() {
        let file = cognitive_complexity(
            "src/lib.cpp",
            r#"
[[test]]
int deeply_nested_test(bool a, bool b, int *items, int n) {
    if (a) {
        for (int i = 0; i < n; i++) {
            while (b) {
                if (items[i] > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
        )
        .expect("cpp is a mapped language");
        let scored = &file.functions[0];

        assert!(scored.is_test, "[[test]] marks the definition as a test");
        assert_eq!(scored.max_nesting_depth, 4);
        assert!(!scored.exceeds_gates());
    }

    #[test]
    fn cpp_repeated_scoring_never_drifts() {
        let source = r#"
int pick(bool a, bool b) {
    if (a) {
        return 1;
    } else if (b) {
        return 2;
    }
    return 0;
}
"#;
        let first = cognitive_complexity("src/lib.cpp", source).expect("cpp is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/lib.cpp", source).expect("cpp is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // C#
    // -----------------------------------------------------------------

    #[test]
    fn csharp_switch_scores_once_and_arms_open_no_nesting() {
        let scored = method_in_class(
            "src/Foo.cs",
            r#"
class Foo {
    int Classify(int value) {
        switch (value) {
            case 0:
                return 10;
            case 1:
                return 11;
            default:
                return 15;
        }
    }
}
"#,
            "Classify",
        );
        assert_eq!(scored.cognitive_score, 1);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.branch_count, 3);
    }

    #[test]
    fn csharp_if_else_if_else_chain_is_flat() {
        let scored = method_in_class(
            "src/Foo.cs",
            r#"
class Foo {
    int Pick(bool a, bool b) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        } else {
            return 3;
        }
    }
}
"#,
            "Pick",
        );
        assert_eq!(scored.cognitive_score, 3);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn csharp_nested_loops_deepen_the_score() {
        let scored = method_in_class(
            "src/Foo.cs",
            r#"
class Foo {
    int Deep(bool a, bool b, int[] items) {
        if (a) {
            foreach (int item in items) {
                while (b) {
                    if (item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
            "Deep",
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(
            scored.max_loop_nesting, 2,
            "foreach > while is two loops deep"
        );
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn csharp_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = method_in_class(
            "src/Foo.cs",
            r#"
class Foo {
    int AllThree(bool a, bool b, bool c) {
        if (a && b && c) {
            return 1;
        }
        return 0;
    }
}
"#,
            "AllThree",
        );
        assert_eq!(and_only.cognitive_score, 2);
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = method_in_class(
            "src/Foo.cs",
            r#"
class Foo {
    int Mixed(bool a, bool b, bool c) {
        if (a && b || c) {
            return 1;
        }
        return 0;
    }
}
"#,
            "Mixed",
        );
        assert_eq!(mixed.cognitive_score, 3);
    }

    #[test]
    fn csharp_test_attribute_at_the_definition_exempts_the_method() {
        let scored = method_in_class(
            "src/Foo.cs",
            r#"
class Foo {
    [Test]
    void DeeplyNested() {
        bool a = true, b = true;
        int[] items = { 1 };
        if (a) {
            foreach (int item in items) {
                while (b) {
                    if (item > 0) {
                        return;
                    }
                }
            }
        }
    }
}
"#,
            "DeeplyNested",
        );

        assert!(scored.is_test, "[Test] marks the method as a test");
        assert_eq!(scored.max_nesting_depth, 4);
        assert!(!scored.exceeds_gates());
    }

    #[test]
    fn csharp_repeated_scoring_never_drifts() {
        let source = r#"
class Foo {
    int Pick(bool a, bool b) {
        if (a) {
            return 1;
        } else if (b) {
            return 2;
        }
        return 0;
    }
}
"#;
        let first = cognitive_complexity("src/Foo.cs", source).expect("csharp is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/Foo.cs", source).expect("csharp is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // PHP
    // -----------------------------------------------------------------

    #[test]
    fn php_switch_scores_once_and_arms_open_no_nesting() {
        let scored = only_function_for(
            "src/lib.php",
            &php_source(
                r#"
function classify($value) {
    switch ($value) {
        case 0:
            return 10;
        case 1:
            return 11;
        default:
            return 15;
    }
}
"#,
            ),
        );
        assert_eq!(scored.cognitive_score, 1);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.branch_count, 3);
    }

    #[test]
    fn php_if_elseif_else_chain_is_flat() {
        let scored = only_function_for(
            "src/lib.php",
            &php_source(
                r#"
function pick($a, $b) {
    if ($a) {
        return 1;
    } elseif ($b) {
        return 2;
    } else {
        return 3;
    }
}
"#,
            ),
        );
        assert_eq!(scored.cognitive_score, 3);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 1);
    }

    #[test]
    fn php_three_way_elseif_chain_reports_the_longest_link() {
        let scored = only_function_for(
            "src/lib.php",
            &php_source(
                r#"
function pick($a, $b, $c) {
    if ($a) {
        return 1;
    } elseif ($b) {
        return 2;
    } elseif ($c) {
        return 3;
    } else {
        return 4;
    }
}
"#,
            ),
        );
        assert_eq!(scored.cognitive_score, 4);
        assert_eq!(scored.max_nesting_depth, 1);
        assert_eq!(scored.max_else_if_chain, 2, "two elseif links");
    }

    #[test]
    fn php_nested_loops_deepen_the_score() {
        let scored = only_function_for(
            "src/lib.php",
            &php_source(
                r#"
function deep($a, $b, $items) {
    if ($a) {
        foreach ($items as $item) {
            while ($b) {
                if ($item > 0) {
                    return 1;
                }
            }
        }
    }
    return 0;
}
"#,
            ),
        );
        assert_eq!(scored.cognitive_score, 10);
        assert_eq!(scored.max_nesting_depth, 4);
        assert_eq!(
            scored.max_loop_nesting, 2,
            "foreach > while is two loops deep"
        );
        assert!(scored.exceeds_gates());
    }

    #[test]
    fn php_boolean_run_scores_once_mixed_run_scores_twice() {
        let and_only = only_function_for(
            "src/lib.php",
            &php_source(
                r#"
function all_three($a, $b, $c) {
    if ($a && $b && $c) {
        return 1;
    }
    return 0;
}
"#,
            ),
        );
        assert_eq!(and_only.cognitive_score, 2);
        assert_eq!(and_only.max_boolean_operands, 3);

        let mixed = only_function_for(
            "src/lib.php",
            &php_source(
                r#"
function mixed($a, $b, $c) {
    if ($a && $b || $c) {
        return 1;
    }
    return 0;
}
"#,
            ),
        );
        assert_eq!(mixed.cognitive_score, 3);
    }

    #[test]
    fn php_test_attribute_at_the_definition_exempts_the_method() {
        let file = cognitive_complexity(
            "src/FooTest.php",
            r#"<?php
class FooTest {
    #[Test]
    public function deeplyNested($a, $b, $items) {
        if ($a) {
            foreach ($items as $item) {
                while ($b) {
                    if ($item > 0) {
                        return 1;
                    }
                }
            }
        }
        return 0;
    }
}
"#,
        )
        .expect("php is a mapped language");
        let scored = &file.functions[0];

        assert!(scored.is_test, "#[Test] marks the method as a test");
        assert_eq!(scored.max_nesting_depth, 4, "the depth is still measured");
        assert!(
            !scored.exceeds_gates(),
            "a test is exempt even at depth 4: {scored:?}"
        );
    }

    #[test]
    fn php_repeated_scoring_never_drifts() {
        let source = "<?php\nfunction pick($a, $b) {\n    if ($a) {\n        return 1;\n    } elseif ($b) {\n        return 2;\n    }\n    return 0;\n}\n";
        let first = cognitive_complexity("src/lib.php", source).expect("php is mapped");
        for run in 1..DETERMINISM_RUNS {
            let again = cognitive_complexity("src/lib.php", source).expect("php is mapped");
            assert_eq!(again, first, "run {run} drifted from run 0");
        }
    }

    // -----------------------------------------------------------------
    // Pathological depth — the traversal-depth cap
    // -----------------------------------------------------------------

    /// A source string with `depth` levels of `if true { ... }` nested inside
    /// one function body — far beyond any plausible real function, and far
    /// beyond [`MAX_TRAVERSAL_DEPTH`].
    fn deeply_nested_if_source(depth: usize) -> String {
        let mut source = String::from("fn deep() -> u32 {\n");
        source.push_str(&"if true {\n".repeat(depth));
        source.push_str("1\n");
        source.push_str(&"}\n".repeat(depth));
        source.push_str("}\n");
        source
    }

    #[test]
    fn pathological_nesting_does_not_crash_and_is_reported_as_partial() {
        // Thousands of levels — two orders of magnitude past MAX_TRAVERSAL_DEPTH,
        // and far past any plausible real function. Reaching this assertion at
        // all (rather than a stack-overflow abort) is the point of the test.
        let source = deeply_nested_if_source(5_000);

        let file = cognitive_complexity("src/lib.rs", &source).expect("rust is a mapped language");
        assert_eq!(
            file.functions.len(),
            1,
            "the outer function itself is still found and scored, got {:?}",
            file.functions
        );
        let scored = &file.functions[0];

        assert!(
            scored.is_partial,
            "nesting past the traversal cap must report a partial result, never a fabricated \
             score: {scored:?}"
        );
        assert!(
            scored.exceeds_gates(),
            "a partial result must never read as \"under the gates\": {scored:?}"
        );
    }

    #[test]
    fn nesting_well_under_the_traversal_cap_is_never_marked_partial() {
        // A depth (well) below MAX_TRAVERSAL_DEPTH but far above every other
        // fixture in this suite, to pin the cap does not clip ordinary — if
        // unusually deep — real code.
        let source = deeply_nested_if_source(32);

        let file = cognitive_complexity("src/lib.rs", &source).expect("rust is a mapped language");
        let scored = &file.functions[0];

        assert!(
            !scored.is_partial,
            "32 levels of nesting sits far below the traversal cap: {scored:?}"
        );
        assert_eq!(scored.max_nesting_depth, 32);
    }

    #[test]
    fn pinned_small_fixtures_are_unaffected_by_the_traversal_cap() {
        // Every depth 1-4 fixture already pinned above must produce the exact
        // same numbers with the cap in place — the cap changes nothing for any
        // plausible real function.
        let collect_line_tags = only_function(COLLECT_LINE_TAGS);
        assert!(!collect_line_tags.is_partial);
        assert_eq!(collect_line_tags.cognitive_score, 5);
        assert_eq!(collect_line_tags.max_nesting_depth, 2);

        let edit_line_markers = only_function(EDIT_LINE_MARKERS);
        assert!(!edit_line_markers.is_partial);
        assert_eq!(edit_line_markers.max_nesting_depth, 3);

        let deep = only_function(
            r#"
fn deep(a: bool, b: bool, items: &[u8]) -> u8 {
    if a {
        for item in items {
            while b {
                if *item > 0 {
                    return 1;
                }
            }
        }
    }
    0
}
"#,
        );
        assert!(!deep.is_partial);
        assert_eq!(deep.cognitive_score, 10);
        assert_eq!(deep.max_nesting_depth, 4);
    }
}
