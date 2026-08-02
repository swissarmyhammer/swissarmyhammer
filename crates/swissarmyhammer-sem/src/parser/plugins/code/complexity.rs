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

use tree_sitter::Node;

use super::languages::{dotted_lowercase_extension, get_language_config};

/// The cognitive-complexity score at or above which a function is flagged. The
/// Sonar default.
pub const COGNITIVE_COMPLEXITY_THRESHOLD: u32 = 15;

/// The condition-nesting depth at or above which a function is flagged. The
/// depth `cognitive-complexity.md` already stated: "conditions nested more than
/// 3 levels deep (4+ is a flag)".
pub const NESTING_DEPTH_THRESHOLD: u32 = 4;

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
    name_field: &'static str,
    /// Kinds that add `1 + nesting` and open a nesting level for their body.
    nesting_kinds: &'static [&'static str],
    /// The subset of [`Self::nesting_kinds`] that are conditionals, so an
    /// `else if` is recognized inside an else clause.
    conditional_kinds: &'static [&'static str],
    /// The subset of [`Self::nesting_kinds`] that are loops, counted separately
    /// for [`FunctionComplexity::max_loop_nesting`].
    loop_kinds: &'static [&'static str],
    /// The else-clause kind. It adds a flat +1 and opens no nesting level; an
    /// `else if` inside it is flattened onto the same level.
    else_kinds: &'static [&'static str],
    /// Arm/case kinds. Transparent: no increment, **no nesting level**. This is
    /// the row that keeps a `match` arm from reading as a nested condition.
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
    /// The node kind of an attribute that can mark a function as a test.
    attribute_kinds: &'static [&'static str],
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
    loop_kinds: &["for_expression", "while_expression", "loop_expression"],
    else_kinds: &["else_clause"],
    arm_kinds: &["match_arm"],
    nest_only_kinds: &["closure_expression"],
    logical_kinds: &["binary_expression"],
    logical_operators: &["&&", "||"],
    labelled_jump_kinds: &["break_expression", "continue_expression"],
    label_kinds: &["label"],
    attribute_kinds: &["attribute_item"],
};

/// Every language with a scorer mapping. A language absent here is "not
/// computed", never zero.
static ALL_SPECS: &[&ComplexitySpec] = &[&RUST_SPEC];

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
}

impl FunctionComplexity {
    /// Whether this function trips either gate.
    ///
    /// A test function never trips one: the validator's rule exempts tests
    /// whose complexity is sequential assertions, and that exemption is now
    /// computed from the definition rather than recalled by the model.
    pub fn exceeds_gates(&self) -> bool {
        !self.is_test
            && (self.cognitive_score >= COGNITIVE_COMPLEXITY_THRESHOLD
                || self.max_nesting_depth >= NESTING_DEPTH_THRESHOLD)
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
    collect_functions(tree.root_node(), source, spec, &mut functions);
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
    out: &mut Vec<FunctionComplexity>,
) {
    if spec.function_kinds.contains(&node.kind()) {
        out.push(score_function(node, source, spec));
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_functions(child, source, spec, out);
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
}

/// Score one function node.
fn score_function(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> FunctionComplexity {
    let mut tally = Tally::default();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, source, spec, 0, 0, &mut tally);
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
    }
}

/// The function's declared name, or `<anonymous>`.
fn function_name(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> String {
    node.child_by_field_name(spec.name_field)
        .and_then(|n| node_text(n, source))
        .unwrap_or("<anonymous>")
        .to_string()
}

/// The source text a node spans, when the byte range is valid UTF-8 boundaries.
fn node_text<'s>(node: Node<'_>, source: &'s str) -> Option<&'s str> {
    source.get(node.start_byte()..node.end_byte())
}

/// Whether the definition is marked as a test by an attribute at the definition.
///
/// Rust marks tests with an attribute immediately above the item (`#[test]`,
/// `#[tokio::test]`). The attribute's last path segment must be exactly `test`,
/// so `#[serial_test::serial]` and `#[test_case(..)]` are not tests. The file
/// name is never consulted.
fn is_test_definition(node: Node<'_>, source: &str, spec: &ComplexitySpec) -> bool {
    let mut sibling = node.prev_named_sibling();
    while let Some(current) = sibling {
        if !spec.attribute_kinds.contains(&current.kind()) {
            return false;
        }
        if attribute_marks_test(current, source) {
            return true;
        }
        sibling = current.prev_named_sibling();
    }
    false
}

/// Whether one attribute node names the `test` marker.
fn attribute_marks_test(node: Node<'_>, source: &str) -> bool {
    let Some(text) = node_text(node, source) else {
        return false;
    };
    let inner = text
        .trim()
        .trim_start_matches("#[")
        .trim_end_matches(']')
        .trim();
    let path = inner.split('(').next().unwrap_or(inner).trim();
    path.rsplit("::").next().is_some_and(|last| last == "test")
}

/// Walk one node, accumulating into `tally`.
///
/// `nesting` is the count of enclosing constructs that opened a nesting level.
/// `loop_nesting` is the same count restricted to loops.
fn walk(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    tally: &mut Tally,
) {
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
            walk_conditional(node, source, spec, nesting, inner_loop_nesting, tally, 1);
        } else {
            walk_children(node, source, spec, nesting + 1, inner_loop_nesting, tally);
        }
        return;
    }

