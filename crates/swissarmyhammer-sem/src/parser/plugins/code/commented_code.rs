//! Decide whether a comment block is commented-out code, by re-parsing it.
//!
//! The verdict is objective. A comment block is stripped of its markers and
//! handed back to the grammar the file itself is parsed with. Text that parses
//! as several statements or items, with almost no error nodes, IS code. Text
//! that does not, is prose. No model reads either one.
//!
//! Three gates decide, and every one of them is a measurement:
//!
//! 1. the block spans more than [`MAX_TOLERATED_BLOCK_LINES`] lines,
//! 2. the re-parse yields at least [`MIN_REPARSED_ITEMS`] statements or items,
//! 3. error nodes cover at most [`MAX_ERROR_BYTE_RATIO`] of the re-parsed text.
//!
//! Documentation is excluded before any gate runs, and the exclusion is
//! structural. Where the grammar gives a doc comment a node of its own — Rust's
//! `outer_doc_comment_marker` and `inner_doc_comment_marker` — that node kind is
//! the test. Where it does not, the test is the comment's own opening delimiter
//! (`///`, `/**`, `//!`), which is a token of the language and not a reading of
//! the prose inside it.
//!
//! [`COMMENT_SPECS`] is the roster this module adds to the grammar table next
//! door: one row for each language whose re-parse verdict was measured, and no
//! row for a language whose grammar accepts English as code.

use tree_sitter::Node;

use super::parse_code;

/// The most lines a comment block may span and still be left alone.
///
/// The `no-commented-code` prompt rule this module supersedes says "more than 5
/// lines of code that are commented out", so a block of six lines or more is
/// the first thing that can be a finding.
const MAX_TOLERATED_BLOCK_LINES: usize = 5;

/// The fewest statements or items a re-parse must yield to read as code.
///
/// One statement is what a single stray line of prose parses to in a permissive
/// grammar. Two is the smallest count that says the text has structure.
const MIN_REPARSED_ITEMS: usize = 2;

/// The most of a re-parsed block that tree-sitter may cover with error nodes.
///
/// Chosen from a measured gap, not guessed. Over this workspace and four
/// external repositories — `psf/requests`, `axios/axios`, `BurntSushi/ripgrep`
/// and `gohugoio/hugo`, 2926 files in all — 1949 comment blocks clear the line
/// gate, and every one under 0.31 was read by hand. Three populations came out
/// of that reading:
///
/// | What the block is | Highest ratio | Lowest ratio |
/// |---|---|---|
/// | commented-out code | 0.035 | 0.000 |
/// | standardized metadata | 0.137 | 0.110 |
/// | prose | 0.999 | 0.173 |
///
/// The binding pair is 0.035 and 0.110: the highest ratio among real
/// commented-out code (hugo's `htmltemplate/exec_test.go`, six disabled calls
/// each carrying a `// TODO` tail) and the lowest among the PEP 723 inline
/// script metadata this workspace's own `crates/ane-embedding/convert` scripts
/// carry, whose `# ///` fences are the only part that fails to parse. The gate
/// sits inside that gap, twice the code figure and two thirds of the metadata
/// one, so a block with one truncated line still reads as code while a fenced
/// metadata header and a paragraph of English never do.
const MAX_ERROR_BYTE_RATIO: f64 = 0.07;

/// A block of commented-out code, ready to be reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentedCodeBlock {
    /// The one-based line the block's first comment starts on.
    pub line: usize,
    /// How many lines the block spans.
    pub lines: usize,
    /// The language the block re-parsed as, as the grammar roster names it.
    pub language: &'static str,
}

/// How one language spells its comments, and what makes one documentation.
///
/// The roster next door ([`super::languages`]) routes a file to its grammar;
/// this table says what a comment looks like once that grammar has parsed it.
/// A language with no row here gets no verdict at all — see
/// [`commented_code_blocks`].
#[derive(Debug)]
struct CommentSpec {
    /// The language id, as [`super::ParsedCode::language`] reports it.
    language: &'static str,
    /// The node kinds this grammar gives a comment.
    comment_kinds: &'static [&'static str],
    /// Child node kinds that make the comment above them documentation.
    ///
    /// Rust is the one grammar in the roster that marks a doc comment with a
    /// node of its own. Every other language leaves the delimiter in the
    /// comment's text, which is what [`exempt_openers`](Self::exempt_openers) reads.
    doc_marker_kinds: &'static [&'static str],
    /// Opening delimiters that mark a comment as something other than
    /// disabled code: a documentation delimiter, or a standardized metadata
    /// delimiter the language's own tooling reads.
    ///
    /// Matched against the comment's own first characters, so this reads a
    /// token of the language and never the prose the comment carries.
    exempt_openers: &'static [&'static str],
    /// Line-comment openers, longest first so `///` is tried before `//`.
    line_openers: &'static [&'static str],
    /// Block-comment delimiter pairs, longest opener first.
    block_delimiters: &'static [(&'static str, &'static str)],
}

/// The C-family comment shape: `//` lines and `/* */` blocks, with `///` and
/// `/**` reserved for documentation.
const C_FAMILY_DOC_OPENERS: &[&str] = &["///", "/**", "//!", "/*!"];

/// The C-family line-comment openers, longest first.
const C_FAMILY_LINE_OPENERS: &[&str] = &["//"];

/// The C-family block-comment delimiters.
const C_FAMILY_BLOCK_DELIMITERS: &[(&str, &str)] = &[("/*", "*/")];

/// No grammar in the roster but Rust marks a doc comment with its own node.
const NO_DOC_MARKER_KINDS: &[&str] = &[];

