//! Shared test helpers for the builtin-skill rendering tests.
//!
//! These tests assert against the *rendered* skill bodies — the way the deploy
//! pipeline expands Liquid `{% include "_partials/<name>" %}` tags — and against
//! the `builtin/` source tree as the single source of truth for the guidance
//! text. The helpers below resolve the builtin skills, expand the partial
//! includes, and locate the repo-root `builtin/` directory. They are shared by
//! every `*_guidance.rs` test so the rendering logic has one source of truth.

use std::path::{Path, PathBuf};
use swissarmyhammer_skills::SkillResolver;

/// Repo-root `builtin/` directory — the source of truth for skills and partials.
pub fn builtin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ parent")
        .parent()
        .expect("repo root")
        .join("builtin")
}

/// Strip a leading `---` YAML frontmatter block, returning the markdown body.
pub fn strip_frontmatter(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("---\n") else {
        return content;
    };
    match rest.find("\n---\n") {
        Some(pos) => &rest[pos + "\n---\n".len()..],
        None => content,
    }
}

/// Expand `{% include "_partials/<name>" %}` tags with the partial bodies from
/// `builtin/_partials/`, mirroring what the Liquid renderer does at deploy time.
pub fn expand_partials(body: &str) -> String {
    let partials_dir = builtin_dir().join("_partials");
    let mut rendered = body.to_string();
    for entry in walkdir::WalkDir::new(&partials_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let name = path
            .strip_prefix(&partials_dir)
            .expect("partial under _partials/")
            .with_extension("");
        let tag = format!("{{% include \"_partials/{}\" %}}", name.to_string_lossy());
        if rendered.contains(&tag) {
            let content = std::fs::read_to_string(path).expect("readable partial");
            rendered = rendered.replace(&tag, strip_frontmatter(&content));
        }
    }
    rendered
}

/// Fetch a builtin skill body by name with partials expanded, failing the test
/// if the skill is missing.
pub fn rendered_builtin_instructions(name: &str) -> String {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    let skill = builtins
        .get(name)
        .unwrap_or_else(|| panic!("builtin skill '{name}' should exist"));
    expand_partials(&skill.instructions)
}

/// Assert that `canonical_text` appears in exactly one `builtin/` markdown file —
/// the partial at `expected_rel_path` (relative to `builtin/`).
///
/// Walks the repo-root `builtin/` tree, collects every `.md` file whose contents
/// contain `canonical_text`, and asserts the set of hits equals exactly
/// `[expected_rel_path]`. On failure the panic lists the offending files so a
/// duplicated-guidance regression is immediately diagnosable.
///
/// This is the single source of truth for the "guidance lives in exactly one
/// partial" check shared by every `*_guidance.rs` test.
pub fn assert_guidance_single_source(canonical_text: &str, expected_rel_path: &Path) {
    let builtin = builtin_dir();
    let mut hits: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(&builtin)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let content = std::fs::read_to_string(path).expect("readable builtin file");
        if content.contains(canonical_text) {
            hits.push(
                path.strip_prefix(&builtin)
                    .expect("under builtin/")
                    .to_path_buf(),
            );
        }
    }
    assert_eq!(
        hits,
        vec![expected_rel_path.to_path_buf()],
        "the guidance must live ONLY in builtin/{}, found in: {hits:?}",
        expected_rel_path.display()
    );
}
