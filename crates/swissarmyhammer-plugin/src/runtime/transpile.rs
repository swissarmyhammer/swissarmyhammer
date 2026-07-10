//! TypeScript-to-JavaScript transpilation for plugin modules.
//!
//! Plugin entry modules are authored in TypeScript. Before a module can be
//! evaluated in a V8 isolate it must be reduced to plain JavaScript, because
//! V8 does not understand TypeScript syntax (type annotations, `interface`
//! declarations, `enum`s, and so on).
//!
//! This module performs that reduction with [`deno_ast`], which wraps the swc
//! compiler. The transform is purely **syntactic**: type annotations are
//! erased and TypeScript-only constructs are lowered to JavaScript, but no
//! type-checking is performed. A `.ts` source that is type-incorrect but
//! syntactically valid still transpiles cleanly and runs — type errors are a
//! separate concern from running plugin code.
//!
//! The emitted JavaScript carries an **inline** source map (a
//! `//# sourceMappingURL=data:...` trailer). V8 reads that trailer when it
//! builds stack traces and when a debugger attaches, so errors thrown by the
//! running plugin report original TypeScript line and column numbers rather
//! than positions in the generated JavaScript.

use deno_ast::{
    EmitOptions, MediaType, ModuleSpecifier, ParseParams, SourceMapOption, TranspileModuleOptions,
    TranspileOptions,
};

use crate::error::{Error, Result};

/// JavaScript produced by transpiling a TypeScript plugin module.
///
/// The [`code`](TranspiledModule::code) is plain JavaScript suitable for
/// evaluation in a V8 isolate. It ends with an inline source-map trailer, so
/// the original TypeScript source map is also surfaced separately as
/// [`source_map`](TranspiledModule::source_map) for callers that want to
/// register it with tooling (for example, the V8 Inspector) explicitly.
#[derive(Debug, Clone)]
pub struct TranspiledModule {
    /// Plain JavaScript with an inline `//# sourceMappingURL=` trailer.
    pub code: String,

    /// The standalone JSON source map mapping the emitted JavaScript back to
    /// the original TypeScript source.
    ///
    /// This is always populated: the transpiler is configured to emit a source
    /// map, so a successful transpile always yields one.
    pub source_map: String,
}

/// Transpile a TypeScript source string into JavaScript with a source map.
///
/// The transform erases types and lowers TypeScript-only syntax to JavaScript.
/// It does **not** type-check: a type-incorrect-but-syntactically-valid `.ts`
/// source transpiles successfully here and fails (if at all) only when the
/// resulting JavaScript runs.
///
/// The emitted JavaScript carries an inline source map so V8 stack traces and
/// attached debuggers report original TypeScript positions. The same source
/// map is also returned standalone in [`TranspiledModule::source_map`].
///
/// # Arguments
///
/// * `specifier` - The module URL the source is identified by. It appears in
///   the source map's `sources` array and in V8 stack frames.
/// * `source` - The TypeScript source text to transpile.
///
/// # Errors
///
/// Returns [`Error::Transpile`] if the source cannot be parsed (a genuine
/// *syntax* error, as distinct from a type error) or if emitting JavaScript
/// from the parsed program fails.
pub fn transpile_typescript(specifier: &ModuleSpecifier, source: &str) -> Result<TranspiledModule> {
    // Parse the source as a TypeScript module. `MediaType::TypeScript` selects
    // the TypeScript grammar; `parse_module` only reports syntax errors, never
    // type errors, which is exactly the "syntactic only" contract we want.
    let parsed = deno_ast::parse_module(ParseParams {
        specifier: specifier.clone(),
        text: source.into(),
        media_type: MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })
    .map_err(|e| Error::Transpile(format!("failed to parse module: {e}")))?;

    // Emit JavaScript and a source map. `SourceMapOption::Separate` makes the
    // emitter return the map as a standalone JSON string alongside the code
    // rather than appending a `//# sourceMappingURL=` trailer itself; this code
    // then inlines that map manually via `with_inline_source_map` below.
    // `inline_sources` embeds the original TypeScript text inside the map so a
    // debugger can show original source without a separate fetch.
    let emit_options = EmitOptions {
        source_map: SourceMapOption::Separate,
        inline_sources: true,
        ..Default::default()
    };

    let emitted = parsed
        .transpile(
            &TranspileOptions::default(),
            &TranspileModuleOptions::default(),
            &emit_options,
        )
        .map_err(|e| Error::Transpile(format!("failed to transpile module: {e}")))?
        .into_source();

    // With `SourceMapOption::Separate` the emitter returns the map alongside
    // the code rather than inlining it. Inline it ourselves so V8 picks it up
    // for stack traces and the inspector, and also keep the standalone copy.
    let source_map = emitted
        .source_map
        .ok_or_else(|| Error::Transpile("transpiler produced no source map".to_string()))?;

    let code = with_inline_source_map(&emitted.text, &source_map);

    Ok(TranspiledModule { code, source_map })
}

/// Append a base64-encoded inline source-map trailer to emitted JavaScript.
///
/// V8 recognizes a trailing `//# sourceMappingURL=data:application/json;base64,…`
/// comment and uses the decoded map when constructing stack traces and when a
/// debugger session inspects the module. Inlining the map keeps the runtime
/// self-contained: there is no separate `.map` file to resolve or serve.
fn with_inline_source_map(code: &str, source_map: &str) -> String {
    use base64::Engine as _;

    let encoded = base64::engine::general_purpose::STANDARD.encode(source_map.as_bytes());
    let separator = if code.ends_with('\n') { "" } else { "\n" };
    format!("{code}{separator}//# sourceMappingURL=data:application/json;base64,{encoded}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specifier() -> ModuleSpecifier {
        ModuleSpecifier::parse("file:///plugin/entry.ts").unwrap()
    }

    #[test]
    fn transpile_strips_type_annotations() {
        // `: number` and the return type are TypeScript-only syntax that V8
        // cannot parse; after transpilation they must be gone.
        let ts = "export function add(a: number, b: number): number { return a + b; }";
        let out = transpile_typescript(&specifier(), ts).expect("transpile should succeed");
        assert!(
            !out.code.contains(": number"),
            "type annotations should be erased, got: {}",
            out.code
        );
        assert!(
            out.code.contains("function add"),
            "the function itself should survive transpilation"
        );
    }

    #[test]
    fn transpile_emits_inline_source_map() {
        let ts = "export const x: number = 1;";
        let out = transpile_typescript(&specifier(), ts).expect("transpile should succeed");
        assert!(
            out.code
                .contains("//# sourceMappingURL=data:application/json;base64,"),
            "emitted code should carry an inline source-map trailer"
        );
        assert!(
            !out.source_map.is_empty(),
            "a standalone source map should also be produced"
        );
    }

    #[test]
    fn transpile_lowers_typescript_only_constructs() {
        // `interface` and `enum` are TypeScript-only. The interface is erased
        // entirely; the enum is lowered to a JavaScript object.
        let ts = "interface Point { x: number; }\nexport enum Color { Red, Green }";
        let out = transpile_typescript(&specifier(), ts).expect("transpile should succeed");
        assert!(
            !out.code.contains("interface"),
            "interface declarations should be erased, got: {}",
            out.code
        );
    }

    #[test]
    fn transpile_rejects_genuine_syntax_errors() {
        // An unterminated function body is a real syntax error — not a type
        // error — so the transpiler must reject it.
        let ts = "export function broken( {";
        let result = transpile_typescript(&specifier(), ts);
        assert!(result.is_err(), "a syntax error should fail transpilation");
    }
}