/// The kinds a grammar that spells every comment `comment` uses.
const PLAIN_COMMENT_KINDS: &[&str] = &["comment"];

/// Every language whose comment blocks get a re-parse verdict.
///
/// A language earns a row by measurement, and three of the roster's sixteen
/// have none. `bash`, `ruby` and `elixir` accept a paren-less call, so a line
/// of English parses as a command with arguments and a paragraph of prose
/// re-parses as clean code — the gates cannot separate the two populations
/// there. Those three keep the `no-commented-code` prompt rule. `fortran` has
/// no row for the opposite reason: its `!` comments carry no delimiter
/// convention that separates documentation from a disabled line.
static COMMENT_SPECS: &[&CommentSpec] = &[
    &RUST_COMMENT_SPEC,
    &PYTHON_COMMENT_SPEC,
    &TYPESCRIPT_COMMENT_SPEC,
    &TSX_COMMENT_SPEC,
    &JAVASCRIPT_COMMENT_SPEC,
    &GO_COMMENT_SPEC,
    &JAVA_COMMENT_SPEC,
    &C_COMMENT_SPEC,
    &CPP_COMMENT_SPEC,
    &CSHARP_COMMENT_SPEC,
    &SWIFT_COMMENT_SPEC,
];

static RUST_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "rust",
    comment_kinds: &["line_comment", "block_comment"],
    doc_marker_kinds: &["outer_doc_comment_marker", "inner_doc_comment_marker"],
    exempt_openers: C_FAMILY_DOC_OPENERS,
    line_openers: C_FAMILY_LINE_OPENERS,
    block_delimiters: C_FAMILY_BLOCK_DELIMITERS,
};

static PYTHON_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "python",
    comment_kinds: PLAIN_COMMENT_KINDS,
    doc_marker_kinds: NO_DOC_MARKER_KINDS,
    // A Python docstring is a string expression and never a comment node, so
    // the grammar excludes documentation before this table is consulted, and
    // PEP 723 metadata is excluded by its own unparseable `# ///` fence — see
    // `MAX_ERROR_BYTE_RATIO`.
    exempt_openers: &[],
    line_openers: &["#"],
    block_delimiters: &[],
};

static TYPESCRIPT_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "typescript",
    comment_kinds: PLAIN_COMMENT_KINDS,
    doc_marker_kinds: NO_DOC_MARKER_KINDS,
    exempt_openers: C_FAMILY_DOC_OPENERS,
    line_openers: C_FAMILY_LINE_OPENERS,
    block_delimiters: C_FAMILY_BLOCK_DELIMITERS,
};

static TSX_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "tsx",
    ..TYPESCRIPT_COMMENT_SPEC
};

static JAVASCRIPT_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "javascript",
    ..TYPESCRIPT_COMMENT_SPEC
};

static GO_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "go",
    ..TYPESCRIPT_COMMENT_SPEC
};

static C_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "c",
    ..TYPESCRIPT_COMMENT_SPEC
};

static CPP_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "cpp",
    ..TYPESCRIPT_COMMENT_SPEC
};

static CSHARP_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "csharp",
    ..TYPESCRIPT_COMMENT_SPEC
};

static JAVA_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "java",
    comment_kinds: &["line_comment", "block_comment"],
    doc_marker_kinds: NO_DOC_MARKER_KINDS,
    exempt_openers: C_FAMILY_DOC_OPENERS,
    line_openers: C_FAMILY_LINE_OPENERS,
    block_delimiters: C_FAMILY_BLOCK_DELIMITERS,
};

static SWIFT_COMMENT_SPEC: CommentSpec = CommentSpec {
    language: "swift",
    comment_kinds: &["comment", "multiline_comment"],
    doc_marker_kinds: NO_DOC_MARKER_KINDS,
    exempt_openers: C_FAMILY_DOC_OPENERS,
    line_openers: C_FAMILY_LINE_OPENERS,
    block_delimiters: C_FAMILY_BLOCK_DELIMITERS,
};

/// The comment spec for a language id, `None` when the language has no row.
fn spec_for_language(language: &str) -> Option<&'static CommentSpec> {
    COMMENT_SPECS
        .iter()
        .copied()
        .find(|spec| spec.language == language)
}

/// Every file extension a comment-block verdict covers, dotted and lowercase.
///
/// The extensions of the languages [`COMMENT_SPECS`] holds a row for, in roster
/// order. A caller that routes a path to the verdict reads this list to learn
/// which paths the verdict can answer for.
pub fn commented_code_extensions() -> Vec<&'static str> {
    COMMENT_SPECS
        .iter()
        .filter_map(|spec| super::languages::extensions_for_language(spec.language))
        .flat_map(|extensions| extensions.iter().copied())
        .collect()
}

/// Every block of commented-out code in `source`.
///
/// Returns `None` — meaning **not measured** — when the path routes to no
/// grammar, when the parse fails, or when the language has no row in
/// [`COMMENT_SPECS`]. A caller must report "not computed" for `None` and never
/// substitute an empty list, which would read as "this file hides no code in
/// its comments".
pub fn commented_code_blocks(path: &str, source: &str) -> Option<Vec<CommentedCodeBlock>> {
    let parsed = parse_code(path, source)?;
    let spec = spec_for_language(parsed.language())?;

    let mut comments = Vec::new();
    collect_comments(parsed.tree().root_node(), spec, &mut comments);
    comments.retain(|comment| starts_its_own_line(*comment, source));

    let blocks = group_adjacent(&comments, source)
        .into_iter()
        .filter(|block| {
            !block
                .iter()
                .any(|node| is_exempt_comment(*node, source, spec))
        })
        .filter_map(|block| verdict(&block, path, source, spec))
        .collect();
    Some(blocks)
}

