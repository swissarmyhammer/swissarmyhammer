//! Which declarations a file puts on its public surface, and what a change did
//! to that surface.
//!
//! A diff read by eye shows a body edit and an API break as the same thing:
//! text that moved. This module tells them apart as a measurement over two
//! tree-sitter parses, so a review rule compares rows instead of re-reading
//! declarations.
//!
//! # What public means here
//!
//! [`SURFACE_SPECS`] is the vocabulary: one row per language naming how that
//! language spells "this declaration reaches outside its file" — a modifier
//! keyword, an `export` ancestor, or a convention in the name itself. Adding a
//! language is adding a row.
//!
//! Visibility is read at the declaration itself and never through its parents,
//! so a `pub` function inside a private module reads as public. That is the
//! honest answer for a file-scoped measurement: the declaration says it is API.
//!
//! A language absent from the roster is **not measured** —
//! [`ParsedCode::public_surface`](super::ParsedCode::public_surface) returns
//! `None` — and a caller must report "not computed" rather than "this change
//! moved no public symbol".
//!
//! # One differ
//!
//! [`PublicSurface::changes_from`] matches the two revisions with
//! [`match_entities`], the entity-level differ the `get diff` op runs. Nothing
//! here decides for itself which symbol on one side is which symbol on the
//! other.

use std::collections::HashMap;

use tree_sitter::{Node, Tree};

use super::entity_extractor::extract_entity_nodes;
use super::languages::LanguageConfig;
use crate::model::change::SemanticChange;
use crate::model::entity::SemanticEntity;
use crate::model::identity::{default_similarity, match_entities};

/// Whether a declaration reaches outside the file that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visibility {
    /// The declaration reaches outside its file.
    Public,
    /// The declaration stays inside its file.
    Private,
}

impl Visibility {
    /// The visibility as an evidence row names it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

impl std::fmt::Display for Visibility {
    /// The visibility as an evidence row names it — [`Visibility::label`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// One declaration, as one revision of a file spells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceSymbol {
    /// The entity id the differ matches the two revisions on.
    pub entity_id: String,
    /// The symbol as the source names it, outermost parent first (`Circle::new`).
    pub symbol_path: String,
    /// The declaration with its body removed and its whitespace runs collapsed
    /// (`pub fn new(x: f64) -> Self`).
    pub signature: String,
    /// Whether the declaration reaches outside the file.
    pub visibility: Visibility,
    /// The 1-based line the declaration starts on.
    pub start_line: usize,
}

/// What a change did to one symbol on a file's public surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SurfaceChangeKind {
    /// The change put on the surface a symbol that was not on it.
    Added,
    /// The change took off the surface a symbol that was on it.
    Removed,
    /// The symbol stayed on the surface, and its declaration is spelled
    /// differently.
    SignatureChanged,
    /// The symbol stayed, and whether it reaches outside the file changed.
    VisibilityChanged,
}

impl SurfaceChangeKind {
    /// The change as an evidence row names it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "added to the public surface",
            Self::Removed => "removed from the public surface",
            Self::SignatureChanged => "signature changed",
            Self::VisibilityChanged => "visibility changed",
        }
    }
}

impl std::fmt::Display for SurfaceChangeKind {
    /// The change as an evidence row names it — [`SurfaceChangeKind::label`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// One symbol whose place on a file's public surface a change altered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceChange {
    /// The symbol as the source names it, outermost parent first.
    pub symbol_path: String,
    /// What the change did to it.
    pub kind: SurfaceChangeKind,
    /// The 1-based line the declaration starts on, in whichever revision holds
    /// it.
    pub start_line: usize,
    /// The declaration at the base revision, `None` when the change added it.
    pub before_signature: Option<String>,
    /// The declaration under review, `None` when the change removed it.
    pub after_signature: Option<String>,
}

/// Every declaration one revision of a file holds, read for its surface.
///
/// Private declarations are held too: a change that publishes one is a surface
/// change, and it cannot be seen from the public half alone.
#[derive(Debug, Clone)]
pub struct PublicSurface {
    file_path: String,
    entities: Vec<SemanticEntity>,
    symbols: Vec<SurfaceSymbol>,
}

impl PublicSurface {
    /// Every declaration this revision holds, in the order the file declares
    /// them.
    pub fn symbols(&self) -> &[SurfaceSymbol] {
        &self.symbols
    }

