use tree_sitter::{Node, Tree};

use super::languages::LanguageConfig;
use crate::model::entity::{build_entity_id, SemanticEntity};
use crate::utils::hash::{content_hash, structural_hash};

/// Every entity one parse of a file declares, as the semantic model records it.
///
/// `tree` is a parse of `source_code`; `file_path` names the file that parse
/// came from and seeds every entity id; `config` says which node kinds of the
/// language are entities, which of them contain others, and which calls declare
/// something. The result holds one [`SemanticEntity`] per declaration found, in
/// the order the walk reached it, with a nested declaration recorded after the
/// one that holds it and pointing back at it through
/// [`SemanticEntity::parent_id`]. A file the language declares nothing in reads
/// as an empty list.
///
/// This is [`extract_entity_nodes`] with the syntax nodes dropped. Call this one
/// when the model is the whole answer — indexing, hashing, diffing. Call
/// [`extract_entity_nodes`] instead when the reader also needs the parse an
/// entity came from, to read something [`SemanticEntity`] does not carry, such
/// as a declaration's visibility or the text of its header.
pub fn extract_entities(
    tree: &Tree,
    file_path: &str,
    config: &LanguageConfig,
    source_code: &str,
) -> Vec<SemanticEntity> {
    extract_entity_nodes(tree, file_path, config, source_code)
        .into_iter()
        .map(|(entity, _node)| entity)
        .collect()
}

/// Every entity in `tree`, each paired with the syntax node it was read from.
///
/// The one traversal both entity consumers share. A reader that needs more of a
/// declaration than [`SemanticEntity`] carries — its visibility, the text of its
/// header — reads it off the node here instead of walking the tree a second
/// time, which would let the two walks disagree about what an entity is.
pub fn extract_entity_nodes<'tree>(
    tree: &'tree Tree,
    file_path: &str,
    config: &LanguageConfig,
    source_code: &str,
) -> Vec<(SemanticEntity, Node<'tree>)> {
    let walk = EntityWalk {
        file_path,
        config,
        source: source_code.as_bytes(),
    };
    let mut entities = Vec::new();
    walk.visit(tree.root_node(), None, &mut entities);
    entities
}

/// The inputs one traversal carries unchanged from its root down to every node
/// it reaches.
///
/// Held together so a step of the walk takes only what actually changes as it
/// descends: the node, the entity that holds it, and the entities found so far.
struct EntityWalk<'a> {
    /// The file the parse came from, which seeds every entity id.
    file_path: &'a str,
    /// The language's entity, container, and declaring-call vocabularies.
    config: &'a LanguageConfig,
    /// The parsed source, for reading a node's text.
    source: &'a [u8],
}

