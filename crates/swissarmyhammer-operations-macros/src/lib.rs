//! Procedural macros for defining operations
//!
//! This crate provides the `#[operation]` and `#[param]` attribute macros
//! for defining operations with metadata, and the `operation_tool!`
//! function-like macro for declaring a self-describing operation tool.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Attribute, DeriveInput, Expr, Field, Ident, Lit, LitStr, Meta, Token, Type,
};

/// Attribute macro for defining an operation
///
/// # Usage
///
/// ```ignore
/// #[operation(verb = "add", noun = "task", description = "Create a new task")]
/// #[derive(Debug, Deserialize)]
/// pub struct AddTask {
///     /// The task title
///     #[param(short = 't', alias = "name")]
///     pub title: String,
///
///     /// Optional description
///     #[param(alias = "desc")]
///     pub description: Option<String>,
/// }
/// ```
#[proc_macro_attribute]
pub fn operation(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as OperationArgs);
    let input = parse_macro_input!(item as DeriveInput);

    let name = &input.ident;
    let verb = &args.verb;
    let noun = &args.noun;
    let description = &args.description;

    // Extract field metadata (handle unit structs and named fields)
    let param_metas: Vec<_> = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => fields.named.iter().map(generate_param_meta).collect(),
            syn::Fields::Unit => Vec::new(), // Unit struct has no fields
            syn::Fields::Unnamed(_) => panic!("operation macro does not support tuple structs"),
        },
        _ => panic!("operation macro only supports structs"),
    };

    let num_params = param_metas.len();

    let expanded = quote! {
        #input

        impl swissarmyhammer_operations::Operation for #name {
            fn verb(&self) -> &'static str {
                #verb
            }

            fn noun(&self) -> &'static str {
                #noun
            }

            fn description(&self) -> &'static str {
                #description
            }

            fn parameters(&self) -> &'static [swissarmyhammer_operations::ParamMeta] {
                static PARAMS: [swissarmyhammer_operations::ParamMeta; #num_params] = [
                    #(#param_metas),*
                ];
                &PARAMS
            }
        }
    };

    TokenStream::from(expanded)
}

/// Arguments for the #[operation(...)] attribute
struct OperationArgs {
    verb: String,
    noun: String,
    description: String,
}

impl Parse for OperationArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut verb = None;
        let mut noun = None;
        let mut description = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: Lit = input.parse()?;

            let value_str = match value {
                Lit::Str(s) => s.value(),
                _ => return Err(syn::Error::new_spanned(value, "expected string literal")),
            };

            match ident.to_string().as_str() {
                "verb" => verb = Some(value_str),
                "noun" => noun = Some(value_str),
                "description" => description = Some(value_str),
                other => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!("unknown attribute: {}", other),
                    ))
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(OperationArgs {
            verb: verb.ok_or_else(|| input.error("missing 'verb' attribute"))?,
            noun: noun.ok_or_else(|| input.error("missing 'noun' attribute"))?,
            description: description
                .ok_or_else(|| input.error("missing 'description' attribute"))?,
        })
    }
}

/// Generate ParamMeta for a field
fn generate_param_meta(field: &Field) -> proc_macro2::TokenStream {
    let name = field.ident.as_ref().unwrap().to_string();

    // Extract doc comment as description
    let description = extract_doc_comment(&field.attrs);

    // Check if type is Option<T> to determine required
    let required = !is_option_type(&field.ty);

    // Determine param type from Rust type
    let param_type = rust_type_to_param_type(&field.ty);

    // Extract #[param(...)] attributes
    let (short, aliases) = extract_param_attrs(&field.attrs);

    let short_expr = match short {
        Some(c) => quote! { Some(#c) },
        None => quote! { None },
    };

    let required_call = if required {
        quote! { .required() }
    } else {
        quote! {}
    };

    quote! {
        swissarmyhammer_operations::ParamMeta::new(#name)
            .description(#description)
            .param_type(#param_type)
            #required_call
            .short_opt(#short_expr)
            .aliases(&[#(#aliases),*])
    }
}

/// Extract doc comment from attributes
fn extract_doc_comment(attrs: &[Attribute]) -> String {
    let docs: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let Meta::NameValue(nv) = &attr.meta {
                    if let Expr::Lit(lit) = &nv.value {
                        if let Lit::Str(s) = &lit.lit {
                            return Some(s.value().trim().to_string());
                        }
                    }
                }
            }
            None
        })
        .collect();

    docs.join(" ")
}

/// Check if type is Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Convert Rust type to ParamType
fn rust_type_to_param_type(ty: &Type) -> proc_macro2::TokenStream {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            if segment.ident == "Option" {
                // Extract inner type from Option<T>
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return rust_type_to_param_type(inner);
                    }
                }
            } else if segment.ident == "Vec" {
                return quote! { swissarmyhammer_operations::ParamType::Array };
            } else if segment.ident == "String" || segment.ident == "str" {
                return quote! { swissarmyhammer_operations::ParamType::String };
            } else if segment.ident == "bool" {
                return quote! { swissarmyhammer_operations::ParamType::Boolean };
            } else if segment.ident == "i32"
                || segment.ident == "i64"
                || segment.ident == "u32"
                || segment.ident == "u64"
                || segment.ident == "usize"
                || segment.ident == "isize"
            {
                return quote! { swissarmyhammer_operations::ParamType::Integer };
            } else if segment.ident == "f32" || segment.ident == "f64" {
                return quote! { swissarmyhammer_operations::ParamType::Number };
            }
        }
    }
    // Default to String for unknown types
    quote! { swissarmyhammer_operations::ParamType::String }
}

/// Extract #[param(...)] attributes
fn extract_param_attrs(attrs: &[Attribute]) -> (Option<char>, Vec<String>) {
    let mut short = None;
    let mut aliases = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("param") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("short") {
                    meta.input.parse::<Token![=]>()?;
                    let lit: Lit = meta.input.parse()?;
                    if let Lit::Char(c) = lit {
                        short = Some(c.value());
                    }
                } else if meta.path.is_ident("alias") {
                    meta.input.parse::<Token![=]>()?;
                    let lit: Lit = meta.input.parse()?;
                    if let Lit::Str(s) = lit {
                        aliases.push(s.value());
                    }
                }
                Ok(())
            });
        }
    }

    (short, aliases)
}