    /// What the change from `before` to THIS revision did to the public
    /// surface, in file order.
    ///
    /// The two revisions are matched by [`match_entities`], so a declaration
    /// that only moved down the file is the same symbol here, and a rename is
    /// one symbol re-spelled rather than one removal beside one addition.
    pub fn changes_from(&self, before: &Self) -> Vec<SurfaceChange> {
        let matched = match_entities(
            &before.entities,
            &self.entities,
            &self.file_path,
            Some(&default_similarity),
            None,
            None,
        );
        let index = SurfaceIndex::new(before, self);
        let mut changes: Vec<SurfaceChange> = matched
            .changes
            .iter()
            .filter_map(|change| index.surface_change(change))
            .collect();
        changes.sort_by(|one, other| {
            (one.start_line, &one.symbol_path).cmp(&(other.start_line, &other.symbol_path))
        });
        changes
    }
}

/// The two revisions' declarations, indexed the way the differ's output has to
/// be joined back to them.
struct SurfaceIndex<'a> {
    before_by_id: HashMap<&'a str, &'a SurfaceSymbol>,
    before_by_content: HashMap<&'a str, &'a SurfaceSymbol>,
    after_by_id: HashMap<&'a str, &'a SurfaceSymbol>,
}

impl<'a> SurfaceIndex<'a> {
    /// Index both revisions.
    fn new(before: &'a PublicSurface, after: &'a PublicSurface) -> Self {
        Self {
            before_by_id: by_id(before),
            before_by_content: by_content(before),
            after_by_id: by_id(after),
        }
    }

    /// One matched entity as a surface change, `None` when the two revisions
    /// leave the public surface as it was.
    fn surface_change(&self, change: &SemanticChange) -> Option<SurfaceChange> {
        let after = self.after_by_id.get(change.entity_id.as_str()).copied();
        let before = self.before_symbol(change);
        match (before, after) {
            (None, Some(after)) => appearance(after, SurfaceChangeKind::Added),
            (Some(before), None) => appearance(before, SurfaceChangeKind::Removed),
            (Some(before), Some(after)) => respelling(before, after),
            (None, None) => None,
        }
    }

    /// The base revision's declaration for `change`, `None` when the base
    /// revision does not hold it.
    ///
    /// A renamed or moved symbol is reported under the id it carries UNDER
    /// REVIEW, which the base revision never held, so it is found instead by
    /// the content the differ recorded for its base side.
    fn before_symbol(&self, change: &SemanticChange) -> Option<&'a SurfaceSymbol> {
        self.before_by_id
            .get(change.entity_id.as_str())
            .copied()
            .or_else(|| {
                let content = change.before_content.as_deref()?;
                self.before_by_content.get(content).copied()
            })
    }
}

/// One revision's declarations, keyed by the entity id the differ reports.
fn by_id(surface: &PublicSurface) -> HashMap<&str, &SurfaceSymbol> {
    surface
        .symbols
        .iter()
        .map(|symbol| (symbol.entity_id.as_str(), symbol))
        .collect()
}

/// One revision's declarations, keyed by the entity text the differ records for
/// a renamed symbol's base side.
fn by_content(surface: &PublicSurface) -> HashMap<&str, &SurfaceSymbol> {
    surface
        .entities
        .iter()
        .zip(&surface.symbols)
        .map(|(entity, symbol)| (entity.content.as_str(), symbol))
        .collect()
}

/// A symbol only one revision holds, as a surface change — `None` when that
/// symbol never reached outside the file, so neither adding nor removing it
/// touched the surface.
fn appearance(symbol: &SurfaceSymbol, kind: SurfaceChangeKind) -> Option<SurfaceChange> {
    if symbol.visibility != Visibility::Public {
        return None;
    }
    let signature = Some(symbol.signature.clone());
    let (before_signature, after_signature) = match kind {
        SurfaceChangeKind::Removed => (signature, None),
        SurfaceChangeKind::Added
        | SurfaceChangeKind::SignatureChanged
        | SurfaceChangeKind::VisibilityChanged => (None, signature),
    };
    Some(SurfaceChange {
        symbol_path: symbol.symbol_path.clone(),
        kind,
        start_line: symbol.start_line,
        before_signature,
        after_signature,
    })
}

