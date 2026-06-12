//! Enforces that the work-the-card builtin skills (`implement`, `finish`,
//! `kanban`) instruct the agent to keep a conversation log on the task via
//! the kanban comment ops (`add comment` / `list comments`).
//!
//! The guidance lives in ONE shared partial —
//! `builtin/_partials/record-progress.md` — pulled into each skill via Liquid
//! `{% include "_partials/record-progress" %}`. These tests assert against the
//! *rendered* skill bodies (includes expanded the way the deploy pipeline
//! expands them) and that the canonical guidance text has exactly one source
//! of truth under `builtin/`.
//!
//! Failing this test means a skill body drifted and no longer tells agents to
//! leave a comment trail — or the guidance got duplicated outside the partial.

use std::path::PathBuf;
use swissarmyhammer_skills::SkillResolver;

/// A sentence that exists ONLY in `builtin/_partials/record-progress.md`.
/// Finding it in a rendered skill proves the include expanded; finding it in
/// more than one `builtin/` source file means the guidance was duplicated.
const CANONICAL_GUIDANCE: &str = "burn the same tokens repeating them";

/// Repo-root `builtin/` directory — the source of truth for skills and partials.
fn builtin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/ parent")
        .parent()
        .expect("repo root")
        .join("builtin")
}

/// Strip a leading `---` YAML frontmatter block, returning the markdown body.
fn strip_frontmatter(content: &str) -> &str {
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
fn expand_partials(body: &str) -> String {
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
fn rendered_builtin_instructions(name: &str) -> String {
    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    let skill = builtins
        .get(name)
        .unwrap_or_else(|| panic!("builtin skill '{name}' should exist"));
    expand_partials(&skill.instructions)
}

/// Every work-the-card skill's rendered body must carry the full record-progress
/// guidance: the `add comment` op, reading prior context with `list comments`,
/// and explicit instructions to record failures, discoveries, and blockers —
/// not just milestones.
#[test]
fn work_the_card_skills_render_record_progress_guidance() {
    for name in ["implement", "finish", "kanban"] {
        let body = rendered_builtin_instructions(name);
        assert!(
            !body.contains("{% include \"_partials/record-progress\" %}"),
            "builtin skill '{name}' must expand the record-progress include"
        );
        assert!(
            body.contains("add comment"),
            "builtin skill '{name}' must instruct recording progress via the `add comment` op"
        );
        assert!(
            body.contains("list comments"),
            "builtin skill '{name}' must instruct reading prior context via `list comments`"
        );
        for (marker, what) in [
            ("Milestones", "milestones"),
            ("did not work", "failed approaches / dead ends"),
            ("discoveries", "interesting discoveries"),
            ("Blockers", "blockers"),
        ] {
            assert!(
                body.contains(marker),
                "builtin skill '{name}' must instruct recording {what} (marker '{marker}')"
            );
        }
        assert!(
            body.contains(CANONICAL_GUIDANCE),
            "builtin skill '{name}' must render the canonical record-progress guidance"
        );
    }
}

/// The canonical guidance sentence must exist in exactly one `builtin/` source
/// file — the shared partial. Any duplication means the skills drifted back to
/// inlined copies.
#[test]
fn record_progress_guidance_has_single_source_of_truth() {
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
        if content.contains(CANONICAL_GUIDANCE) {
            hits.push(
                path.strip_prefix(&builtin)
                    .expect("under builtin/")
                    .to_path_buf(),
            );
        }
    }
    assert_eq!(
        hits,
        vec![PathBuf::from("_partials/record-progress.md")],
        "the record-progress guidance must live ONLY in builtin/_partials/record-progress.md, found in: {hits:?}"
    );
}