impl EntityWalk<'_> {
    /// Read every entity at or under `node` into `entities`, each recorded
    /// under `parent_id`.
    ///
    /// Exactly one of the readings below claims `node`. A node that IS an
    /// entity is recorded and its own children walked as that entity's; a node
    /// that only wraps a declaration is followed through; and a node that is
    /// neither passes its children on unchanged.
    fn visit<'tree>(
        &self,
        node: Node<'tree>,
        parent_id: Option<&str>,
        entities: &mut Vec<(SemanticEntity, Node<'tree>)>,
    ) {
        if self.read_declaring_call(node, parent_id, entities) {
            return;
        }
        if self.read_declaration(node, parent_id, entities) {
            return;
        }
        if self.follow_export(node, parent_id, entities) {
            return;
        }
        self.visit_children(node, parent_id, entities);
    }

    /// Record `node` when it is a call the language declares things with —
    /// Elixir's `def`, `defmodule` and the rest. Answers whether it was one.
    fn read_declaring_call<'tree>(
        &self,
        node: Node<'tree>,
        parent_id: Option<&str>,
        entities: &mut Vec<(SemanticEntity, Node<'tree>)>,
    ) -> bool {
        if node.kind() != "call" || self.config.call_entity_identifiers.is_empty() {
            return false;
        }
        let Some((name, entity_type)) = extract_call_entity(node, self.config, self.source) else {
            return false;
        };
        self.record(node, &name, entity_type, parent_id, entities);
        true
    }

    /// Record `node` when the language's entity vocabulary names its kind and
    /// the declaration names itself. Answers whether it was one.
    fn read_declaration<'tree>(
        &self,
        node: Node<'tree>,
        parent_id: Option<&str>,
        entities: &mut Vec<(SemanticEntity, Node<'tree>)>,
    ) -> bool {
        if !self.config.entity_node_types.contains(&node.kind()) {
            return false;
        }
        let Some(name) = extract_name(node, self.source) else {
            return false;
        };
        let entity_type = if node.kind() == DECORATED_DEFINITION_KIND {
            map_decorated_type(node)
        } else {
            map_node_type(node.kind())
        };
        self.record(node, &name, entity_type, parent_id, entities);
        true
    }

    /// Follow an export statement to the declaration it exports, so the
    /// declaration rather than the wrapper is the entity. Answers whether
    /// `node` was one.
    fn follow_export<'tree>(
        &self,
        node: Node<'tree>,
        parent_id: Option<&str>,
        entities: &mut Vec<(SemanticEntity, Node<'tree>)>,
    ) -> bool {
        if node.kind() != "export_statement" {
            return false;
        }
        let Some(declaration) = node.child_by_field_name("declaration") else {
            return false;
        };
        self.visit(declaration, parent_id, entities);
        true
    }

    /// Record `node` as one entity called `name`, then walk the entities
    /// declared inside it.
    fn record<'tree>(
        &self,
        node: Node<'tree>,
        name: &str,
        entity_type: &str,
        parent_id: Option<&str>,
        entities: &mut Vec<(SemanticEntity, Node<'tree>)>,
    ) {
        let content = node_text(node, self.source).to_string();
        let entity = SemanticEntity {
            id: build_entity_id(self.file_path, entity_type, name, parent_id),
            file_path: self.file_path.to_string(),
            entity_type: entity_type.to_string(),
            name: name.to_string(),
            parent_id: parent_id.map(String::from),
            content_hash: content_hash(&content),
            structural_hash: Some(structural_hash(node, self.source)),
            content,
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            metadata: None,
        };

        let entity_id = entity.id.clone();
        entities.push((entity, node));
        self.visit_contained(node, &entity_id, entities);
    }

    /// Walk the entities `node`'s container children hold — the methods of a
    /// class, the functions of an Elixir module — recorded under `parent_id`.
    fn visit_contained<'tree>(
        &self,
        node: Node<'tree>,
        parent_id: &str,
        entities: &mut Vec<(SemanticEntity, Node<'tree>)>,
    ) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if !self.config.container_node_types.contains(&child.kind()) {
                continue;
            }
            let mut inner_cursor = child.walk();
            for nested in child.named_children(&mut inner_cursor) {
                self.visit(nested, Some(parent_id), entities);
            }
        }
    }

    /// Walk `node`'s children as entities of whatever holds `node` itself,
    /// because `node` declares nothing of its own.
    fn visit_children<'tree>(
        &self,
        node: Node<'tree>,
        parent_id: Option<&str>,
        entities: &mut Vec<(SemanticEntity, Node<'tree>)>,
    ) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit(child, parent_id, entities);
        }
    }
}

/// The field a grammar gives a declaration's own identifier.
const NAME_FIELD: &str = "name";

/// The field a C-family grammar gives the thing a declaration declares, which
/// is where the name sits when the declaration itself carries no
/// [`NAME_FIELD`].
const DECLARATOR_FIELD: &str = "declarator";

/// The node kind of a bare name.
const IDENTIFIER_KIND: &str = "identifier";

/// The node kind of a bare name in type position.
const TYPE_IDENTIFIER_KIND: &str = "type_identifier";