/// Two revisions of one symbol as a surface change — `None` when they leave the
/// surface as it was, which is every edit under an unchanged public declaration
/// and every edit to a declaration that stays inside the file.
fn respelling(before: &SurfaceSymbol, after: &SurfaceSymbol) -> Option<SurfaceChange> {
    let kind = if before.visibility != after.visibility {
        SurfaceChangeKind::VisibilityChanged
    } else if after.visibility == Visibility::Public && before.signature != after.signature {
        SurfaceChangeKind::SignatureChanged
    } else {
        return None;
    };
    Some(SurfaceChange {
        symbol_path: after.symbol_path.clone(),
        kind,
        start_line: after.start_line,
        before_signature: Some(before.signature.clone()),
        after_signature: Some(after.signature.clone()),
    })
}

/// Read every declaration `tree` holds, `None` when the language has no row in
/// [`SURFACE_SPECS`].
pub(super) fn read(
    tree: &Tree,
    file_path: &str,
    config: &'static LanguageConfig,
    source: &str,
) -> Option<PublicSurface> {
    let spec = spec_for_language(config.id)?;
    let extracted = extract_entity_nodes(tree, file_path, config, source);
    let symbols = {
        let names: HashMap<&str, &str> = extracted
            .iter()
            .map(|(entity, _node)| (entity.id.as_str(), entity.name.as_str()))
            .collect();
        let parents: HashMap<&str, &str> = extracted
            .iter()
            .filter_map(|(entity, _node)| Some((entity.id.as_str(), entity.parent_id.as_deref()?)))
            .collect();
        extracted
            .iter()
            .map(|(entity, node)| SurfaceSymbol {
                entity_id: entity.id.clone(),
                symbol_path: symbol_path(entity, &names, &parents),
                signature: signature(*node, spec, source),
                visibility: visibility(*node, &entity.name, spec, source),
                start_line: entity.start_line,
            })
            .collect()
    };
    Some(PublicSurface {
        file_path: file_path.to_string(),
        entities: extracted
            .into_iter()
            .map(|(entity, _node)| entity)
            .collect(),
        symbols,
    })
}

/// How many parents a symbol path names before it stops climbing. Deep enough
/// for any real nesting, and a bound the climb cannot exceed however the entity
/// ids were built.
const MAX_SYMBOL_PATH_DEPTH: usize = 16;

/// What a symbol path puts between a parent's name and its child's.
const SYMBOL_PATH_SEPARATOR: &str = "::";

/// The symbol as the source names it, outermost parent first (`Circle::new`).
///
/// Built by climbing the entity's own parent chain, so it reads the way the file
/// declares it whatever the language spells a scope separator as.
fn symbol_path(
    entity: &SemanticEntity,
    names: &HashMap<&str, &str>,
    parents: &HashMap<&str, &str>,
) -> String {
    let mut parts = vec![entity.name.as_str()];
    let mut current = entity.parent_id.as_deref();
    while let Some(id) = current {
        if parts.len() >= MAX_SYMBOL_PATH_DEPTH {
            break;
        }
        let Some(name) = names.get(id) else {
            break;
        };
        parts.push(name);
        current = parents.get(id).copied();
    }
    parts.reverse();
    parts.join(SYMBOL_PATH_SEPARATOR)
}

/// The field every mapped grammar gives a declaration's body. A signature is
/// the declaration text up to it.
const BODY_FIELD: &str = "body";

/// The declaration `node` spells, with its body removed and its whitespace runs
/// collapsed.
///
/// A declaration the grammar gives no body — a constant, a type alias — is read
/// to the end of its first line instead, which is the whole of every such
/// declaration and a bound on the ones that run on.
fn signature(node: Node<'_>, spec: &SurfaceSpec, source: &str) -> String {
    let declaration = declaration_node(node, spec);
    let end = declaration.child_by_field_name(BODY_FIELD).map_or_else(
        || first_line_end(declaration, source),
        |body| body.start_byte(),
    );
    let text = source
        .get(declaration.start_byte()..end)
        .unwrap_or_default();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The byte at which `node`'s first line ends, or where the node does when it
/// holds one line only.
fn first_line_end(node: Node<'_>, source: &str) -> usize {
    let start = node.start_byte();
    let text = source.get(start..node.end_byte()).unwrap_or_default();
    match text.find('\n') {
        Some(offset) => start + offset,
        None => node.end_byte(),
    }
}

/// The node that actually spells the declaration — `node` itself, unless
/// [`SurfaceSpec::wrapper_fields`] names a field whose child is the declaration
/// the wrapper stands above.
fn declaration_node<'tree>(node: Node<'tree>, spec: &SurfaceSpec) -> Node<'tree> {
    spec.wrapper_fields
        .iter()
        .find_map(|field| node.child_by_field_name(field))
        .unwrap_or(node)
}

