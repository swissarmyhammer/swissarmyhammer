//! Validator introspection ops for the `review` tool: `list/get/check validators`.
//!
//! These three ops are pure loader reads — no agent, no index, fast. They load
//! the full builtin → user → project RuleSet stack via
//! [`swissarmyhammer_validators::load_rules`] and report on it:
//!
//! - `list validators` — one summary row per loaded RuleSet (name, description,
//!   source layer, match globs, severity, probes, rule count, path), optionally
//!   filtered by `source` and/or a path/glob `match`.
//! - `get validator` — one RuleSet's full rule bodies + probes.
//! - `check validators` — lint every loaded RuleSet: frontmatter is valid (it
//!   parsed), declared globs compile, no stray `triggerMatcher`, and every
//!   declared probe exists in the engine's probe catalog.

use serde::Serialize;
use swissarmyhammer_validators::review::probe_exists;
use swissarmyhammer_validators::validators::{
    compile_glob_patterns, matches_any_pattern, ValidatorMatch,
};
use swissarmyhammer_validators::{load_rules, RuleSet};

/// A `list validators` summary row.
#[derive(Debug, Serialize)]
pub struct ValidatorSummary {
    /// The RuleSet name.
    pub name: String,
    /// The RuleSet description (mandate).
    pub description: String,
    /// Which precedence layer it came from (`builtin` / `user` / `project`).
    pub source_layer: String,
    /// The file globs the RuleSet matches against.
    pub match_globs: Vec<String>,
    /// The RuleSet's default severity (`info` / `warn` / `error`).
    pub severity: String,
    /// The probe names the RuleSet declares.
    pub probes: Vec<String>,
    /// How many rules the RuleSet carries.
    pub rule_count: usize,
    /// The RuleSet directory path.
    pub path: String,
}

/// One rule in a `get validator` response.
#[derive(Debug, Serialize)]
pub struct RuleDetail {
    /// The rule name.
    pub name: String,
    /// The rule's effective severity word (`info` / `warn` / `error`).
    pub severity: String,
    /// The rule's markdown body verbatim.
    pub body: String,
}

/// A `get validator` response — one RuleSet's full detail.
#[derive(Debug, Serialize)]
pub struct ValidatorDetail {
    /// The RuleSet name.
    pub name: String,
    /// The full parsed frontmatter (manifest), as the loader holds it.
    pub frontmatter: ValidatorFrontmatterView,
    /// Which precedence layer it came from.
    pub source_layer: String,
    /// The RuleSet directory path.
    pub path: String,
    /// The probe names the RuleSet declares.
    pub probes: Vec<String>,
    /// Each rule's name, severity, and full body.
    pub rules: Vec<RuleDetail>,
}

/// The frontmatter view rendered into a `get validator` response.
#[derive(Debug, Serialize)]
pub struct ValidatorFrontmatterView {
    /// The RuleSet name.
    pub name: String,
    /// The RuleSet description.
    pub description: String,
    /// The default severity word.
    pub severity: String,
    /// The file globs.
    pub match_globs: Vec<String>,
    /// The declared tags.
    pub tags: Vec<String>,
    /// The package version.
    pub version: String,
}

/// One `check validators` problem.
#[derive(Debug, Serialize)]
pub struct ValidatorProblem {
    /// The RuleSet path (or name) the problem is about.
    pub path: String,
    /// What is wrong.
    pub problem: String,
}

/// A `check validators` response: overall `ok` plus every problem found.
#[derive(Debug, Serialize)]
pub struct CheckValidatorsResponse {
    /// True when no problem was found across every loaded RuleSet.
    pub ok: bool,
    /// How many RuleSets were loaded and linted.
    pub count: usize,
    /// Every lint problem found (empty when `ok`).
    pub errors: Vec<ValidatorProblem>,
}

/// The match globs of a RuleSet, or an empty vec when it matches everything.
fn match_globs(ruleset: &RuleSet) -> Vec<String> {
    ruleset
        .manifest
        .match_criteria
        .as_ref()
        .map(|m: &ValidatorMatch| m.files.clone())
        .unwrap_or_default()
}

/// Build the summary row for one RuleSet.
fn summary(ruleset: &RuleSet) -> ValidatorSummary {
    ValidatorSummary {
        name: ruleset.name().to_string(),
        description: ruleset.description().to_string(),
        source_layer: ruleset.source.to_string(),
        match_globs: match_globs(ruleset),
        severity: ruleset.manifest.severity.to_string(),
        probes: ruleset.manifest.probes.clone(),
        rule_count: ruleset.rules.len(),
        path: ruleset.base_path.display().to_string(),
    }
}