    if spec.arm_kinds.contains(&kind) {
        // Transparent: an arm is a branch of one decision, not a nested
        // decision. No increment and no nesting level.
        tally.branch_count += 1;
        walk_children(node, source, spec, nesting, loop_nesting, tally);
        return;
    }

    if spec.nest_only_kinds.contains(&kind) {
        walk_children(node, source, spec, nesting + 1, loop_nesting, tally);
        return;
    }

    if spec.labelled_jump_kinds.contains(&kind) && carries_label(node, spec) {
        tally.flat_increment();
        return;
    }

    if is_boolean_root(node, spec) {
        walk_boolean(node, source, spec, nesting, loop_nesting, tally);
        return;
    }

    walk_children(node, source, spec, nesting, loop_nesting, tally);
}

/// Walk a node's named children at the given levels.
fn walk_children(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    tally: &mut Tally,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, source, spec, nesting, loop_nesting, tally);
    }
}

/// Walk a conditional's children: its body one level deeper, and its else
/// clause at the conditional's OWN level.
///
/// The else clause is the one child that must not inherit the level the
/// conditional opened. Sonar charges `else` a flat +1 at the level of the `if`
/// it belongs to, and an `if`/`else if`/`else` chain is one decision with
/// several branches, not a staircase.
///
/// `chain` is the 1-based position in the `else if` chain, so
/// [`FunctionComplexity::max_else_if_chain`] can record the longest one.
#[allow(clippy::too_many_arguments)]
fn walk_conditional(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    tally: &mut Tally,
    chain: u32,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if spec.else_kinds.contains(&child.kind()) {
            walk_else(child, source, spec, nesting, loop_nesting, tally, chain);
            continue;
        }
        walk(child, source, spec, nesting + 1, loop_nesting, tally);
    }
}

/// Walk an else clause: a flat +1, with an `else if` flattened onto the same
/// level and its own else clause continuing the chain.
#[allow(clippy::too_many_arguments)]
fn walk_else(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    tally: &mut Tally,
    chain: u32,
) {
    tally.flat_increment();
    tally.branch_count += 1;

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if spec.conditional_kinds.contains(&child.kind()) {
            // `else if`: the flat +1 above is its whole increment, and it opens
            // no extra level — it is a sibling branch, not a nested condition.
            tally.max_else_if_chain = tally.max_else_if_chain.max(chain);
            tally.max_nesting_depth = tally.max_nesting_depth.max(nesting + 1);
            walk_conditional(child, source, spec, nesting, loop_nesting, tally, chain + 1);
            continue;
        }
        walk(child, source, spec, nesting + 1, loop_nesting, tally);
    }
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
        tally,
        None,
        &mut sequences,
        &mut operators,
    );
    tally.cognitive_score += sequences;
    tally.max_boolean_operands = tally.max_boolean_operands.max(operators + 1);
}

/// Recurse a boolean chain, counting operator-run changes and walking every
/// non-boolean operand normally.
#[allow(clippy::too_many_arguments)]
fn boolean_chain(
    node: Node<'_>,
    source: &str,
    spec: &ComplexitySpec,
    nesting: u32,
    loop_nesting: u32,
    tally: &mut Tally,
    parent_operator: Option<&str>,
    sequences: &mut u32,
    operators: &mut u32,
) {
    let Some(operator) = logical_operator(node, spec) else {
        // A plain operand: it may still hold conditions of its own.
        walk(node, source, spec, nesting, loop_nesting, tally);
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
            cognitive_complexity("src/app.py", "def f():\n    pass\n").is_none(),
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
}