/// Collect every comment node under `node`, in document order.
fn collect_comments<'t>(node: Node<'t>, spec: &CommentSpec, out: &mut Vec<Node<'t>>) {
    if spec.comment_kinds.contains(&node.kind()) {
        out.push(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_comments(child, spec, out);
    }
}

/// Whether nothing but whitespace precedes this comment on its own line.
///
/// A comment sitting after code is annotating that line, not disabling it.
/// Measured on `gohugoio/hugo`: `gofmt` aligns a run of trailing comments into
/// one column, so a column test cannot tell them from a block — six aligned
/// `// true || false && ...` annotations re-parsed as six clean Go statements.
/// What separates them is the live code to their left.
fn starts_its_own_line(comment: Node<'_>, source: &str) -> bool {
    let start = comment.start_byte();
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    source[line_start..start].trim().is_empty()
}

/// The row the comment's last character sits on.
///
/// A grammar may put the terminating newline INSIDE the comment node —
/// tree-sitter-rust does it for `///` and `//!` but not for a plain `//` — so
/// `end_position().row` can already be the next line. Read that way, a module
/// doc comment reaches across the blank line below it and swallows the block
/// beneath, and the whole block is then exempted as documentation.
fn end_row(comment: Node<'_>, source: &str) -> usize {
    let text = &source[comment.start_byte()..comment.end_byte()];
    match text.ends_with('\n') {
        true => comment.end_position().row.saturating_sub(1),
        false => comment.end_position().row,
    }
}

/// Split comment nodes into blocks of vertically adjacent comments.
///
/// Two comments join one block when the second starts on the line after the
/// first ends.
fn group_adjacent<'t>(comments: &[Node<'t>], source: &str) -> Vec<Vec<Node<'t>>> {
    let mut blocks: Vec<Vec<Node<'t>>> = Vec::new();
    for comment in comments {
        let joins_previous = blocks
            .last()
            .and_then(|block| block.last())
            .is_some_and(|previous| end_row(*previous, source) + 1 == comment.start_position().row);
        match joins_previous {
            true => blocks
                .last_mut()
                .expect("joins_previous is only true when a block exists")
                .push(*comment),
            false => blocks.push(vec![*comment]),
        }
    }
    blocks
}

/// Whether this comment is exempt: documentation, or standardized metadata.
///
/// Two structural tests, in order: a child node the grammar marks as a doc
/// marker, then the comment's own opening delimiter. Neither reads the prose.
fn is_exempt_comment(comment: Node<'_>, source: &str, spec: &CommentSpec) -> bool {
    let mut cursor = comment.walk();
    if comment
        .children(&mut cursor)
        .any(|child| spec.doc_marker_kinds.contains(&child.kind()))
    {
        return true;
    }
    let text = &source[comment.start_byte()..comment.end_byte()];
    spec.exempt_openers
        .iter()
        .any(|opener| text.starts_with(opener))
}

