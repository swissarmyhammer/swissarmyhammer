//! `expect doctor` — the static health check, both as the scoped top-level trait
//! verb (`expect doctor [scope]`) and as a `sah doctor` provider.
//!
//! Per `ideas/expect.md` §"The tool is doctorable", the expectation diagnostics
//! surface two ways from one source:
//!
//! - [`ExpectTool`](super::ExpectTool)'s [`Doctorable`](swissarmyhammer_common::health::Doctorable)
//!   impl calls [`health_checks`] so a plain `sah doctor` answers "are the
//!   expectation specs valid?" alongside every other system check.
//! - [`run_doctor`] is the scoped entry point — `expect doctor [scope]` — that
//!   returns the structured per-spec diagnostics and rolls them up to an exit
//!   code (clean ⇒ 0, warning ⇒ 1, error ⇒ 2).
//!
//! Both read the same pure static [`diagnose`] over each spec's raw text. No
//! system is driven and no model is consulted: the only dynamic input is the
//! injected [`DoctorFacts`], which this production layer fills with the live model
//! registry ([`ModelManager::list_agents`]). A pinned `model:` that has gone
//! missing is a **warning, not an error** — grading falls back to the default and
//! the golden compare catches any divergence as drift.

use std::path::{Path, PathBuf};

use serde::Serialize;
use swissarmyhammer_common::health::{HealthCheck, HealthStatus};
use swissarmyhammer_config::model::ModelManager;
use swissarmyhammer_expect::{
    diagnose, render, DiagnosticStatus, DoctorFacts, ExpectError, ExpectationLoader,
    FieldDiagnostic, RawSpec,
};

/// The `sah doctor` category the expectation diagnostics report under.
pub const EXPECT_CATEGORY: &str = "expect";

/// Exit code for a clean spec — no findings worse than `Ok`.
const EXIT_OK: i32 = 0;
/// Exit code when the worst finding is a `Warning`.
const EXIT_WARNING: i32 = 1;
/// Exit code when any finding is an `Error`.
const EXIT_ERROR: i32 = 2;

/// The `sah doctor` message when a repo carries no `*.expect.md` specs — so the
/// `expect` category still surfaces one line, keeping "no specs" distinct from
/// "specs healthy" (mirrors the review tool's all-valid OK summary).
const NO_SPECS_MESSAGE: &str = "no expectation specs found";

/// One spec's doctor result: its repo-relative identity and the per-field
/// diagnostics [`diagnose`] produced for it.
#[derive(Debug, Clone, Serialize)]
pub struct SpecDoctor {
    /// The spec's repo-relative identity (`.expect.md` stripped).
    pub path: String,
    /// Every per-field finding for this spec, in `diagnose` order.
    pub diagnostics: Vec<FieldDiagnostic>,
}

/// The result of `expect doctor [scope]`: per-spec diagnostics plus the rolled-up
/// exit code (0 clean, 1 warning, 2 error) across every spec.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    /// One entry per spec in the resolved scope.
    pub specs: Vec<SpecDoctor>,
    /// The worst status across all specs, as an exit code.
    pub exit_code: i32,
}

/// The exit code a single diagnostic status maps to.
///
/// The single source of truth for the status → exit mapping; the report exit code
/// is the worst (numerically largest) over all findings.
fn exit_code(status: DiagnosticStatus) -> i32 {
    match status {
        DiagnosticStatus::Ok => EXIT_OK,
        DiagnosticStatus::Warning => EXIT_WARNING,
        DiagnosticStatus::Error => EXIT_ERROR,
    }
}

/// The `sah doctor` [`HealthStatus`] a diagnostic status maps to.
fn health_status(status: DiagnosticStatus) -> HealthStatus {
    match status {
        DiagnosticStatus::Ok => HealthStatus::Ok,
        DiagnosticStatus::Warning => HealthStatus::Warning,
        DiagnosticStatus::Error => HealthStatus::Error,
    }
}