/// Dummy proc macro for field-level param attributes
/// This is just a marker attribute, the operation macro reads it
#[proc_macro_attribute]
pub fn param(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Just pass through the item unchanged
    // The #[operation] macro reads these attributes
    item
}

/// Function-like macro for declaring a self-describing operation tool.
///
/// `operation_tool!` builds an [`rmcp::model::Tool`] definition directly from a
/// set of operations. The generated code derives **both** the flat wire schema
/// (via `swissarmyhammer_operations::generate_mcp_schema`) and the
/// `io.swissarmyhammer/operations` discovery `_meta` tree (via
/// `swissarmyhammer_operations::generate_operations_meta`) from the *same*
/// operation slice. A tool author writes the operation structs plus this
/// invocation and never hand-assembles `_meta`, so the discovery metadata can
/// never drift from the operation definitions.
///
/// The wire contract is unchanged — `op` remains the single selector and the
/// `tools/call` handler stays an `op` match. The `_meta` is purely additive
/// discovery metadata.
///
/// # Usage
///
/// ```ignore
/// use rmcp::model::Tool;
/// use swissarmyhammer_operations::{operation, operation_tool, Operation};
///
/// #[operation(verb = "add", noun = "task", description = "Create a new task")]
/// struct AddTask {
///     /// The task title
///     title: String,
/// }
///
/// #[operation(verb = "get", noun = "task", description = "Get a task by id")]
/// struct GetTask {
///     /// The task id
///     id: String,
/// }
///
/// fn operations() -> Vec<&'static dyn Operation> {
///     vec![
///         Box::leak(Box::new(AddTask { title: String::new() })) as &dyn Operation,
///         Box::leak(Box::new(GetTask { id: String::new() })) as &dyn Operation,
///     ]
/// }
///
/// let tool: Tool = operation_tool! {
///     name: "kanban",
///     description: "Kanban board operations",
///     operations: operations(),
/// };
/// // tool.input_schema["properties"]["op"]["enum"] lists "add task" / "get task"
/// // tool.meta["io.swissarmyhammer/operations"]["task"]["add"]["op"] == "add task"
/// ```
///
/// # Arguments
///
/// The macro accepts three named fields, in any order, comma-separated, with an
/// optional trailing comma:
///
/// * `name` - the tool name, a string literal
/// * `description` - the tool description, a string literal
/// * `operations` - an expression evaluating to a value that coerces to
///   `&[&dyn swissarmyhammer_operations::Operation]` (e.g. a
///   `Vec<&dyn Operation>` or array).
#[proc_macro]
pub fn operation_tool(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as OperationToolArgs);

    let name = &args.name;
    let description = &args.description;
    let operations = &args.operations;

    // The generated code references the runtime crates by path, exactly as the
    // `#[operation]` macro references `swissarmyhammer_operations::Operation`.
    // The proc-macro crate itself never links them.
    let expanded = quote! {
        {
            // Bind the operation set once so both generators see the identical
            // slice — there is a single source of truth for `_meta`.
            let __operations: &[&dyn swissarmyhammer_operations::Operation] = &#operations;

            // Flat wire schema: `op` enum plus all parameters.
            let __schema = swissarmyhammer_operations::generate_mcp_schema(
                __operations,
                swissarmyhammer_operations::SchemaConfig::new(#description),
            );
            let __schema_map = match __schema {
                ::serde_json::Value::Object(map) => map,
                _ => ::serde_json::Map::new(),
            };

            // Discovery `_meta`: the noun -> verb -> { op, ... } tree, attached
            // under the well-known `io.swissarmyhammer/operations` key.
            let __ops_meta = swissarmyhammer_operations::generate_operations_meta(__operations);
            let mut __meta = ::rmcp::model::Meta::new();
            __meta.0.insert(
                "io.swissarmyhammer/operations".to_string(),
                __ops_meta,
            );

            let mut __tool = ::rmcp::model::Tool::new(#name, #description, __schema_map);
            __tool.meta = Some(__meta);
            __tool
        }
    };

    TokenStream::from(expanded)
}

/// Parsed arguments for the [`operation_tool!`] macro.
///
/// Holds the `name` and `description` string literals and the `operations`
/// expression. See [`operation_tool!`] for the accepted syntax.
struct OperationToolArgs {
    name: LitStr,
    description: LitStr,
    operations: Expr,
}

impl Parse for OperationToolArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let mut operations = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match ident.to_string().as_str() {
                "name" => name = Some(input.parse::<LitStr>()?),
                "description" => description = Some(input.parse::<LitStr>()?),
                "operations" => operations = Some(input.parse::<Expr>()?),
                other => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!(
                            "unknown field: {} (expected name, description, or operations)",
                            other
                        ),
                    ))
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(OperationToolArgs {
            name: name.ok_or_else(|| input.error("missing 'name' field"))?,
            description: description.ok_or_else(|| input.error("missing 'description' field"))?,
            operations: operations.ok_or_else(|| input.error("missing 'operations' field"))?,
        })
    }
}