/// The verdict for one comment block, `None` when any gate rejects it.
fn verdict(
    block: &[Node<'_>],
    path: &str,
    source: &str,
    spec: &CommentSpec,
) -> Option<CommentedCodeBlock> {
    let first = block.first()?;
    let last = block.last()?;
    let lines = end_row(*last, source) - first.start_position().row + 1;
    if lines <= MAX_TOLERATED_BLOCK_LINES {
        return None;
    }

    let candidate = block
        .iter()
        .map(|comment| strip_markers(&source[comment.start_byte()..comment.end_byte()], spec))
        .collect::<Vec<String>>()
        .join("\n");

    parses_as_code(path, &candidate).then_some(CommentedCodeBlock {
        line: first.start_position().row + 1,
        lines,
        language: spec.language,
    })
}

/// Strip a comment's delimiters, leaving the text it wrapped.
///
/// Every line keeps the indentation that follows its delimiter, because the
/// indentation is the code's own and Python needs it back exactly.
fn strip_markers(text: &str, spec: &CommentSpec) -> String {
    for (opener, closer) in spec.block_delimiters {
        if let Some(inner) = text.strip_prefix(opener) {
            let inner = inner.strip_suffix(closer).unwrap_or(inner);
            return strip_block_continuations(inner);
        }
    }
    strip_line_opener(text, spec)
}

/// Strip the line-comment opener, keeping everything after it verbatim.
///
/// The space a writer conventionally puts after the opener is kept, because
/// every line of a block carries it and a uniform indent changes no grammar's
/// verdict. Keeping it also keeps the strip reversible: what comes back is the
/// comment's own text, shifted, never re-spaced.
fn strip_line_opener(line: &str, spec: &CommentSpec) -> String {
    let trimmed = line.trim_start();
    let indent_width = line.len() - trimmed.len();
    for opener in spec.line_openers {
        if let Some(rest) = trimmed.strip_prefix(opener) {
            return format!("{}{}", &line[..indent_width], rest);
        }
    }
    line.to_string()
}

/// Strip the leading `*` a C-family block comment puts on each interior line.
///
/// Applied only when EVERY non-empty interior line carries one, so a block
/// comment written without the convention keeps its text unchanged.
fn strip_block_continuations(inner: &str) -> String {
    let non_empty = || inner.lines().filter(|line| !line.trim().is_empty());
    let all_starred =
        non_empty().count() > 0 && non_empty().all(|line| line.trim_start().starts_with('*'));
    if !all_starred {
        return inner.to_string();
    }
    inner
        .lines()
        .map(|line| match line.trim_start().strip_prefix('*') {
            Some(rest) => rest.to_string(),
            None => line.to_string(),
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Whether `candidate` re-parses as code in this file's own language.
fn parses_as_code(path: &str, candidate: &str) -> bool {
    let Some(reparsed) = parse_code(path, candidate) else {
        return false;
    };
    let root = reparsed.tree().root_node();
    if error_byte_ratio(root, candidate.len()) > MAX_ERROR_BYTE_RATIO {
        return false;
    }
    item_count(root) >= MIN_REPARSED_ITEMS
}

/// The share of `total_bytes` that tree-sitter covered with error nodes.
///
/// An error node's children are not walked: the whole span is already unparsed,
/// and descending would count the same bytes twice.
fn error_byte_ratio(root: Node<'_>, total_bytes: usize) -> f64 {
    if total_bytes == 0 {
        return 1.0;
    }
    error_bytes(root) as f64 / total_bytes as f64
}

/// The bytes the top-most error nodes under `node` cover.
fn error_bytes(node: Node<'_>) -> usize {
    if node.is_error() {
        return node.end_byte() - node.start_byte();
    }
    let mut cursor = node.walk();
    node.children(&mut cursor).map(error_bytes).sum()
}

/// The node-kind endings every grammar in the roster gives a statement or a
/// declared item.
///
/// Tree-sitter grammars name these nodes to a shared convention —
/// `expression_statement`, `lexical_declaration`, `function_definition`,
/// `function_item`, `struct_specifier` — so one suffix list reads all eleven
/// languages without a per-language table. A `declaration` node with no prefix
/// (C and C++ spell it that way) matches the `declaration` ending too.
const ITEM_KIND_ENDINGS: &[&str] = &[
    "statement",
    "declaration",
    "definition",
    "item",
    "specifier",
];

/// The node kinds that carry a name and nothing else.
///
/// A permissive grammar wraps a bare word in a statement — tree-sitter-go reads
/// the word `heading` as `(expression_statement (identifier))` — so a list of
/// words in a comment re-parses as a run of clean statements. A statement built
/// from these kinds alone is a word, not code.
const NAME_ONLY_KINDS: &[&str] = &[
    "identifier",
    "label_name",
    "type_identifier",
    "field_identifier",
    "property_identifier",
    "dotted_name",
];

/// The statements and items a re-parse yielded, at any depth.
///
/// Counted at any depth rather than at the root, because a commented-out
/// function is ONE item at the root and everything that makes it code sits
/// inside its body. A statement holding nothing but a name does not count —
/// see [`NAME_ONLY_KINDS`].
fn item_count(node: Node<'_>) -> usize {
    if node.is_error() {
        return 0;
    }
    let mut cursor = node.walk();
    let below: usize = node.named_children(&mut cursor).map(item_count).sum();
    let counts = is_item_kind(node.kind()) && has_substance(node);
    below + usize::from(counts)
}

/// Whether a node holds anything beyond statement wrappers and bare names.
fn has_substance(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    let substantial = node.named_children(&mut cursor).any(|child| {
        let kind = child.kind();
        if NAME_ONLY_KINDS.contains(&kind) {
            return false;
        }
        if is_item_kind(kind) {
            return has_substance(child);
        }
        true
    });
    substantial
}

/// Whether a node kind names a statement or a declared item.
fn is_item_kind(kind: &str) -> bool {
    ITEM_KIND_ENDINGS
        .iter()
        .any(|ending| kind.ends_with(ending))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One language's samples: a block that IS commented-out code, and three
    /// that are not.
    ///
    /// Every row carries all four, so a change that stops reporting a language
    /// and a change that starts over-reporting one both fail here.
    struct LanguageSamples {
        /// A file name that routes to this language's grammar.
        path: &'static str,
        /// A commented-out function of six lines or more. Exactly one finding.
        commented_out_function: &'static str,
        /// A documentation comment carrying a code example. No finding.
        documented_example: &'static str,
        /// Six lines of prose about the code. No finding.
        prose: &'static str,
        /// Two lines of genuinely commented-out code. No finding.
        short_snippet: &'static str,
    }

    /// Every language [`COMMENT_SPECS`] holds a row for, with its samples.
    static LANGUAGE_SAMPLES: &[LanguageSamples] = &[
        LanguageSamples {
            path: "probe.rs",
            commented_out_function: concat!(
                "// fn disabled(limit: i32) -> i32 {\n",
                "//     let mut total = 0;\n",
                "//     for value in 0..limit {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     total\n",
                "// }\n",
                "pub fn live() {}\n",
            ),
            documented_example: concat!(
                "/// Adds the readings.\n",
                "///\n",
                "/// ```\n",
                "/// let mut total = 0;\n",
                "/// for value in 0..4 {\n",
                "///     total += value;\n",
                "/// }\n",
                "/// assert_eq!(total, 6);\n",
                "/// ```\n",
                "pub fn live() {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "pub fn live() {}\n",
            ),
            short_snippet: concat!(
                "// const TOTAL: i32 = 0;\n",
                "// const BAND: i32 = 1;\n",
                "pub fn live() {}\n",
            ),
        },
        LanguageSamples {
            path: "probe.py",
            commented_out_function: concat!(
                "# def disabled(limit):\n",
                "#     total = 0\n",
                "#     for value in range(limit):\n",
                "#         total += value\n",
                "#     return total\n",
                "#\n",
                "def live():\n",
                "    return 1\n",
            ),
            documented_example: concat!(
                "def live():\n",
                "    \"\"\"Add the readings.\n",
                "\n",
                "    total = 0\n",
                "    for value in range(4):\n",
                "        total += value\n",
                "    return total\n",
                "    \"\"\"\n",
                "    return 1\n",
            ),
            prose: concat!(
                "# The reading loop below walks the grid once.\n",
                "# It keeps the running band in a local, because the\n",
                "# caller has no use for the partial sums.\n",
                "# TODO: the band should come from configuration once\n",
                "# the settings file grows a section for it.\n",
                "# See the design note for why the order matters.\n",
                "def live():\n",
                "    return 1\n",
            ),
            short_snippet: concat!(
                "# total = 0\n",
                "# band = 1\n",
                "def live():\n",
                "    return 1\n"
            ),
        },
        LanguageSamples {
            path: "probe.ts",
            commented_out_function: concat!(
                "// function disabled(limit: number): number {\n",
                "//     let total = 0;\n",
                "//     for (let value = 0; value < limit; value++) {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     return total;\n",
                "// }\n",
                "export function live(): void {}\n",
            ),
            documented_example: concat!(
                "/**\n",
                " * Adds the readings.\n",
                " *\n",
                " * let total = 0;\n",
                " * for (let value = 0; value < 4; value++) {\n",
                " *     total += value;\n",
                " * }\n",
                " * return total;\n",
                " */\n",
                "export function live(): void {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "export function live(): void {}\n",
            ),
            short_snippet: concat!(
                "// let total = 0;\n",
                "// let band = 1;\n",
                "export function live(): void {}\n",
            ),
        },
        LanguageSamples {
            path: "probe.tsx",
            commented_out_function: concat!(
                "// function disabled(limit: number): number {\n",
                "//     let total = 0;\n",
                "//     for (let value = 0; value < limit; value++) {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     return total;\n",
                "// }\n",
                "export function live(): void {}\n",
            ),
            documented_example: concat!(
                "/**\n",
                " * Adds the readings.\n",
                " *\n",
                " * let total = 0;\n",
                " * for (let value = 0; value < 4; value++) {\n",
                " *     total += value;\n",
                " * }\n",
                " * return total;\n",
                " */\n",
                "export function live(): void {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "export function live(): void {}\n",
            ),
            short_snippet: concat!(
                "// let total = 0;\n",
                "// let band = 1;\n",
                "export function live(): void {}\n",
            ),
        },
        LanguageSamples {
            path: "probe.js",
            commented_out_function: concat!(
                "// function disabled(limit) {\n",
                "//     let total = 0;\n",
                "//     for (let value = 0; value < limit; value++) {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     return total;\n",
                "// }\n",
                "export function live() {}\n",
            ),
            documented_example: concat!(
                "/**\n",
                " * Adds the readings.\n",
                " *\n",
                " * let total = 0;\n",
                " * for (let value = 0; value < 4; value++) {\n",
                " *     total += value;\n",
                " * }\n",
                " * return total;\n",
                " */\n",
                "export function live() {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "export function live() {}\n",
            ),
            short_snippet: concat!(
                "// let total = 0;\n",
                "// let band = 1;\n",
                "export function live() {}\n",
            ),
        },
        LanguageSamples {
            path: "probe.go",
            commented_out_function: concat!(
                "// func disabled(limit int) int {\n",
                "//     total := 0\n",
                "//     for value := 0; value < limit; value++ {\n",
                "//         total += value\n",
                "//     }\n",
                "//     return total\n",
                "// }\n",
                "func Live() {}\n",
            ),
            documented_example: concat!(
                "/**\n",
                " * Adds the readings.\n",
                " *\n",
                " * total := 0\n",
                " * for value := 0; value < 4; value++ {\n",
                " *     total += value\n",
                " * }\n",
                " * return total\n",
                " */\n",
                "func Live() {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "func Live() {}\n",
            ),
            short_snippet: concat!(
                "// var total = 0\n",
                "// var band = 1\n",
                "func Live() {}\n"
            ),
        },
        LanguageSamples {
            path: "Probe.java",
            commented_out_function: concat!(
                "// int disabled(int limit) {\n",
                "//     int total = 0;\n",
                "//     for (int value = 0; value < limit; value++) {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     return total;\n",
                "// }\n",
                "class Live {}\n",
            ),
            documented_example: concat!(
                "/**\n",
                " * Adds the readings.\n",
                " *\n",
                " * int total = 0;\n",
                " * for (int value = 0; value < 4; value++) {\n",
                " *     total += value;\n",
                " * }\n",
                " * return total;\n",
                " */\n",
                "class Live {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "class Live {}\n",
            ),
            short_snippet: concat!(
                "// class Total {}\n",
                "// class Band {}\n",
                "class Live {}\n"
            ),
        },
        LanguageSamples {
            path: "probe.c",
            commented_out_function: concat!(
                "// int disabled(int limit) {\n",
                "//     int total = 0;\n",
                "//     for (int value = 0; value < limit; value++) {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     return total;\n",
                "// }\n",
                "int live(void) { return 1; }\n",
            ),
            documented_example: concat!(
                "/**\n",
                " * Adds the readings.\n",
                " *\n",
                " * int total = 0;\n",
                " * for (int value = 0; value < 4; value++) {\n",
                " *     total += value;\n",
                " * }\n",
                " * return total;\n",
                " */\n",
                "int live(void) { return 1; }\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "int live(void) { return 1; }\n",
            ),
            short_snippet: concat!(
                "// int total = 0;\n",
                "// int band = 1;\n",
                "int live(void) { return 1; }\n",
            ),
        },
        LanguageSamples {
            path: "probe.cpp",
            commented_out_function: concat!(
                "// int disabled(int limit) {\n",
                "//     int total = 0;\n",
                "//     for (int value = 0; value < limit; value++) {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     return total;\n",
                "// }\n",
                "int live() { return 1; }\n",
            ),
            documented_example: concat!(
                "/**\n",
                " * Adds the readings.\n",
                " *\n",
                " * int total = 0;\n",
                " * for (int value = 0; value < 4; value++) {\n",
                " *     total += value;\n",
                " * }\n",
                " * return total;\n",
                " */\n",
                "int live() { return 1; }\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "int live() { return 1; }\n",
            ),
            short_snippet: concat!(
                "// int total = 0;\n",
                "// int band = 1;\n",
                "int live() { return 1; }\n",
            ),
        },
        LanguageSamples {
            path: "Probe.cs",
            commented_out_function: concat!(
                "// int Disabled(int limit) {\n",
                "//     int total = 0;\n",
                "//     for (int value = 0; value < limit; value++) {\n",
                "//         total += value;\n",
                "//     }\n",
                "//     return total;\n",
                "// }\n",
                "class Live {}\n",
            ),
            documented_example: concat!(
                "/// <summary>Adds the readings.</summary>\n",
                "/// <example>\n",
                "/// int total = 0;\n",
                "/// for (int value = 0; value &lt; 4; value++) {\n",
                "///     total += value;\n",
                "/// }\n",
                "/// return total;\n",
                "/// </example>\n",
                "class Live {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "class Live {}\n",
            ),
            short_snippet: concat!(
                "// class Total {}\n",
                "// class Band {}\n",
                "class Live {}\n"
            ),
        },
        LanguageSamples {
            path: "Probe.swift",
            commented_out_function: concat!(
                "// func disabled(limit: Int) -> Int {\n",
                "//     var total = 0\n",
                "//     for value in 0..<limit {\n",
                "//         total += value\n",
                "//     }\n",
                "//     return total\n",
                "// }\n",
                "func live() {}\n",
            ),
            documented_example: concat!(
                "/// Adds the readings.\n",
                "///\n",
                "/// var total = 0\n",
                "/// for value in 0..<4 {\n",
                "///     total += value\n",
                "/// }\n",
                "/// return total\n",
                "func live() {}\n",
            ),
            prose: concat!(
                "// The reading loop below walks the grid once.\n",
                "// It keeps the running band in a local, because the\n",
                "// caller has no use for the partial sums.\n",
                "// TODO: the band should come from configuration once\n",
                "// the settings file grows a section for it.\n",
                "// See the design note for why the order matters.\n",
                "func live() {}\n",
            ),
            short_snippet: concat!(
                "// var total = 0\n",
                "// var band = 1\n",
                "func live() {}\n"
            ),
        },
    ];

    /// The blocks reported for a sample, with a message naming the sample.
    fn blocks_of(path: &str, source: &str, sample: &str) -> Vec<CommentedCodeBlock> {
        commented_code_blocks(path, source)
            .unwrap_or_else(|| panic!("{path} must be measured for its {sample} sample"))
    }

    #[test]
    fn every_language_reports_a_commented_out_function() {
        for samples in LANGUAGE_SAMPLES {
            let blocks = blocks_of(
                samples.path,
                samples.commented_out_function,
                "commented-out function",
            );
            assert_eq!(
                blocks.len(),
                1,
                "{} must report its commented-out function once, got {blocks:?}",
                samples.path
            );
            assert_eq!(blocks[0].line, 1, "{}", samples.path);
        }
    }

    #[test]
    fn every_language_leaves_a_documented_example_alone() {
        for samples in LANGUAGE_SAMPLES {
            let blocks = blocks_of(
                samples.path,
                samples.documented_example,
                "documented example",
            );
            assert!(
                blocks.is_empty(),
                "{} must not report code inside a doc comment, got {blocks:?}",
                samples.path
            );
        }
    }

    #[test]
    fn every_language_leaves_prose_alone() {
        for samples in LANGUAGE_SAMPLES {
            let blocks = blocks_of(samples.path, samples.prose, "prose");
            assert!(
                blocks.is_empty(),
                "{} must not report a paragraph of prose, got {blocks:?}",
                samples.path
            );
        }
    }

    #[test]
    fn every_language_leaves_a_short_snippet_alone() {
        for samples in LANGUAGE_SAMPLES {
            let blocks = blocks_of(samples.path, samples.short_snippet, "short snippet");
            assert!(
                blocks.is_empty(),
                "{} must not report a two-line snippet, got {blocks:?}",
                samples.path
            );
        }
    }

    /// A commented-out Rust function written as one block comment, in the
    /// `*`-continuation style a C-family block comment conventionally uses.
    const RUST_BLOCK_COMMENTED_FUNCTION: &str = concat!(
        "/*\n",
        " * fn disabled(limit: i32) -> i32 {\n",
        " *     let mut total = 0;\n",
        " *     for value in 0..limit {\n",
        " *         total += value;\n",
        " *     }\n",
        " *     total\n",
        " * }\n",
        " */\n",
        "pub fn live() {}\n",
    );

    #[test]
    fn a_block_comment_hiding_a_function_is_reported() {
        let blocks = blocks_of("probe.rs", RUST_BLOCK_COMMENTED_FUNCTION, "block comment");
        assert_eq!(
            blocks.len(),
            1,
            "a `/* */` comment hiding a function is the same finding a `//` run is, got {blocks:?}"
        );
        assert_eq!(blocks[0].lines, 9);
    }

    /// A Python function whose body holds a commented-out block, indented.
    ///
    /// The strip must give the code its own indentation back, or the re-parse
    /// is an indentation error rather than a function.
    const PYTHON_INDENTED_COMMENTED_BLOCK: &str = concat!(
        "def live(rows):\n",
        "    # for row in rows:\n",
        "    #     total = 0\n",
        "    #     for cell in row:\n",
        "    #         total += cell\n",
        "    #     if total > 0:\n",
        "    #         yield total\n",
        "    return 1\n",
    );

    #[test]
    fn an_indented_commented_out_block_keeps_its_own_indentation() {
        let blocks = blocks_of(
            "probe.py",
            PYTHON_INDENTED_COMMENTED_BLOCK,
            "indented commented-out block",
        );
        assert_eq!(
            blocks.len(),
            1,
            "the strip must return the code's own indentation, got {blocks:?}"
        );
        assert_eq!(blocks[0].line, 2);
        assert_eq!(blocks[0].lines, 6);
    }

    /// Six lines of one call, commented out. Real code, and one statement.
    const TYPESCRIPT_ONE_STATEMENT_BLOCK: &str = concat!(
        "// renderBoard(\n",
        "//   columns,\n",
        "//   tasks,\n",
        "//   theme,\n",
        "//   locale\n",
        "// );\n",
        "export function live(): void {}\n",
    );

    /// Six lines holding exactly two commented-out statements.
    const TYPESCRIPT_TWO_STATEMENT_BLOCK: &str = concat!(
        "// renderBoard(\n",
        "//   columns,\n",
        "//   tasks,\n",
        "//   theme\n",
        "// );\n",
        "// commitBoard(columns);\n",
        "export function live(): void {}\n",
    );

    #[test]
    fn a_block_that_re_parses_to_two_statements_is_reported() {
        let blocks = blocks_of(
            "probe.ts",
            TYPESCRIPT_TWO_STATEMENT_BLOCK,
            "two-statement block",
        );
        assert_eq!(
            blocks.len(),
            1,
            "two statements is the gate, so this block is over it, got {blocks:?}"
        );
    }

    #[test]
    fn a_block_that_re_parses_to_one_statement_is_left_alone() {
        let blocks = blocks_of(
            "probe.ts",
            TYPESCRIPT_ONE_STATEMENT_BLOCK,
            "one-statement block",
        );
        assert!(
            blocks.is_empty(),
            "one statement is under the item gate, got {blocks:?}"
        );
    }

    /// A five-line commented-out Rust function: one line under the gate.
    const RUST_FIVE_LINE_COMMENTED_FUNCTION: &str = concat!(
        "// fn disabled() -> i32 {\n",
        "//     let total = 0;\n",
        "//     let band = 1;\n",
        "//     total + band\n",
        "// }\n",
        "pub fn live() {}\n",
    );

    /// The same function with one more line, which puts it over the gate.
    const RUST_SIX_LINE_COMMENTED_FUNCTION: &str = concat!(
        "// fn disabled() -> i32 {\n",
        "//     let total = 0;\n",
        "//     let band = 1;\n",
        "//     let step = 2;\n",
        "//     total + band + step\n",
        "// }\n",
        "pub fn live() {}\n",
    );

    #[test]
    fn the_line_gate_sits_between_five_lines_and_six() {
        let five = blocks_of(
            "probe.rs",
            RUST_FIVE_LINE_COMMENTED_FUNCTION,
            "five-line block",
        );
        assert!(
            five.is_empty(),
            "five lines is not more than five, got {five:?}"
        );
        let six = blocks_of(
            "probe.rs",
            RUST_SIX_LINE_COMMENTED_FUNCTION,
            "six-line block",
        );
        assert_eq!(six.len(), 1, "six lines is over the gate, got {six:?}");
        assert_eq!(six[0].lines, 6);
    }

    /// Six lines of Go, each carrying a trailing comment a formatter aligned.
    ///
    /// This is the `gohugoio/hugo` shape that a column test could not tell from
    /// a block: `gofmt` pads the code so every `//` lands in one column, and
    /// the comments alone re-parse as six clean Go statements. The live code to
    /// their left is what separates them.
    const GO_ALIGNED_TRAILING_COMMENTS: &str = concat!(
        "func Live(p func(int) bool) {\n",
        "\tcheck(p(10), true)  // true || true && true && true\n",
        "\tcheck(p(2), false)  // true || true && false && false\n",
        "\tcheck(p(1), false)  // true || false && false && false\n",
        "\tcheck(p(3), false)  // false || false && false && false\n",
        "\tcheck(p(4), false)  // false || false && false && false\n",
        "\tcheck(p(42), false) // false || false && false && false\n",
        "}\n",
    );

    #[test]
    fn a_run_of_aligned_trailing_comments_is_not_one_block() {
        let blocks = blocks_of(
            "probe.go",
            GO_ALIGNED_TRAILING_COMMENTS,
            "trailing comment run",
        );
        assert!(
            blocks.is_empty(),
            "a comment with live code to its left annotates that line, got {blocks:?}"
        );
    }

    /// A Go doc comment listing seven bare words, one per line.
    ///
    /// The `gohugoio/hugo` shape that made the item gate need a substance test:
    /// tree-sitter-go reads each word as `(expression_statement (identifier))`,
    /// so a list of words re-parses with no error node at all.
    const GO_WORD_LIST_COMMENT: &str = concat!(
        "// Hooks:\n",
        "// table\n",
        "// passthrough\n",
        "// link\n",
        "// image\n",
        "// heading\n",
        "// codeblock\n",
        "// blockquote\n",
        "func Live() {}\n",
    );

    #[test]
    fn a_list_of_bare_words_is_not_code() {
        let blocks = blocks_of("probe.go", GO_WORD_LIST_COMMENT, "word list");
        assert!(
            blocks.is_empty(),
            "a statement holding one bare name is a word, got {blocks:?}"
        );
    }

    /// A PEP 723 inline script metadata block, byte for byte the header this
    /// workspace's own `crates/ane-embedding/convert` scripts carry.
    ///
    /// Every dependency line re-parses as a clean Python assignment, so only
    /// the `# ///` fences fail — an error ratio of 0.110, which is what puts
    /// this block over [`MAX_ERROR_BYTE_RATIO`] and not under it.
    const PYTHON_INLINE_SCRIPT_METADATA: &str = concat!(
        "#!/usr/bin/env -S uv run --python 3.12\n",
        "# /// script\n",
        "# requires-python = \">=3.12,<3.13\"\n",
        "# dependencies = [\n",
        "#     \"torch>=2.7,<2.8\",\n",
        "#     \"transformers>=4.51,<4.52\",\n",
        "#     \"coremltools>=8.0,<10.0\",\n",
        "#     \"numpy\",\n",
        "#     \"peft>=0.11\",\n",
        "#     \"huggingface_hub>=0.20\",\n",
        "#     \"scikit-learn\",\n",
        "# ]\n",
        "# ///\n",
        "def live():\n",
        "    return 1\n",
    );

    #[test]
    fn python_inline_script_metadata_is_not_commented_out_code() {
        let blocks = blocks_of(
            "probe.py",
            PYTHON_INLINE_SCRIPT_METADATA,
            "PEP 723 metadata",
        );
        assert!(
            blocks.is_empty(),
            "a PEP 723 fence marks standardized metadata, got {blocks:?}"
        );
    }

    /// A Rust module doc comment, a blank line, then a commented-out function.
    ///
    /// tree-sitter-rust puts the terminating newline inside a `//!` node, so
    /// the doc comment's `end_position().row` is already the blank line below
    /// it. Read naively, the doc comment joins the block two lines down and
    /// exempts the whole thing.
    const RUST_DOC_COMMENT_ABOVE_A_COMMENTED_BLOCK: &str = concat!(
        "//! A probe module.\n",
        "\n",
        "// fn disabled(limit: i32) -> i32 {\n",
        "//     let mut total = 0;\n",
        "//     for value in 0..limit {\n",
        "//         total += value;\n",
        "//     }\n",
        "//     total\n",
        "// }\n",
        "\n",
        "pub fn live() {}\n",
    );

    #[test]
    fn a_doc_comment_above_a_blank_line_does_not_swallow_the_block_below() {
        let blocks = blocks_of(
            "probe.rs",
            RUST_DOC_COMMENT_ABOVE_A_COMMENTED_BLOCK,
            "doc comment above a block",
        );
        assert_eq!(
            blocks.len(),
            1,
            "the doc comment ends on its own line, so the block below stands alone, got {blocks:?}"
        );
        assert_eq!(blocks[0].line, 3);
        assert_eq!(blocks[0].lines, 7);
    }

    #[test]
    fn a_rust_doc_comment_is_excluded_by_its_grammar_marker_node() {
        let source = LANGUAGE_SAMPLES[0].documented_example;
        let parsed = parse_code("probe.rs", source).expect("rust is in the roster");
        let mut comments = Vec::new();
        collect_comments(parsed.tree().root_node(), &RUST_COMMENT_SPEC, &mut comments);

        assert!(!comments.is_empty(), "the sample must carry doc comments");
        for comment in &comments {
            let mut cursor = comment.walk();
            assert!(
                comment
                    .children(&mut cursor)
                    .any(|child| RUST_COMMENT_SPEC.doc_marker_kinds.contains(&child.kind())),
                "the rust grammar must mark this doc comment with a marker node, not a delimiter"
            );
        }
    }

    #[test]
    fn a_language_with_no_comment_spec_is_reported_as_not_measured() {
        for path in ["probe.rb", "probe.sh", "probe.ex", "probe.php", "probe.f90"] {
            assert_eq!(
                commented_code_blocks(path, "# nothing here\n"),
                None,
                "{path} has no comment spec, so its verdict is not measured"
            );
        }
    }

    #[test]
    fn a_path_the_roster_does_not_claim_is_reported_as_not_measured() {
        assert_eq!(commented_code_blocks("notes.txt", "# hello\n"), None);
        assert_eq!(commented_code_blocks("Makefile", "all:\n"), None);
    }

    #[test]
    fn every_spec_language_contributes_its_extensions() {
        let extensions = commented_code_extensions();
        for expected in [
            ".rs", ".py", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".go", ".java", ".c", ".h",
            ".cpp", ".cs", ".swift",
        ] {
            assert!(
                extensions.contains(&expected),
                "{expected} must be covered, got {extensions:?}"
            );
        }
        for absent in [".rb", ".sh", ".ex", ".php", ".f90"] {
            assert!(
                !extensions.contains(&absent),
                "{absent} has no comment spec, so it must not be covered"
            );
        }
        assert_eq!(
            extensions.len(),
            COMMENT_SPECS
                .iter()
                .filter_map(|spec| super::super::languages::extensions_for_language(spec.language))
                .map(<[&str]>::len)
                .sum::<usize>(),
            "every spec language must resolve to a roster row"
        );
    }
}
