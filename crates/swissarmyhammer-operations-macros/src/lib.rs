//! Procedural macros for defining operations
//!
//! This crate provides the `#[operation]` and `#[param]` attribute macros
//! for defining operations with metadata.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Attribute, DeriveInput, Expr, Field, Ident, Lit, Meta, Token, Type,
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

    // A field is required only when it is a non-Option type AND carries no
    // serde default. A `#[serde(default ...)]` field is filled in by serde when
    // the key is absent, so it is genuinely optional at dispatch even though its
    // Rust type is not `Option<T>` (e.g. `Vec<T>`, `bool`, `HashMap`). Marking
    // such a field required would make a schema-honoring CLI reject valid
    // no-arg invocations.
    let required = !is_option_type(&field.ty) && !has_serde_default(&field.attrs);

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

/// Check if a field carries a serde default (`#[serde(default)]` or
/// `#[serde(default = "...")]`, possibly alongside other serde options).
///
/// A serde-defaulted field is optional at dispatch — serde supplies the value
/// when the input omits the key — so it must not be reported as `required` in
/// the operation schema, regardless of whether its Rust type is `Option<T>`.
fn has_serde_default(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("serde") {
            return false;
        }
        // Scan the `#[serde(...)]` argument token stream directly for a top-level
        // `default` identifier. We deliberately do NOT use `parse_nested_meta`
        // here: serde also accepts list-valued items like
        // `rename(serialize = "x")` and `bound(...)`, whose nested `(...)` group
        // `parse_nested_meta`'s closure cannot consume, so the walk errors at that
        // token and aborts *before* it ever reaches a later `default`. That false
        // negative wrongly marks a defaulted field as required — the exact bug this
        // function exists to prevent.
        //
        // A flat ident scan over the top-level tokens can't be aborted by a
        // sibling item's shape, so a `default` anywhere in the list is detected
        // regardless of what precedes it. Only top-level idents count: a `default`
        // nested inside a `(...)` group (such as `rename(...)`) arrives as a single
        // opaque `TokenTree::Group`, so its contents cannot trigger a false
        // positive.
        let Meta::List(list) = &attr.meta else {
            return false;
        };
        list.tokens
            .clone()
            .into_iter()
            .any(|tt| matches!(tt, proc_macro2::TokenTree::Ident(ident) if ident == "default"))
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    /// A bare `#[serde(default)]` must be detected.
    #[test]
    fn bare_default_is_detected() {
        let attr: Attribute = parse_quote!(#[serde(default)]);
        assert!(has_serde_default(&[attr]));
    }

    /// A `#[serde(default = "fn")]` must be detected.
    #[test]
    fn valued_default_is_detected() {
        let attr: Attribute = parse_quote!(#[serde(default = "make_default")]);
        assert!(has_serde_default(&[attr]));
    }

    /// A serde attribute with no `default` item must NOT be detected.
    #[test]
    fn no_default_is_not_detected() {
        let attr: Attribute = parse_quote!(#[serde(skip_serializing_if = "Option::is_none")]);
        assert!(!has_serde_default(&[attr]));
    }

    /// Regression: a list-valued serde item (`rename(serialize = "x")`)
    /// preceding `default` must NOT mask the `default`. The old
    /// `parse_nested_meta` walk returned `Err` on the unconsumed `(...)` group
    /// and aborted before reaching `default`, wrongly marking the field
    /// required. `default` anywhere in the serde list must be detected
    /// regardless of other list-valued items.
    #[test]
    fn list_valued_item_before_default_does_not_mask_it() {
        let attr: Attribute = parse_quote!(#[serde(rename(serialize = "x"), default)]);
        assert!(
            has_serde_default(&[attr]),
            "`default` after a list-valued serde item must still be detected"
        );
    }

    /// A `bound(...)` list-valued item with nested parens preceding `default`
    /// must also not mask it.
    #[test]
    fn bound_item_before_default_does_not_mask_it() {
        let attr: Attribute = parse_quote!(#[serde(bound(deserialize = "T: Clone"), default)]);
        assert!(has_serde_default(&[attr]));
    }

    /// A non-serde attribute is ignored entirely.
    #[test]
    fn non_serde_attribute_ignored() {
        let attr: Attribute = parse_quote!(#[doc = "a comment"]);
        assert!(!has_serde_default(&[attr]));
    }
}