/// Whether the declaration at `node` reaches outside its file.
///
/// The order is deliberate. A modifier the declaration carries wins over
/// everything, because it is the language's own statement about the
/// declaration; an exporting ancestor comes next; and only a declaration that
/// carries neither falls through to what its name says.
fn visibility(node: Node<'_>, name: &str, spec: &SurfaceSpec, source: &str) -> Visibility {
    let words = modifier_words(node, spec, source);
    if words.iter().any(|word| spec.private_words.contains(word)) {
        return Visibility::Private;
    }
    if words.iter().any(|word| spec.public_words.contains(word)) {
        return Visibility::Public;
    }
    if has_exporting_ancestor(node, spec) {
        return Visibility::Public;
    }
    spec.name_convention.visibility(name)
}

/// Every word of every visibility modifier `node` carries.
///
/// A modifier is one word in some languages (`pub`, `public`) and a run of them
/// in others (Java's `@Override public static`), and Rust spells a restricted
/// one as a word inside brackets (`pub(crate)`). Splitting on everything that
/// is not part of a word reads all three the same way.
fn modifier_words<'a>(node: Node<'_>, spec: &SurfaceSpec, source: &'a str) -> Vec<&'a str> {
    if spec.modifier_kinds.is_empty() {
        return Vec::new();
    }
    let mut cursor = node.walk();
    let modifiers: Vec<Node<'_>> = node
        .named_children(&mut cursor)
        .filter(|child| spec.modifier_kinds.contains(&child.kind()))
        .collect();
    drop(cursor);
    modifiers
        .into_iter()
        .flat_map(|modifier| node_words(modifier, source))
        .collect()
}

/// The words in one node's text.
fn node_words<'a>(node: Node<'_>, source: &'a str) -> impl Iterator<Item = &'a str> {
    source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or_default()
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
}

/// Whether any ancestor of `node` is a statement that exports what it wraps.
fn has_exporting_ancestor(node: Node<'_>, spec: &SurfaceSpec) -> bool {
    if spec.exporting_ancestor_kinds.is_empty() {
        return false;
    }
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if spec.exporting_ancestor_kinds.contains(&ancestor.kind()) {
            return true;
        }
        current = ancestor.parent();
    }
    false
}

/// The character a name starts with to keep a declaration inside its file in a
/// language that leaves visibility to convention.
const PRIVATE_NAME_PREFIX: char = '_';

/// What a declaration's NAME says about its visibility in a language that
/// spells no modifier on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameConvention {
    /// The name says nothing: a declaration carrying neither a modifier nor an
    /// export stays inside its file.
    Silent,
    /// An initial capital reaches outside the file — Go's export convention.
    CapitalIsPublic,
    /// A leading underscore stays inside the file, and every other name reaches
    /// outside it — Python's convention.
    UnderscoreIsPrivate,
}

impl NameConvention {
    /// What this convention says about a declaration called `name`.
    fn visibility(self, name: &str) -> Visibility {
        match self {
            Self::Silent => Visibility::Private,
            Self::CapitalIsPublic => match name.chars().next() {
                Some(first) if first.is_uppercase() => Visibility::Public,
                _ => Visibility::Private,
            },
            Self::UnderscoreIsPrivate => {
                if name.starts_with(PRIVATE_NAME_PREFIX) {
                    Visibility::Private
                } else {
                    Visibility::Public
                }
            }
        }
    }
}

/// One language's row in the visibility roster: how it spells "this declaration
/// reaches outside its file".
///
/// One row read as data. A language is mapped here only once every field below
/// is true of it; until then the language is not measured at all.
struct SurfaceSpec {
    /// The language id, mirroring the [`LanguageConfig::id`] it pairs with.
    language: &'static str,
    /// Node kinds among a declaration's own named children that spell a
    /// visibility modifier.
    modifier_kinds: &'static [&'static str],
    /// Modifier words that reach outside the file.
    public_words: &'static [&'static str],
    /// Modifier words that do NOT, whatever else the same modifier says. Read
    /// first, so Rust's `pub(self)` loses to the `pub` beside it and a
    /// TypeScript member marked `private` stays private inside an exported
    /// class.
    private_words: &'static [&'static str],
    /// Node kinds among a declaration's ancestors that export what they wrap.
    exporting_ancestor_kinds: &'static [&'static str],
    /// What the declaration's name says when it carries neither a modifier nor
    /// an exporting ancestor.
    name_convention: NameConvention,
    /// Fields whose child IS the declaration, for an entity node that only
    /// wraps one — Python's `decorated_definition`, whose `definition` field
    /// holds the `def` or `class` the decorators stand above.
    wrapper_fields: &'static [&'static str],
}

