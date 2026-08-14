use super::*;
use swissarmyhammer_common::test_utils::CurrentDirGuard;

/// A temporary directory with a `.git` marker, so config discovery stops
/// there instead of walking up into the real repository.
fn isolated_project() -> tempfile::TempDir {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    std::fs::create_dir(temp_dir.path().join(".git")).expect(".git marker");
    temp_dir
}

/// Write `.sah/sah.yaml` with the given content in the current project.
fn write_sah_config(content: &str) {
    let config_path = ModelManager::ensure_config_structure(&ModelPaths::sah()).unwrap();
    std::fs::write(&config_path, content).unwrap();
}

/// The embedding stack loads its models by name through `ModelManager`.
/// Collapsing the chat side must leave that path intact: both embedding
/// YAMLs still resolve, still parse, and still select an embedding executor.
#[test]
fn embedding_models_still_resolve_through_model_manager() {
    for name in ["nomic-embed-code", "qwen-embedding"] {
        let info = ModelManager::find_agent_by_name(name)
            .unwrap_or_else(|e| panic!("builtin embedding model `{name}` must resolve: {e}"));
        let config = parse_model_config(&info.content)
            .unwrap_or_else(|e| panic!("builtin embedding model `{name}` must parse: {e}"));
        let executor = config
            .select_executor()
            .unwrap_or_else(|| panic!("`{name}` must offer an executor for this platform"));
        assert!(
            matches!(
                executor,
                ModelExecutorConfig::LlamaEmbedding(_) | ModelExecutorConfig::AneEmbedding(_)
            ),
            "`{name}` must select an embedding executor, got {executor:?}"
        );
    }
}

/// `builtin/models/` is now an embedding-only library. A leftover chat YAML
/// would resurrect the model-name lookup this card removed.
#[test]
fn builtin_models_are_all_embedding_models() {
    let models = ModelManager::load_builtin_models().expect("builtin models load");
    let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["nomic-embed-code", "qwen-embedding"],
        "builtin/models/ must hold only the embedding models"
    );
}

/// An unconfigured review scope runs `claude --model haiku`, chosen through
/// the configuration field rather than a model-name lookup.
#[test]
#[serial_test::serial(cwd)]
fn unconfigured_review_scope_resolves_to_the_haiku_switch() {
    let temp_dir = isolated_project();
    let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();

    let config = ModelManager::resolve_review_chat_config(&ModelPaths::sah()).unwrap();
    assert_eq!(
        config.model.as_deref(),
        Some(REVIEW_DEFAULT_CLAUDE_MODEL),
        "an unconfigured review scope must pick the baked-in Haiku switch"
    );
    assert_eq!(
        config.claude_args(),
        vec!["--model".to_string(), "haiku".to_string()],
        "the review scope must spawn `claude --model haiku`"
    );
}

/// `sah doctor`-style reporting and the spawned process read the same
/// resolver, so the model reported can never disagree with the model run.
#[test]
#[serial_test::serial(cwd)]
fn reported_review_model_matches_the_switch_that_is_run() {
    let temp_dir = isolated_project();
    let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
    write_sah_config("review:\n  model: opus\n");

    let reported = ModelManager::resolve_review_chat_model(&ModelPaths::sah()).unwrap();
    let run = ModelManager::resolve_review_chat_config(&ModelPaths::sah()).unwrap();
    assert_eq!(reported, "opus");
    assert_eq!(
        run.claude_args(),
        vec!["--model".to_string(), reported],
        "the reported model must be the one the spawned claude receives"
    );
}

/// Precedence is `review.model` → top-level `model:` → the baked-in Haiku
/// switch. Only the meaning of the value changed: it is now the Claude CLI
/// `--model` switch, not the name of a model YAML.
#[test]
fn review_chat_model_precedence() {
    assert_eq!(
        ModelManager::review_chat_model_from(
            ReviewModel(Some("opus".into())),
            DefaultModel(Some("sonnet".into()))
        ),
        "opus",
        "an explicit review.model wins"
    );
    assert_eq!(
        ModelManager::review_chat_model_from(
            ReviewModel(None),
            DefaultModel(Some("sonnet".into()))
        ),
        "sonnet",
        "an overall model: drives review when review.model is unset"
    );
    assert_eq!(
        ModelManager::review_chat_model_from(ReviewModel(None), DefaultModel(None)),
        REVIEW_DEFAULT_CLAUDE_MODEL,
        "a fully unconfigured review scope falls to the baked-in Haiku switch"
    );
}

/// Pins the literal switch, not just the symbol. Every other test in this
/// file compares against `REVIEW_DEFAULT_CLAUDE_MODEL` itself, which stays
/// green even if the fallback path stops reading the constant, as long as
/// both sides still resolve to the same symbol. This test fails the moment
/// anyone edits the constant's value, with no hand-editing-and-reverting
/// required to prove that.
#[test]
fn review_default_claude_model_is_the_literal_haiku_switch() {
    assert_eq!(
        REVIEW_DEFAULT_CLAUDE_MODEL, "haiku",
        "the baked-in review-scope default must stay the literal `haiku` switch"
    );
}

/// The default (non-review) chat scope stays plain `claude` with no
/// `--model`, so the Claude CLI's own default applies.
#[test]
#[serial_test::serial(cwd)]
fn unconfigured_default_scope_spawns_plain_claude() {
    let temp_dir = isolated_project();
    let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();

    let config = ModelManager::resolve_chat_config(&ModelPaths::sah()).unwrap();
    assert!(config.model.is_none(), "no switch is configured");
    assert!(
        config.claude_args().is_empty(),
        "plain claude carries no --model switch"
    );
}

/// A top-level `model:` drives the default scope too.
#[test]
#[serial_test::serial(cwd)]
fn configured_default_scope_uses_the_configured_switch() {
    let temp_dir = isolated_project();
    let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
    write_sah_config("model: sonnet\n");

    let config = ModelManager::resolve_chat_config(&ModelPaths::sah()).unwrap();
    assert_eq!(
        config.claude_args(),
        vec!["--model".to_string(), "sonnet".to_string()]
    );
}

/// A non-string `model:` (e.g. a number) is ignored rather than coerced, so
/// a mistyped config falls back to the default instead of spawning
/// `claude --model 3`.
#[test]
#[serial_test::serial(cwd)]
fn non_string_model_value_is_ignored() {
    let temp_dir = isolated_project();
    let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
    write_sah_config("model: 3\n");

    let config = ModelManager::resolve_chat_config(&ModelPaths::sah()).unwrap();
    assert!(
        config.model.is_none(),
        "a non-string model: must be ignored, got {:?}",
        config.model
    );
}

/// A blank switch is a configuration error, not a `claude --model ""` spawn.
#[test]
#[serial_test::serial(cwd)]
fn blank_switch_is_a_configuration_error() {
    let temp_dir = isolated_project();
    let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
    write_sah_config("review:\n  model: \"   \"\n");

    match ModelManager::resolve_review_chat_config(&ModelPaths::sah()) {
        Err(ModelError::ConfigError(msg)) => {
            assert!(
                msg.contains("model"),
                "the error must name the offending setting, got: {msg}"
            );
        }
        other => panic!("expected a ConfigError for a blank model switch, got {other:?}"),
    }
}
