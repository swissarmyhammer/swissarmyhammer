//! Tests for [cognitive complexity](super).
//!
//! The tests are split by language, one module for each [`ComplexitySpec`]
//! row, because that is the unit the scorer is written in. The review engine
//! renders a whole file into one agent prompt, and a file over the per-file
//! prompt cap is not reviewed at all, so a test tree this size has to be
//! several files rather than one.
//!
//! - [`rust`] — the reference language: match arms, chains, nesting, boolean
//!   runs, labelled jumps, the test exemption, and score determinism.
//! - [`typescript_family`] — TypeScript, TSX and JavaScript, the three rows
//!   that share `typescript_family_spec`.
//! - [`python`], [`java`], [`csharp`], [`php`], [`go`], [`ruby`],
//!   [`fortran`], [`swift`], [`elixir`] — one row each.
//! - [`c_family`] — C and C++, the two rows that share `c_family_spec`.
//! - [`depth`] — the traversal-depth cap, against pathological nesting.
//!
//! This module carries what those thirteen share: the scoring helpers, the
//! determinism run count, and the two Rust source fixtures the depth tests
//! reuse.

mod c_family;
mod csharp;
mod depth;
mod elixir;
mod fortran;
mod go;
mod java;
mod php;
mod python;
mod ruby;
mod rust;
mod swift;
mod typescript_family;
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