/// Shared field values for a language that spells no modifier, exports nothing,
/// leaves nothing to a naming convention, and wraps no declaration.
const SURFACE_SPEC_DEFAULTS: SurfaceSpec = SurfaceSpec {
    language: "",
    modifier_kinds: &[],
    public_words: &[],
    private_words: &[],
    exporting_ancestor_kinds: &[],
    name_convention: NameConvention::Silent,
    wrapper_fields: &[],
};

/// Rust. A declaration reaches past its module only through a
/// `visibility_modifier`, whose restricted spellings put the scope in brackets
/// (`pub(crate)`, `pub(super)`, `pub(in path)`). `pub(self)` is the one
/// spelling that reaches no further than no modifier at all, and it is told
/// apart by the `self` word inside those brackets.
const RUST_SURFACE: SurfaceSpec = SurfaceSpec {
    language: "rust",
    modifier_kinds: &["visibility_modifier"],
    public_words: &["pub"],
    private_words: &["self"],
    ..SURFACE_SPEC_DEFAULTS
};

/// Go. The language carries no visibility keyword: an identifier starting with
/// an upper-case letter is exported from its package, and every other
/// identifier is not.
const GO_SURFACE: SurfaceSpec = SurfaceSpec {
    language: "go",
    name_convention: NameConvention::CapitalIsPublic,
    ..SURFACE_SPEC_DEFAULTS
};

/// Python. The language enforces no visibility at all; a leading underscore is
/// the convention that marks a name private. A decorated definition is a
/// `decorated_definition` node standing above the `def` or `class` it decorates,
/// so the signature is read from that inner declaration.
const PYTHON_SURFACE: SurfaceSpec = SurfaceSpec {
    language: "python",
    name_convention: NameConvention::UnderscoreIsPrivate,
    wrapper_fields: &["definition"],
    ..SURFACE_SPEC_DEFAULTS
};

/// Java. `public` and `protected` reach outside the declaring type; `private`
/// and the package-private default do not. A declaration's modifiers sit in one
/// `modifiers` node together with its annotations, so the words are read out of
/// that node rather than off the declaration itself.
const JAVA_SURFACE: SurfaceSpec = SurfaceSpec {
    language: "java",
    modifier_kinds: &["modifiers"],
    public_words: &["public", "protected"],
    private_words: &["private"],
    ..SURFACE_SPEC_DEFAULTS
};

/// The row of one JavaScript-family language.
///
/// JavaScript, TypeScript and TSX all reach outside a file through `export`,
/// and the TypeScript grammars spell a class member's access as an
/// `accessibility_modifier` the JavaScript grammar never produces. The three
/// rows are built here rather than written three times, so they cannot drift.
const fn javascript_family_surface(language: &'static str) -> SurfaceSpec {
    SurfaceSpec {
        language,
        modifier_kinds: &["accessibility_modifier"],
        public_words: &["public", "protected"],
        private_words: &["private"],
        exporting_ancestor_kinds: &["export_statement"],
        name_convention: NameConvention::Silent,
        wrapper_fields: &[],
    }
}

/// Every language whose public surface is measured. A language absent from this
/// roster is not measured at all.
static SURFACE_SPECS: &[SurfaceSpec] = &[
    RUST_SURFACE,
    GO_SURFACE,
    PYTHON_SURFACE,
    JAVA_SURFACE,
    javascript_family_surface("typescript"),
    javascript_family_surface("tsx"),
    javascript_family_surface("javascript"),
];

/// The roster row for `language`, `None` when the language has none.
///
/// Reads [`SURFACE_SPECS`], a slice of [`SurfaceSpec`] VALUES, where two of its
/// three same-named siblings read slices of references. The four rosters hold
/// four unrelated types, so the four one-line bodies stay apart — see the
/// `parser::plugins::code` module doc.
fn spec_for_language(language: &str) -> Option<&'static SurfaceSpec> {
    SURFACE_SPECS.iter().find(|spec| spec.language == language)
}

