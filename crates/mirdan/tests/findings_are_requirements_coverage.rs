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
///
/// The 'implementer' agent is not in this list. That agent gets the stance
/// through the `implement` skill. The `implement` skill stays in
/// `COVERED_SKILLS`.
const COVERED_AGENTS: &[&str] = &[
    "reviewer",
    "tester",
    "committer",
    "double-check",
    "general-purpose",
];

/// Builtin skills that write or judge code. Each must render the stance.
const COVERED_SKILLS: &[&str] = &["implement", "finish", "review", "kanban", "tdd", "test"];

/// The Liquid tag each covered file must carry in its raw body.
const PARTIAL_TAG: &str = "{% include \"_partials/findings-are-requirements\" %}";

/// Sentences that exist ONLY in `builtin/_partials/findings-are-requirements.md`.
/// Finding one in a rendered body proves the include resolved and expanded.
///
/// Two sentences, not one: the first is the anti-editorializing rule and the
/// second is the no-severity-tier rule. Both were previously duplicated in prose,
/// so both are pinned. A rendered body that carries only one of them is a partial
/// stance and fails.
///
/// This list must stay identical to `CANONICAL_STANCE` in
/// `crates/swissarmyhammer-skills/tests/findings_are_requirements_guidance.rs`,
/// which pins the same sentences to exactly one `builtin/` source file. The two
/// guards cover different things — that one owns single-source-of-truth, this one
/// owns the production render of every covered agent and skill — so both
/// must pin the whole stance or an agent can drop a sentence and still pass.
const CANONICAL_STANCE: &[&str] = &[
    "Do not decide you know better than the rule.",
    "There is no severity tier. Every finding is mandatory.",
];

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
    for stance in CANONICAL_STANCE {
        assert!(
            body.contains(stance),
            "builtin {subject} must render the stance sentence: {stance}"
        );
    }
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
