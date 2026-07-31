//! Coverage guard: every builtin agent and skill that writes or judges code must
//! carry the rule-obedience stance — a validator rule and a review finding are
//! requirements, not suggestions.
//!
//! The stance lives in ONE shared partial —
//! `builtin/_partials/findings-are-requirements.md` — pulled into each agent and
//! skill via Liquid `{% include "_partials/findings-are-requirements" %}`.
//!
//! These tests drive the **production** render path. `AgentResolver` and
//! `SkillResolver` return the raw body with the tag still in it, and
//! `TemplateLibrary::render_text` expands the include from the embedded
//! `builtin/_partials/` set. That is the pair of calls `install_profile_agents`
//! and `render_profile_skill` make, so this test also proves the `_partials/`
//! prefix resolves — both installer paths only log a warning and fall back to the
//! raw body when a render fails, which would silently ship the literal Liquid tag
//! to the model.
//!
//! Failing this test means an agent or skill dropped the include, so it no longer
//! tells the model that findings are mandatory.
//!
//! `crates/swissarmyhammer-skills/tests/findings_are_requirements_guidance.rs`
//! holds the companion check that the stance has exactly one source of truth
//! under `builtin/`.

use swissarmyhammer_agents::AgentResolver;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_skills::SkillResolver;
use swissarmyhammer_templating::TemplateLibrary;

/// Builtin agents that write or judge code. Each must render the stance.
const COVERED_AGENTS: &[&str] = &["implementer", "reviewer", "tester", "committer"];

/// Builtin skills that write or judge code. Each must render the stance.
const COVERED_SKILLS: &[&str] = &["implement", "finish", "review"];

/// The Liquid tag each covered file must carry in its raw body.
const PARTIAL_TAG: &str = "{% include \"_partials/findings-are-requirements\" %}";

/// A sentence that exists ONLY in `builtin/_partials/findings-are-requirements.md`.
/// Finding it in a rendered body proves the include resolved and expanded.
const CANONICAL_STANCE: &str = "Do not decide you know better than the rule.";

/// Labels the stance forbids. The partial must name each one, so nothing can
/// invent a severity tier the rules do not have.
const BANNED_LABELS: &[&str] = &["nit", "minor", "cosmetic", "polish", "pedantry", "churn"];

/// The template context the profile installer renders agents and skills with.
fn profile_template_context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.set(
        "version".to_string(),
        serde_json::json!(env!("CARGO_PKG_VERSION")),
    );
    ctx
}

/// Assert one rendered body carries the whole stance.
///
/// `subject` names the file under test (`agent 'reviewer'`, `skill 'finish'`) so a
/// failure says which builtin drifted.
fn assert_renders_stance(
    subject: &str,
    raw: &str,
    library: &TemplateLibrary,
    ctx: &TemplateContext,
) {
    assert!(
        raw.contains(PARTIAL_TAG),
        "builtin {subject} must include the findings-are-requirements partial"
    );

    let body = library
        .render_text(raw, ctx)
        .unwrap_or_else(|err| panic!("builtin {subject} must render: {err}"));

    assert!(
        !body.contains(PARTIAL_TAG),
        "builtin {subject} must expand the findings-are-requirements include"
    );
    assert!(
        body.contains(CANONICAL_STANCE),
        "builtin {subject} must render the findings-are-requirements stance"
    );
    for label in BANNED_LABELS {
        assert!(
            body.contains(&format!("\"{label}\"")),
            "builtin {subject} must forbid labelling a finding '{label}'"
        );
    }
    assert!(
        body.contains("mark the task stuck"),
        "builtin {subject} must route a true rule conflict to a stuck task"
    );
}

#[test]
fn code_writing_agents_render_findings_are_requirements() {
    let agents = AgentResolver::new().resolve_builtins();
    let library = TemplateLibrary::default();
    let ctx = profile_template_context();

    for name in COVERED_AGENTS {
        let agent = agents
            .get(*name)
            .unwrap_or_else(|| panic!("builtin agent '{name}' should exist"));
        assert_renders_stance(
            &format!("agent '{name}'"),
            &agent.instructions,
            &library,
            &ctx,
        );
    }
}

#[test]
fn code_writing_skills_render_findings_are_requirements() {
    let skills = SkillResolver::new().resolve_builtins();
    let library = TemplateLibrary::default();
    let ctx = profile_template_context();

    for name in COVERED_SKILLS {
        let skill = skills
            .get(*name)
            .unwrap_or_else(|| panic!("builtin skill '{name}' should exist"));
        assert_renders_stance(
            &format!("skill '{name}'"),
            &skill.instructions,
            &library,
            &ctx,
        );
    }
}