#[cfg(test)]
mod tests {
    use super::{PublicSurface, SurfaceChange, SurfaceChangeKind, Visibility};
    use crate::parser::plugins::code::parse_code;

    /// The surface of one source file, read through the real parser and the
    /// real roster.
    fn surface(path: &str, source: &str) -> PublicSurface {
        parse_code(path, source)
            .expect("the grammar roster parses this language")
            .public_surface(path, source)
            .expect("the visibility roster measures this language")
    }

    /// The declaration the roster read for `symbol_path`.
    fn symbol(path: &str, source: &str, symbol_path: &str) -> super::SurfaceSymbol {
        surface(path, source)
            .symbols()
            .iter()
            .find(|symbol| symbol.symbol_path == symbol_path)
            .unwrap_or_else(|| panic!("`{symbol_path}` is declared in `{path}`"))
            .clone()
    }

    /// Whether the roster reads `symbol_path` as reaching outside its file.
    fn visibility_of(path: &str, source: &str, symbol_path: &str) -> Visibility {
        symbol(path, source, symbol_path).visibility
    }

    /// What the change from `before` to `after` did to one file's surface.
    fn changes(path: &str, before: &str, after: &str) -> Vec<SurfaceChange> {
        surface(path, after).changes_from(&surface(path, before))
    }

    /// A Rust file spelling every visibility the language has, so one parse
    /// answers for the whole modifier vocabulary.
    const RUST_VISIBILITIES: &str = "\
        pub fn open() {}\n\
        pub(crate) fn crate_wide() {}\n\
        pub(super) fn parent_wide() {}\n\
        pub(self) fn same_module() {}\n\
        fn shut() {}\n";

    #[test]
    fn rust_reads_a_pub_modifier_as_reaching_outside_the_file() {
        assert_eq!(
            visibility_of("a.rs", RUST_VISIBILITIES, "open"),
            Visibility::Public
        );
        assert_eq!(
            visibility_of("a.rs", RUST_VISIBILITIES, "crate_wide"),
            Visibility::Public
        );
        assert_eq!(
            visibility_of("a.rs", RUST_VISIBILITIES, "parent_wide"),
            Visibility::Public
        );
    }

    /// `pub(self)` restricts an item to the module it already sits in, which is
    /// what carrying no modifier at all already means.
    #[test]
    fn rust_reads_pub_self_and_a_bare_declaration_as_staying_inside_the_file() {
        assert_eq!(
            visibility_of("a.rs", RUST_VISIBILITIES, "same_module"),
            Visibility::Private
        );
        assert_eq!(
            visibility_of("a.rs", RUST_VISIBILITIES, "shut"),
            Visibility::Private
        );
    }

    #[test]
    fn a_signature_stops_where_the_body_starts() {
        assert_eq!(
            symbol(
                "a.rs",
                "pub fn one(value: u8) -> u8 {\n    value\n}\n",
                "one"
            )
            .signature,
            "pub fn one(value: u8) -> u8"
        );
    }

    /// A declaration the grammar gives no body — a Rust constant — is the whole
    /// of its own first line.
    #[test]
    fn a_bodyless_declaration_is_its_own_signature() {
        assert_eq!(
            symbol("a.rs", "pub const LIMIT: u8 = 3;\n", "LIMIT").signature,
            "pub const LIMIT: u8 = 3;"
        );
    }

    #[test]
    fn a_symbol_path_names_the_parents_it_is_declared_under() {
        let source = "pub struct Circle;\nimpl Circle {\n    pub fn new() -> Self { Self }\n}\n";

        assert_eq!(
            symbol("a.rs", source, "Circle::new").visibility,
            Visibility::Public
        );
    }

    #[test]
    fn go_reads_an_initial_capital_as_exported() {
        let source = "package p\n\nfunc Exported() {}\n\nfunc unexported() {}\n";

        assert_eq!(
            visibility_of("a.go", source, "Exported"),
            Visibility::Public
        );
        assert_eq!(
            visibility_of("a.go", source, "unexported"),
            Visibility::Private
        );
    }

    #[test]
    fn python_reads_a_leading_underscore_as_private() {
        let source = "def api():\n    pass\n\ndef _helper():\n    pass\n";

        assert_eq!(visibility_of("a.py", source, "api"), Visibility::Public);
        assert_eq!(
            visibility_of("a.py", source, "_helper"),
            Visibility::Private
        );
    }