/// Whether a RuleSet passes the `source` and `match` filters.
///
/// `source` is one of `builtin` / `user` / `project` / `all` (or absent = all).
/// `match` is a path/glob string the RuleSet's own match globs must overlap with
/// (matched leniently: the filter is treated as a path and tested against each of
/// the RuleSet's globs).
fn passes_filters(ruleset: &RuleSet, source: Option<&str>, match_filter: Option<&str>) -> bool {
    if let Some(source) = source {
        if !source.eq_ignore_ascii_case("all")
            && !ruleset.source.to_string().eq_ignore_ascii_case(source)
        {
            return false;
        }
    }

    if let Some(needle) = match_filter {
        let globs = match_globs(ruleset);
        let compiled = compile_glob_patterns(&globs);
        let hit =
            matches_any_pattern(needle, &compiled) || globs.iter().any(|g| g.contains(needle));
        if !hit {
            return false;
        }
    }

    true
}

/// `list validators`: load the full RuleSet stack, filter, and return summaries
/// sorted by name.
///
/// # Errors
///
/// Returns a message when [`load_rules`] fails (user/project directory read
/// error).
pub fn list_validators(
    source: Option<&str>,
    match_filter: Option<&str>,
) -> Result<Vec<ValidatorSummary>, String> {
    let loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    let mut summaries: Vec<ValidatorSummary> = loader
        .list_rulesets()
        .into_iter()
        .filter(|rs| passes_filters(rs, source, match_filter))
        .map(summary)
        .collect();
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(summaries)
}

/// `get validator`: load the stack and return one RuleSet's full detail.
///
/// # Errors
///
/// Returns a message when [`load_rules`] fails or when no RuleSet is named
/// `name`.
pub fn get_validator(name: &str) -> Result<ValidatorDetail, String> {
    let loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    let ruleset = loader
        .get_ruleset(name)
        .ok_or_else(|| format!("no validator named '{name}'"))?;

    let rules = ruleset
        .rules
        .iter()
        .map(|rule| RuleDetail {
            name: rule.name.clone(),
            severity: rule.effective_severity(ruleset).to_string(),
            body: rule.body.clone(),
        })
        .collect();

    Ok(ValidatorDetail {
        name: ruleset.name().to_string(),
        frontmatter: ValidatorFrontmatterView {
            name: ruleset.name().to_string(),
            description: ruleset.description().to_string(),
            severity: ruleset.manifest.severity.to_string(),
            match_globs: match_globs(ruleset),
            tags: ruleset.manifest.tags.clone(),
            version: ruleset.manifest.metadata.version.clone(),
        },
        source_layer: ruleset.source.to_string(),
        path: ruleset.base_path.display().to_string(),
        probes: ruleset.manifest.probes.clone(),
        rules,
    })
}

/// `check validators`: lint every loaded RuleSet and report load failures.
///
/// Reports a problem when a RuleSet declares a glob that does not compile, sets a
/// stray `triggerMatcher` (review validators match by file, not by event), or
/// declares a probe that is not in the engine's probe catalog.
///
/// A RuleSet whose frontmatter does not parse never reaches the loaded set, but
/// it is **not** silently dropped: the loader retains each parse failure
/// ([`load_failures`](swissarmyhammer_validators::ValidatorLoader::load_failures))
/// and this lint surfaces every one as an error naming the offending path and its
/// parse problem. A broken validator never aborts the run — the rest still load.
///
/// # Errors
///
/// Returns a message when [`load_rules`] fails.
pub fn check_validators() -> Result<CheckValidatorsResponse, String> {
    let loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    let mut errors: Vec<ValidatorProblem> = Vec::new();

    let rulesets = loader.list_rulesets();
    let count = rulesets.len();
    for ruleset in rulesets {
        let path = ruleset.base_path.display().to_string();
        lint_ruleset(ruleset, &path, &mut errors);
    }

    // Dropped (unparseable) validators: each is reported, not swallowed.
    for failure in loader.load_failures() {
        errors.push(ValidatorProblem {
            path: failure.path.display().to_string(),
            problem: format!(
                "failed to load ({} validator): {}",
                failure.source, failure.error
            ),
        });
    }

    errors.sort_by(|a, b| {
        (a.path.as_str(), a.problem.as_str()).cmp(&(b.path.as_str(), b.problem.as_str()))
    });
    Ok(CheckValidatorsResponse {
        ok: errors.is_empty(),
        count,
        errors,
    })
}

/// Lint one RuleSet, appending any problems found.
fn lint_ruleset(ruleset: &RuleSet, path: &str, errors: &mut Vec<ValidatorProblem>) {
    // Globs must compile.
    for glob in match_globs(ruleset) {
        if glob::Pattern::new(&glob).is_err() {
            errors.push(ValidatorProblem {
                path: path.to_string(),
                problem: format!("invalid match glob '{glob}'"),
            });
        }
    }

    // A review validator matches by changed file, never by a hook-event string;
    // a stray triggerMatcher is a misconfiguration.
    if ruleset.manifest.trigger_matcher.is_some() {
        errors.push(ValidatorProblem {
            path: path.to_string(),
            problem: "stray `triggerMatcher`: review validators match by file, not by event"
                .to_string(),
        });
    }

    // Every declared probe must exist in the engine's probe catalog.
    for probe in &ruleset.manifest.probes {
        if !probe_exists(probe) {
            errors.push(ValidatorProblem {
                path: path.to_string(),
                problem: format!("declared probe '{probe}' is not in the probe catalog"),
            });
        }
    }
}