/// The names of every model in the live sah registry, for [`DoctorFacts`].
///
/// A registry that fails to load yields an empty list rather than aborting the
/// whole doctor pass — every pinned `model:` then degrades to a warning, which is
/// the same safe fallback a genuinely missing model already gets.
fn available_models() -> Vec<String> {
    ModelManager::list_agents()
        .map(|agents| agents.into_iter().map(|agent| agent.name).collect())
        .unwrap_or_default()
}

/// The production [`DoctorFacts`]: the live model registry, and no project
/// provisioning facts (so a `setup:` is unverifiable — a warning, not an error).
pub fn production_facts() -> DoctorFacts {
    DoctorFacts {
        available_models: available_models(),
        known_setup_commands: None,
    }
}

/// The repo root the context-free `sah doctor` rollup discovers specs under.
///
/// Like every CWD-relative doctor check, it prefers the enclosing git repository
/// root and falls back to the process working directory, never panicking.
fn doctor_repo_root() -> PathBuf {
    swissarmyhammer_common::utils::find_git_repository_root()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

/// The `fix` hint for a diagnostic: its concrete suggestion and/or the closed
/// allowed set, in the same shape [`render`] prints under a finding.
fn fix_hint(diagnostic: &FieldDiagnostic) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(suggestion) = &diagnostic.suggestion {
        parts.push(format!("suggestion: {suggestion}"));
    }
    if let Some(allowed) = &diagnostic.allowed {
        parts.push(format!("allowed: {}", allowed.join(" | ")));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// Map one spec's diagnostic to a [`HealthCheck`] under the `expect` category.
///
/// The name carries the spec identity and the field so the rolled-up `sah doctor`
/// line points exactly at what to fix; the message and fix come straight from the
/// engine's finding.
fn health_check_for(identity: &str, diagnostic: &FieldDiagnostic) -> HealthCheck {
    HealthCheck {
        name: format!("{identity}: {}", diagnostic.field),
        status: health_status(diagnostic.status),
        message: diagnostic.message.clone(),
        fix: fix_hint(diagnostic),
        category: EXPECT_CATEGORY.to_string(),
    }
}

/// Map every diagnostic of every spec to a [`HealthCheck`] (the `sah doctor`
/// rollup), injecting `facts` so the dynamic checks stay deterministic.
pub fn health_checks_for(specs: &[RawSpec], facts: &DoctorFacts) -> Vec<HealthCheck> {
    specs
        .iter()
        .flat_map(|spec| {
            diagnose(&spec.content, facts)
                .into_iter()
                .map(move |diagnostic| health_check_for(&spec.path, &diagnostic))
        })
        .collect()
}

/// The production `sah doctor` entry: discover every spec under the repo root
/// resolved from the session CWD, and diagnose each against the live registry.
pub fn health_checks() -> Vec<HealthCheck> {
    health_checks_in(&doctor_repo_root())
}

/// Discover every spec under `repo_root`, diagnose each against the live registry,
/// and return one [`HealthCheck`] per finding.
///
/// Root-explicit so it is testable without touching the process CWD. An empty
/// repo still yields one OK line (so the `expect` category never silently
/// vanishes from `sah doctor`); a loader failure is itself one error check.
fn health_checks_in(repo_root: &Path) -> Vec<HealthCheck> {
    let loader = ExpectationLoader::new(repo_root);
    match loader.discover_raw(None) {
        Ok(specs) if specs.is_empty() => vec![HealthCheck::ok(
            "Expectations",
            NO_SPECS_MESSAGE,
            EXPECT_CATEGORY,
        )],
        Ok(specs) => health_checks_for(&specs, &production_facts()),
        Err(err) => vec![HealthCheck::error(
            "Expectations",
            format!("failed to discover expectation specs: {err}"),
            Some("Ensure the repository's `*.expect.md` files are readable".to_string()),
            EXPECT_CATEGORY,
        )],
    }
}

/// Diagnose each raw spec against `facts`, building the structured report and its
/// rolled-up exit code.
pub fn doctor_report(specs: &[RawSpec], facts: &DoctorFacts) -> DoctorReport {
    let specs: Vec<SpecDoctor> = specs
        .iter()
        .map(|spec| SpecDoctor {
            path: spec.path.clone(),
            diagnostics: diagnose(&spec.content, facts),
        })
        .collect();

    let exit_code = specs
        .iter()
        .flat_map(|spec| spec.diagnostics.iter())
        .map(|diagnostic| exit_code(diagnostic.status))
        .max()
        .unwrap_or(EXIT_OK);

    DoctorReport { specs, exit_code }
}

/// The scoped `expect doctor [scope]` entry: discover the raw specs under
/// `repo_root` for `scope`, diagnose them against `facts`, and return the report.
pub fn run_doctor(
    repo_root: &Path,
    scope: Option<&str>,
    facts: &DoctorFacts,
) -> Result<DoctorReport, ExpectError> {
    let loader = ExpectationLoader::new(repo_root);
    let specs = loader.discover_raw(scope)?;
    Ok(doctor_report(&specs, facts))
}

/// Render a [`DoctorReport`] as the human `✓`/`✗` output, one [`render`] block
/// per spec.
pub fn render_report(report: &DoctorReport) -> String {
    report
        .specs
        .iter()
        .map(|spec| render(&spec.path, &spec.diagnostics))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The injected registry the dynamic-field tests validate against — a fixed
    /// list so no live model is consulted.
    fn facts() -> DoctorFacts {
        DoctorFacts {
            available_models: vec![
                "claude-sonnet-4-6".to_string(),
                "qwen-coder-flash".to_string(),
            ],
            known_setup_commands: None,
        }
    }

    /// A raw spec from its identity and content.
    fn raw(path: &str, content: &str) -> RawSpec {
        RawSpec {
            path: path.to_string(),
            content: content.to_string(),
        }
    }

    /// A fully valid spec: required fields, stated intent, one deterministic
    /// criterion, no pinned model — every finding is `Ok`.
    const CLEAN_SPEC: &str = "---\ndescription: a clean spec\nsurface: cli\n---\n\nThe app reduces the total when a coupon is applied.\n\n## Then\n- [ ] After applying, the total is $40\n";

    /// A valid spec whose only fault is a pinned model outside the registry — a
    /// warning, not an error.
    const WARNING_SPEC: &str = "---\ndescription: a warning spec\nsurface: cli\nmodel: not-a-real-model\n---\n\nThe app reduces the total when a coupon is applied.\n\n## Then\n- [ ] After applying, the total is $40\n";

    /// A malformed spec: an unknown frontmatter key the strict parser rejects.
    const ERROR_SPEC: &str = "---\ndescription: a malformed spec\nsurfce: cli\n---\n\nThe app reduces the total when a coupon is applied.\n\n## Then\n- [ ] After applying, the total is $40\n";

    #[test]
    fn clean_spec_exits_zero() {
        let report = doctor_report(&[raw("src/clean", CLEAN_SPEC)], &facts());
        assert_eq!(report.exit_code, EXIT_OK);
    }

    #[test]
    fn warning_only_spec_exits_one() {
        let report = doctor_report(&[raw("src/warn", WARNING_SPEC)], &facts());
        assert_eq!(report.exit_code, EXIT_WARNING);
    }

    #[test]
    fn error_spec_exits_two() {
        let report = doctor_report(&[raw("src/bad", ERROR_SPEC)], &facts());
        assert_eq!(report.exit_code, EXIT_ERROR);
    }

    #[test]
    fn exit_code_is_the_worst_status_across_specs() {
        // A clean spec next to a malformed one still exits with the error code.
        let report = doctor_report(
            &[raw("src/clean", CLEAN_SPEC), raw("src/bad", ERROR_SPEC)],
            &facts(),
        );
        assert_eq!(report.exit_code, EXIT_ERROR);
        assert_eq!(report.specs.len(), 2);
    }

    #[test]
    fn health_checks_for_a_clean_spec_are_all_ok_under_expect_category() {
        let checks = health_checks_for(&[raw("src/clean", CLEAN_SPEC)], &facts());
        assert!(!checks.is_empty(), "a clean spec still yields findings");
        for check in &checks {
            assert_eq!(check.category, EXPECT_CATEGORY);
            assert_eq!(
                check.status,
                HealthStatus::Ok,
                "clean spec finding should be Ok: {} / {}",
                check.name,
                check.message
            );
        }
    }

    #[test]
    fn health_checks_for_a_malformed_spec_include_an_expect_error() {
        let checks = health_checks_for(&[raw("src/bad", ERROR_SPEC)], &facts());
        let error = checks
            .iter()
            .find(|c| c.status == HealthStatus::Error)
            .expect("a malformed spec must surface an error health check");
        assert_eq!(error.category, EXPECT_CATEGORY);
        assert!(
            error.name.contains("src/bad"),
            "the check should name the offending spec, got: {}",
            error.name
        );
    }

    #[test]
    fn missing_pinned_model_is_a_warning_not_an_error() {
        let checks = health_checks_for(&[raw("src/warn", WARNING_SPEC)], &facts());
        assert!(
            checks.iter().any(|c| c.status == HealthStatus::Warning),
            "a missing pinned model should warn"
        );
        assert!(
            !checks.iter().any(|c| c.status == HealthStatus::Error),
            "a missing pinned model must not error"
        );
    }

    #[test]
    fn health_checks_in_an_empty_repo_yield_one_ok() {
        // No `*.expect.md` anywhere: the expect category must still surface a
        // single OK line (so "no specs" and "specs healthy" stay distinguishable
        // in `sah doctor`), mirroring the review tool's all-valid OK summary.
        let repo = tempfile::TempDir::new().unwrap();
        let checks = health_checks_in(repo.path());
        assert_eq!(checks.len(), 1, "empty repo yields exactly one check");
        assert_eq!(checks[0].status, HealthStatus::Ok);
        assert_eq!(checks[0].category, EXPECT_CATEGORY);
        assert_eq!(checks[0].message, NO_SPECS_MESSAGE);
    }

    #[test]
    fn render_report_shows_each_spec_path_and_a_fix_arrow() {
        // A malformed spec renders its identity, the offending field, and the
        // engine's `→` fix line — the human half of the structured report.
        let report = doctor_report(&[raw("src/bad", ERROR_SPEC)], &facts());
        let rendered = render_report(&report);
        assert!(rendered.contains("src/bad"), "render: {rendered}");
        assert!(rendered.contains("surfce"), "render: {rendered}");
        assert!(rendered.contains('→'), "render: {rendered}");
    }

    #[test]
    fn run_doctor_over_a_fixture_repo_resolves_exit_codes() {
        use std::fs;
        let repo = tempfile::TempDir::new().unwrap();
        fs::write(repo.path().join("clean.expect.md"), CLEAN_SPEC).unwrap();
        let clean = run_doctor(repo.path(), None, &facts()).unwrap();
        assert_eq!(clean.exit_code, EXIT_OK);

        fs::write(repo.path().join("bad.expect.md"), ERROR_SPEC).unwrap();
        let dirty = run_doctor(repo.path(), None, &facts()).unwrap();
        assert_eq!(dirty.exit_code, EXIT_ERROR);

        // A scope narrows to one spec.
        let scoped = run_doctor(repo.path(), Some("clean"), &facts()).unwrap();
        assert_eq!(scoped.specs.len(), 1);
        assert_eq!(scoped.exit_code, EXIT_OK);
    }
}