    /// A decorated definition is a wrapper node standing above the `def` it
    /// decorates, so the signature has to be read from the inner declaration
    /// rather than from the decorator line.
    #[test]
    fn python_reads_a_decorated_definitions_signature_from_the_definition() {
        let source = "@staticmethod\ndef build(value):\n    return value\n";

        assert_eq!(
            symbol("a.py", source, "build").signature,
            "def build(value):"
        );
    }

    /// A Java declaration's modifiers sit in one node beside its annotations,
    /// so `public` has to be found among the words of that run.
    #[test]
    fn java_reads_public_out_of_a_modifier_run_that_also_holds_an_annotation() {
        let source = "public class A {\n  @Override public int one() { return 1; }\n  private void two() {}\n  int three() { return 3; }\n}\n";

        assert_eq!(
            visibility_of("A.java", source, "A::one"),
            Visibility::Public
        );
        assert_eq!(
            visibility_of("A.java", source, "A::two"),
            Visibility::Private
        );
        assert_eq!(
            visibility_of("A.java", source, "A::three"),
            Visibility::Private,
            "a package-private Java method does not reach outside its package"
        );
    }

    /// A TypeScript file whose class is exported and whose members disagree
    /// about access, so the export ancestor and the member modifier are both
    /// exercised by one parse.
    const TYPESCRIPT_EXPORTS: &str = "\
        export function one(): void {}\n\
        function two(): void {}\n\
        export class C {\n\
          public open(): void {}\n\
          private shut(): void {}\n\
        }\n";

    #[test]
    fn typescript_reads_an_export_ancestor_as_reaching_outside_the_file() {
        assert_eq!(
            visibility_of("a.ts", TYPESCRIPT_EXPORTS, "one"),
            Visibility::Public
        );
        assert_eq!(
            visibility_of("a.ts", TYPESCRIPT_EXPORTS, "two"),
            Visibility::Private
        );
    }

    /// A member marked `private` stays inside its class however the class
    /// reaches outside the file.
    #[test]
    fn typescript_reads_a_private_member_of_an_exported_class_as_private() {
        assert_eq!(
            visibility_of("a.ts", TYPESCRIPT_EXPORTS, "C::open"),
            Visibility::Public
        );
        assert_eq!(
            visibility_of("a.ts", TYPESCRIPT_EXPORTS, "C::shut"),
            Visibility::Private
        );
    }

    #[test]
    fn a_language_with_no_roster_row_is_not_measured() {
        let source = "def one\n  1\nend\n";

        assert!(
            parse_code("a.rb", source)
                .expect("the grammar roster parses Ruby")
                .public_surface("a.rb", source)
                .is_none(),
            "an unmapped language must read as unknown, never as a file with no public surface"
        );
    }

    /// The differ matches the two revisions, so a declaration the change only
    /// moved down the file is one symbol on both sides rather than a removal
    /// beside an addition.
    #[test]
    fn a_public_declaration_that_only_moved_is_no_surface_change() {
        let before = "pub fn one() {}\nfn two() {}\n";
        let after = "fn two() {}\npub fn one() {}\n";

        assert_eq!(
            changes("a.rs", before, after),
            Vec::new(),
            "moving a declaration leaves the surface as it was"
        );
    }

    #[test]
    fn a_renamed_public_function_is_reported_once_with_both_spellings() {
        let before = "pub fn old_name(value: u8) -> u8 { value + 1 }\n";
        let after = "pub fn new_name(value: u8) -> u8 { value + 1 }\n";

        let changes = changes("a.rs", before, after);

        assert_eq!(changes.len(), 1, "one symbol re-spelled, got: {changes:?}");
        assert_eq!(changes[0].kind, SurfaceChangeKind::SignatureChanged);
        assert_eq!(
            changes[0].before_signature.as_deref(),
            Some("pub fn old_name(value: u8) -> u8")
        );
        assert_eq!(
            changes[0].after_signature.as_deref(),
            Some("pub fn new_name(value: u8) -> u8")
        );
    }

    #[test]
    fn a_private_declaration_added_and_removed_is_no_surface_change() {
        let before = "fn one() {}\n";
        let after = "fn two() {}\n";

        assert_eq!(
            changes("a.rs", before, after),
            Vec::new(),
            "a file's private declarations are not its public surface"
        );
    }

