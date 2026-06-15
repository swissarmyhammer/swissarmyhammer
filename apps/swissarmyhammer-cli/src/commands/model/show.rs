//! Model show command - display current model configuration

use crate::context::CliContext;
use colored::Colorize;
use comfy_table::Cell;
use swissarmyhammer_config::model::{ModelManager, ModelPaths};

/// A single scope row in the `sah model show` table: (scope, model name, source).
type ModelRow = (String, String, String);

/// Build the rows describing the configured models for `sah model show`.
///
/// Produces one row per scope:
/// - `default`: the global default model (top-level `model:`), or
///   `claude-code (default)` when unset.
/// - `review`: the review-tool override (`review.model`), or — when no
///   review-specific model is configured — the effective default shown as
///   `<model> (default)`. That effective default follows the overall `model:`
///   when one is set, otherwise the baked-in `claude-code-haiku`.
///
/// Each row is `(scope, model_name, source)`. The source column reflects where
/// the named model was discovered (builtin/project/user), `default` for the
/// unconfigured fallbacks, or `error` when the config could not be read.
///
/// The review row resolves through `ModelManager::resolve_review_agent_name`,
/// the same rule used by `resolve_review_agent_config` and the serve-time review
/// factory wiring, so the displayed model never disagrees with the one the
/// review scope executes.
fn build_model_rows(paths: &ModelPaths) -> Vec<ModelRow> {
    let default_row = match ModelManager::get_agent(paths) {
        Ok(Some(name)) => {
            let source = resolve_source(&name);
            ("default".to_string(), name, source)
        }
        Ok(None) => (
            "default".to_string(),
            "claude-code (default)".to_string(),
            "default".to_string(),
        ),
        Err(_) => (
            "default".to_string(),
            "(error)".to_string(),
            "error".to_string(),
        ),
    };

    let review_row = match ModelManager::get_review_agent(paths) {
        Ok(Some(name)) => {
            let source = resolve_source(&name);
            ("review".to_string(), name, source)
        }
        Ok(None) => {
            // No explicit `review.model`: display the effective default, which
            // follows the overall `model:` when set, else the baked-in
            // claude-code-haiku — so the table agrees with what the review
            // tooling actually runs.
            match ModelManager::resolve_review_agent_name(paths) {
                Ok(name) => (
                    "review".to_string(),
                    format!("{name} (default)"),
                    "default".to_string(),
                ),
                Err(_) => (
                    "review".to_string(),
                    "(error)".to_string(),
                    "error".to_string(),
                ),
            }
        }
        Err(_) => (
            "review".to_string(),
            "(error)".to_string(),
            "error".to_string(),
        ),
    };

    vec![default_row, review_row]
}

/// Resolve the display source for a configured model name.
fn resolve_source(name: &str) -> String {
    ModelManager::find_agent_by_name(name)
        .map(|info| format!("{:?}", info.source))
        .unwrap_or_else(|_| "unknown".to_string())
}

pub async fn execute_show_command(
    _context: &CliContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("{}", "Current Model:".bold());

    let mut table = swissarmyhammer_doctor::new_table();

    table.set_header(vec!["Scope", "Model", "Source"]);

    for (scope, name, source) in build_model_rows(&ModelPaths::sah()) {
        table.add_row(vec![Cell::new(scope), Cell::new(name), Cell::new(source)]);
    }

    println!("{table}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
    use swissarmyhammer_config::model::REVIEW_DEFAULT_AGENT;

    #[test]
    #[serial_test::serial(cwd)]
    fn test_build_model_rows_unset_review_shows_default_indicator() {
        // Isolated, empty project: no `.sah/sah.yaml` configured yet.
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let rows = build_model_rows(&ModelPaths::sah());

        assert_eq!(rows.len(), 2, "should render a default and a review row");
        assert_eq!(rows[0].0, "default");
        assert_eq!(rows[1].0, "review");
        // Unset review must show the *effective* baked-in default model
        // (claude-code-haiku), matching what the review tooling actually runs —
        // not a vague "(uses default)" that disagrees with execution.
        assert_eq!(
            rows[1].1,
            format!("{REVIEW_DEFAULT_AGENT} (default)"),
            "unset review should display the baked-in claude-code-haiku default"
        );
        assert_eq!(rows[1].2, "default");
    }

    /// When an overall `model:` is set but `review.model` is not, the review row
    /// must show that overall model as its (inherited) default — not the
    /// baked-in claude-code-haiku.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_build_model_rows_unset_review_inherits_overall_default() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let paths = ModelPaths::sah();
        // Pick a builtin that is NOT the review factory default so the
        // inheritance is observable.
        let overall = ModelManager::load_builtin_models()
            .expect("builtin models")
            .into_iter()
            .map(|m| m.name)
            .find(|n| n != REVIEW_DEFAULT_AGENT)
            .expect("a non-haiku builtin");

        ModelManager::use_agent(&overall, &paths).expect("set overall model");

        let rows = build_model_rows(&paths);

        assert_eq!(rows[0].1, overall, "default row shows the overall model");
        assert_eq!(rows[1].0, "review");
        assert_eq!(
            rows[1].1,
            format!("{overall} (default)"),
            "review must inherit the explicit overall model, not fall to claude-code-haiku"
        );
    }

    /// Regression guard for the bug where `sah model` showed `review: (uses
    /// default)` while the review tooling actually resolved `claude-code-haiku`.
    /// The displayed review model MUST contain whatever
    /// `resolve_review_agent_name` resolves, so the display and execution paths
    /// cannot drift apart.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_review_row_agrees_with_resolved_review_name() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let paths = ModelPaths::sah();
        let resolved =
            ModelManager::resolve_review_agent_name(&paths).expect("resolve review name");

        let rows = build_model_rows(&paths);
        let review = &rows[1];
        assert_eq!(review.0, "review");
        assert!(
            review.1.contains(&resolved),
            "displayed review model '{}' must contain the resolved effective name '{}'",
            review.1,
            resolved
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_build_model_rows_renders_both_default_and_review() {
        let env = IsolatedTestEnvironment::new().expect("isolated env");
        let _cwd = CurrentDirGuard::new(env.temp_dir()).expect("cwd guard");

        let paths = ModelPaths::sah();

        // Configure a default model and a distinct review override.
        let builtin = ModelManager::load_builtin_models().expect("builtin models");
        let default_model = builtin[0].name.clone();
        let review_model = builtin
            .iter()
            .map(|m| m.name.clone())
            .find(|n| *n != default_model)
            .unwrap_or_else(|| default_model.clone());

        ModelManager::use_agent(&default_model, &paths).expect("set default model");
        ModelManager::use_agent_for(
            &review_model,
            swissarmyhammer_config::model::ModelTarget::Review,
            &paths,
        )
        .expect("set review model");

        let rows = build_model_rows(&paths);

        assert_eq!(rows[0].0, "default");
        assert_eq!(
            rows[0].1, default_model,
            "default row should show the default model"
        );
        assert_eq!(rows[1].0, "review");
        assert_eq!(
            rows[1].1, review_model,
            "review row should show the review override"
        );
    }
}