/// The node kind Python gives a `def` or `class` that carries decorators.
const DECORATED_DEFINITION_KIND: &str = "decorated_definition";

/// Node kinds that declare a variable through `variable_declarator` children
/// rather than through a [`NAME_FIELD`] of their own — JavaScript and
/// TypeScript `let`, `const`, and `var`.
const VARIABLE_DECLARATION_KINDS: &[&str] = &["lexical_declaration", "variable_declaration"];

/// Node kinds whose name sits inside their [`DECLARATOR_FIELD`] — the C family,
/// where `int *make(void)` names a declarator chain rather than an identifier.
const DECLARATOR_NAMED_KINDS: &[&str] = &["function_definition", "declaration", "type_definition"];

/// Node kinds a Python `decorated_definition` stands above.
const DECORATED_INNER_KINDS: &[&str] = &["function_definition", "class_definition"];

/// The name the declaration at `node` gives itself, `None` when it names
/// nothing.
///
/// A [`NAME_FIELD`] answers for every grammar that has one, so the readings
/// under it are only for the kinds that spell a name some other way. A kind
/// that spells one nowhere — and a kind whose own reading came up empty — falls
/// back to the first identifier among the children.
fn extract_name(node: Node, source: &[u8]) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name(NAME_FIELD) {
        return Some(node_text(name_node, source).to_string());
    }
    declared_name(node, source).or_else(|| first_identifier_name(node, source))
}

/// The name a declaration spells somewhere other than a [`NAME_FIELD`], `None`
/// when its kind spells one nowhere.
fn declared_name(node: Node, source: &[u8]) -> Option<String> {
    let kind = node.kind();
    if VARIABLE_DECLARATION_KINDS.contains(&kind) {
        return variable_declarator_name(node, source);
    }
    if kind == DECORATED_DEFINITION_KIND {
        return decorated_definition_name(node, source);
    }
    if DECLARATOR_NAMED_KINDS.contains(&kind) {
        return node
            .child_by_field_name(DECLARATOR_FIELD)
            .and_then(|declarator| extract_declarator_name(declarator, source));
    }
    if kind == "template_declaration" {
        return template_declaration_name(node, source);
    }
    None
}

/// The name the first named declarator of a `let`/`const`/`var` declaration
/// carries.
fn variable_declarator_name(node: Node, source: &[u8]) -> Option<String> {
    find_in_named_children(node, |child| {
        if child.kind() != "variable_declarator" {
            return None;
        }
        let name = child.child_by_field_name(NAME_FIELD)?;
        Some(node_text(name, source).to_string())
    })
}

/// The name of the `def` or `class` a Python decorated definition stands above.
fn decorated_definition_name(node: Node, source: &[u8]) -> Option<String> {
    find_in_named_children(node, |child| {
        if !DECORATED_INNER_KINDS.contains(&child.kind()) {
            return None;
        }
        let name = child.child_by_field_name(NAME_FIELD)?;
        Some(node_text(name, source).to_string())
    })
}

/// The name of the declaration a C++ `template` stands above, which is a
/// sibling of the template's parameter list.
fn template_declaration_name(node: Node, source: &[u8]) -> Option<String> {
    find_in_named_children(node, |child| {
        if child.kind() == "template_parameter_list" {
            return None;
        }
        if let Some(name) = child.child_by_field_name(NAME_FIELD) {
            return Some(node_text(name, source).to_string());
        }
        let declarator = child.child_by_field_name(DECLARATOR_FIELD)?;
        extract_declarator_name(declarator, source)
    })
}

/// The first identifier among `node`'s named children — the reading for a
/// declaration whose grammar names it nowhere else.
fn first_identifier_name(node: Node, source: &[u8]) -> Option<String> {
    find_in_named_children(node, |child| {
        is_identifier(child).then(|| node_text(child, source).to_string())
    })
}

/// Whether `node` is a bare name, in value or in type position.
fn is_identifier(node: Node) -> bool {
    node.kind() == IDENTIFIER_KIND || node.kind() == TYPE_IDENTIFIER_KIND
}