    /// One public declaration, kept unchanged on both sides, so the change
    /// under test is the second declaration alone.
    const KEPT_PUBLIC_DECLARATION: &str = "pub fn one(value: u8) -> u8 { value }\n";

    /// A second public declaration, added on one side only.
    const OTHER_PUBLIC_DECLARATION: &str = "pub fn two(value: u8) -> u8 { value + 1 }\n";

    /// The declaration [`OTHER_PUBLIC_DECLARATION`] spells, as a surface row
    /// reports it.
    const OTHER_PUBLIC_SIGNATURE: &str = "pub fn two(value: u8) -> u8";

    #[test]
    fn a_public_declaration_the_change_added_is_reported_as_added() {
        let before = KEPT_PUBLIC_DECLARATION.to_string();
        let after = format!("{KEPT_PUBLIC_DECLARATION}{OTHER_PUBLIC_DECLARATION}");

        let changes = changes("a.rs", &before, &after);

        assert_eq!(changes.len(), 1, "one symbol added, got: {changes:?}");
        assert_eq!(changes[0].symbol_path, "two");
        assert_eq!(changes[0].kind, SurfaceChangeKind::Added);
        assert_eq!(changes[0].before_signature, None);
        assert_eq!(
            changes[0].after_signature.as_deref(),
            Some(OTHER_PUBLIC_SIGNATURE)
        );
    }

    #[test]
    fn a_public_declaration_the_change_removed_is_reported_as_removed() {
        let before = format!("{KEPT_PUBLIC_DECLARATION}{OTHER_PUBLIC_DECLARATION}");
        let after = KEPT_PUBLIC_DECLARATION.to_string();

        let changes = changes("a.rs", &before, &after);

        assert_eq!(changes.len(), 1, "one symbol removed, got: {changes:?}");
        assert_eq!(changes[0].symbol_path, "two");
        assert_eq!(changes[0].kind, SurfaceChangeKind::Removed);
        assert_eq!(
            changes[0].before_signature.as_deref(),
            Some(OTHER_PUBLIC_SIGNATURE)
        );
        assert_eq!(changes[0].after_signature, None);
    }

    /// One declaration, spelled private and public, so a test tells the two
    /// visibilities of one symbol apart.
    const HIDDEN_DECLARATION: &str = "fn one(value: u8) -> u8 { value }\n";

    /// The declaration [`HIDDEN_DECLARATION`] spells, as a surface row reports
    /// it.
    const HIDDEN_SIGNATURE: &str = "fn one(value: u8) -> u8";

    /// The declaration [`KEPT_PUBLIC_DECLARATION`] spells, as a surface row
    /// reports it.
    const PUBLISHED_SIGNATURE: &str = "pub fn one(value: u8) -> u8";

    /// Publishing a declaration that stayed inside the file puts it on the
    /// surface, though nothing of it but the modifier moved.
    #[test]
    fn a_declaration_the_change_published_is_reported_as_visibility_changed() {
        let changes = changes("a.rs", HIDDEN_DECLARATION, KEPT_PUBLIC_DECLARATION);

        assert_eq!(changes.len(), 1, "one symbol published, got: {changes:?}");
        assert_eq!(changes[0].symbol_path, "one");
        assert_eq!(changes[0].kind, SurfaceChangeKind::VisibilityChanged);
        assert_eq!(
            changes[0].before_signature.as_deref(),
            Some(HIDDEN_SIGNATURE)
        );
        assert_eq!(
            changes[0].after_signature.as_deref(),
            Some(PUBLISHED_SIGNATURE)
        );
    }

    /// Hiding a declaration is a visibility change and not a removal: the
    /// symbol is still declared, and the differ still matches it across the two
    /// revisions.
    #[test]
    fn a_declaration_the_change_hid_is_reported_as_visibility_changed() {
        let changes = changes("a.rs", KEPT_PUBLIC_DECLARATION, HIDDEN_DECLARATION);

        assert_eq!(changes.len(), 1, "one symbol hidden, got: {changes:?}");
        assert_eq!(changes[0].symbol_path, "one");
        assert_eq!(changes[0].kind, SurfaceChangeKind::VisibilityChanged);
        assert_eq!(
            changes[0].before_signature.as_deref(),
            Some(PUBLISHED_SIGNATURE)
        );
        assert_eq!(
            changes[0].after_signature.as_deref(),
            Some(HIDDEN_SIGNATURE)
        );
    }
}
