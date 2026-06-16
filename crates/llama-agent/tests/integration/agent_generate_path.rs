//! Real-model coverage for `AgentServer`'s own (non-ACP) generate-path helpers.
//!
//! `agent.rs` carries an API surface distinct from the ACP `prompt` loop:
//! `generate_session_title` / `title_via_model`. The fallback (no-model /
//! model-error) branch is unit-tested in `acp/server.rs`; the **success**
//! branch — the model actually produces a title that `normalize_title`
//! shapes — needs a real model and is exercised here against the small
//! Qwen3-0.6B test model.

use serial_test::serial;
use tracing::info;

use crate::integration::real_model_helpers::{real_model_config, try_init_real_model_agent};

/// `generate_session_title` success branch: a real model call produces a
/// non-empty title for a non-empty first user message.
///
/// This drives `title_via_model` end-to-end — render title prompt → create
/// context → `GenerationHelper::generate_text_with_borrowed_model` → normalize —
/// and the `Ok(Some(title))` arm of `generate_session_title`. The result is
/// normalized (whitespace collapsed, capped to the title length), so we assert a
/// bounded, non-empty title rather than exact text the tiny model can't pin.
#[tokio::test]
#[serial]
async fn test_generate_session_title_success_branch() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_real_model_agent(real_model_config()).await else {
        return;
    };

    let title = agent
        .generate_session_title("Help me write a Python script to rename files in a folder.")
        .await;

    info!("generated title: {:?}", title);

    let title = title.expect("a non-empty first user message must yield Some(title)");
    assert!(
        !title.trim().is_empty(),
        "title-via-model success branch must produce a non-empty title"
    );
    // normalize_title caps the title length; a sane title is not a runaway
    // paragraph. The cap is generous; this just guards against an unnormalized
    // full generation leaking through.
    assert!(
        title.chars().count() <= 120,
        "normalized title should be short, got {} chars: {:?}",
        title.chars().count(),
        title
    );
}

/// Empty/whitespace first message must short-circuit to `None` WITHOUT a model
/// call — the guard at the top of `generate_session_title`. This pins the
/// early-return arm that the success test deliberately steps over.
#[tokio::test]
#[serial]
async fn test_generate_session_title_empty_message_returns_none() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    let Some(agent) = try_init_real_model_agent(real_model_config()).await else {
        return;
    };

    assert!(
        agent.generate_session_title("   ").await.is_none(),
        "whitespace-only first message must yield None (no title to make)"
    );
}