/// The first value `read` yields for a named child of `node`.
///
/// The cursor a child walk needs is created and dropped here, so a caller reads
/// the children it wants as one expression instead of as a loop.
fn find_in_named_children<'tree, T>(
    node: Node<'tree>,
    read: impl FnMut(Node<'tree>) -> Option<T>,
) -> Option<T> {
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find_map(read);
    drop(cursor);
    found
}

/// Extract the name from a C declarator (handles pointer_declarator, function_declarator, etc.)
fn extract_declarator_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        IDENTIFIER_KIND | TYPE_IDENTIFIER_KIND | "field_identifier" => {
            Some(node_text(node, source).to_string())
        }
        "qualified_identifier" | "scoped_identifier" => {
            // For C++ qualified names like ClassName::method, return the full qualified name
            Some(node_text(node, source).to_string())
        }
        "pointer_declarator"
        | "function_declarator"
        | "array_declarator"
        | "parenthesized_declarator" => match node.child_by_field_name(DECLARATOR_FIELD) {
            Some(inner) => extract_declarator_name(inner, source),
            None => first_identifier_name(node, source),
        },
        _ => match node.child_by_field_name(NAME_FIELD) {
            Some(name) => Some(node_text(name, source).to_string()),
            None => first_identifier_name(node, source),
        },
    }
}

/// The text `node` spans, empty when the span is not valid UTF-8.
///
/// This copy takes BYTES and VALIDATES the UTF-8. The same-named copies in
/// `complexity` and `duplication` take `&str` and slice it, which fails on a
/// codepoint boundary instead. The four contracts, and why one cannot serve
/// them all, are recorded in the `parser::plugins::code` module doc.
fn node_text<'a>(node: Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// The semantic model's name for a tree-sitter node kind, which is the kind
/// itself when no language spells that concept differently.
fn map_node_type(tree_sitter_type: &str) -> &str {
    match tree_sitter_type {
        "function_declaration" | "function_definition" | "function_item" => "function",
        "method_declaration" | "method_definition" | "method" | "singleton_method" => "method",
        "class_declaration" | "class_definition" | "class_specifier" => "class",
        "interface_declaration" => "interface",
        "type_alias_declaration" | "type_declaration" | "type_item" | "type_definition" => "type",
        "enum_declaration" | "enum_item" | "enum_specifier" => "enum",
        "struct_item" | "struct_specifier" | "struct_declaration" => "struct",
        "union_specifier" => "union",
        "impl_item" => "impl",
        "trait_item" | "trait_declaration" => "trait",
        "mod_item" | "module" | "namespace_definition" | "namespace_declaration" => "module",
        "export_statement" => "export",
        "lexical_declaration" | "variable_declaration" | "var_declaration" | "declaration" => {
            "variable"
        }
        "const_declaration" | "const_item" => "constant",
        "static_item" => "static",
        "decorated_definition" => "decorated_definition",
        "constructor_declaration" => "constructor",
        "field_declaration" | "public_field_definition" | "field_definition" => "field",
        "property_declaration" => "property",
        "annotation_type_declaration" => "annotation",
        "template_declaration" => "template",
        other => other,
    }
}

/// Extract entity info from a call node (Elixir macros like def, defmodule, etc.)
fn extract_call_entity(
    node: Node,
    config: &LanguageConfig,
    source: &[u8],
) -> Option<(String, &'static str)> {
    let target = node.child_by_field_name("target")?;
    if target.kind() != IDENTIFIER_KIND {
        return None;
    }
    let keyword = node_text(target, source);

    if !config.call_entity_identifiers.contains(&keyword) {
        return None;
    }

    let entity_type = match keyword {
        "defmodule" => "module",
        "def" | "defp" | "defdelegate" => "function",
        "defmacro" | "defmacrop" => "macro",
        "defguard" | "defguardp" => "guard",
        "defprotocol" => "protocol",
        "defimpl" => "impl",
        "defstruct" => "struct",
        "defexception" => "exception",
        _ => return None,
    };

    // Get arguments node (child by kind, not field name)
    let mut cursor = node.walk();
    let args = node
        .named_children(&mut cursor)
        .find(|c| c.kind() == "arguments")?;

    let name = match keyword {
        "defmodule" | "defprotocol" => extract_first_alias_or_identifier(args, source)?,
        "defimpl" => {
            let base = extract_first_alias_or_identifier(args, source)?;
            if let Some(target) = extract_keyword_value(args, "for", source) {
                format!("{} for {}", base, target)
            } else {
                base
            }
        }
        "defstruct" => "__struct__".to_string(),
        "defexception" => "__exception__".to_string(),
        _ => {
            // def, defp, defmacro, defguard, defdelegate
            // First arg is a call (fn with params), identifier (arity-0),
            // or binary_operator (defguard with when clause)
            let mut cursor = args.walk();
            let first_arg = args.named_children(&mut cursor).next()?;
            extract_fn_name_from_arg(first_arg, source)?
        }
    };

    Some((name, entity_type))
}

/// Extract function name from a def/defp/defmacro/defguard argument.
/// Handles: call (fn with params), identifier (arity-0), binary_operator (defguard when clause)
fn extract_fn_name_from_arg(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "call" => match node.child_by_field_name("target") {
            Some(fn_target) => Some(node_text(fn_target, source).to_string()),
            None => find_in_named_children(node, |child| {
                (child.kind() == IDENTIFIER_KIND).then(|| node_text(child, source).to_string())
            }),
        },
        IDENTIFIER_KIND => Some(node_text(node, source).to_string()),
        "binary_operator" => {
            // defguard is_positive(x) when ... -> left side has the actual call/identifier
            let left = node.child_by_field_name("left")?;
            extract_fn_name_from_arg(left, source)
        }
        _ => None,
    }
}

/// The first module alias or bare name among a call's arguments — the name an
/// Elixir `defmodule` or `defprotocol` declares.
fn extract_first_alias_or_identifier(args: Node, source: &[u8]) -> Option<String> {
    find_in_named_children(args, |child| {
        matches!(child.kind(), "alias" | IDENTIFIER_KIND)
            .then(|| node_text(child, source).to_string())
    })
}

/// The value a call's keyword arguments give `key` — the `for:` of Elixir's
/// `defimpl Protocol, for: Target`.
fn extract_keyword_value(args: Node, key: &str, source: &[u8]) -> Option<String> {
    find_in_named_children(args, |child| keyword_list_value(child, key, source))
}

/// The value `node` gives `key`, `None` when `node` is not a keyword list or
/// holds no such key.
fn keyword_list_value(node: Node, key: &str, source: &[u8]) -> Option<String> {
    if node.kind() != "keywords" {
        return None;
    }
    find_in_named_children(node, |pair| pair_value_for_key(pair, key, source))
}

/// `pair`'s value when `pair` is the keyword pair for `key`, `None` otherwise.
///
/// A pair spells its key either with the trailing colon the source carries
/// (`for:`) or without it, depending on how the grammar cut the key node, so
/// both spellings are accepted.
fn pair_value_for_key(pair: Node, key: &str, source: &[u8]) -> Option<String> {
    if pair.kind() != "pair" {
        return None;
    }
    let pair_key = pair.child_by_field_name("key")?;
    let key_text = node_text(pair_key, source).trim();
    if key_text != format!("{key}:") && key_text != key {
        return None;
    }
    let pair_value = pair.child_by_field_name("value")?;
    Some(node_text(pair_value, source).to_string())
}

/// For Python decorated_definition, check the inner node to determine the real type.
fn map_decorated_type(node: Node) -> &'static str {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "class_definition" => return "class",
            "function_definition" => return "function",
            _ => {}
        }
    }
    "function"
}
