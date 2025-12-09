//! Integration tests for template caching functionality.
//!
//! These tests verify the template caching API surface and basic functionality
//! without requiring a loaded model. Full end-to-end testing of cache hit/miss
//! behavior requires a real model file and is covered by other integration tests.

use llama_agent::{
    chat_template::ChatTemplateEngine,
    generation::GenerationHelper,
    model::ModelManager,
    types::{
        GenerationRequest, Message, MessageRole, ModelConfig, ModelSource, Session, SessionId,
        ToolDefinition,
    },
};
use std::sync::Arc;
use std::time::SystemTime;
use tempfile::TempDir;

#[tokio::test]
async fn test_session_template_caching_workflow() {
    // This test verifies the API exists and basic structure
    // Full functionality requires a real model to work

    let temp_dir = TempDir::new().unwrap();
    let config = ModelConfig {
        source: ModelSource::Local {
            folder: temp_dir.path().to_path_buf(),
            filename: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 1,
        n_threads_batch: 1,
        use_hf_params: false,
        retry_config: llama_agent::types::RetryConfig::default(),
        debug: false,
    };

    match ModelManager::new(config) {
        Ok(manager) => {
            // Verify template cache is initialized
            let stats = manager.get_template_cache_stats();
            assert_eq!(stats.entries, 0);

            // Create a test session
            let _session = Session {
                id: SessionId::new(),
                messages: vec![Message {
                    role: MessageRole::System,
                    content: "You are a helpful assistant.".to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                }],
                mcp_servers: Vec::new(),
                available_tools: vec![ToolDefinition {
                    name: "test_tool".to_string(),
                    description: "A test tool".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }),
                    server_name: "test_server".to_string(),
                }],
                available_prompts: Vec::new(),
                created_at: SystemTime::now(),
                updated_at: SystemTime::now(),
                compaction_history: Vec::new(),
                transcript_path: None,
                context_state: None,
                template_token_count: None,
            };

            // Note: Full test would require loaded model and context
            // This test verifies the API exists and compiles
        }
        Err(_) => {
            // Expected in test environment without real model
        }
    }
}

#[tokio::test]
async fn test_template_cache_stats_api() {
    let temp_dir = TempDir::new().unwrap();
    let config = ModelConfig {
        source: ModelSource::Local {
            folder: temp_dir.path().to_path_buf(),
            filename: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 1,
        n_threads_batch: 1,
        use_hf_params: false,
        retry_config: llama_agent::types::RetryConfig::default(),
        debug: false,
    };

    match ModelManager::new(config) {
        Ok(manager) => {
            // Test that we can get cache stats
            let stats = manager.get_template_cache_stats();
            assert_eq!(stats.entries, 0);
            assert_eq!(stats.hits, 0);
            assert_eq!(stats.misses, 0);
            assert_eq!(stats.hit_rate, 0.0);
        }
        Err(_) => {
            // Expected in test environment
        }
    }
}

#[tokio::test]
async fn test_chat_engine_extract_template_components() {
    let engine = ChatTemplateEngine::new();

    let session = Session {
        id: SessionId::new(),
        messages: vec![
            Message {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        ],
        mcp_servers: Vec::new(),
        available_tools: vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather information".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
            server_name: "weather_server".to_string(),
        }],
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: None,
    };

    let result = engine.extract_template_components(&session);
    assert!(result.is_ok());

    let (system_prompt, tools_json) = result.unwrap();
    assert_eq!(system_prompt, "You are a helpful assistant.");
    assert!(tools_json.contains("get_weather"));
    assert!(tools_json.contains("Get weather information"));
}

#[tokio::test]
async fn test_chat_engine_extract_template_components_no_tools() {
    let engine = ChatTemplateEngine::new();

    let session = Session {
        id: SessionId::new(),
        messages: vec![Message {
            role: MessageRole::System,
            content: "You are a helpful assistant.".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }],
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: None,
    };

    let result = engine.extract_template_components(&session);
    assert!(result.is_ok());

    let (system_prompt, tools_json) = result.unwrap();
    assert_eq!(system_prompt, "You are a helpful assistant.");
    assert_eq!(tools_json, "");
}

#[tokio::test]
async fn test_chat_engine_extract_template_with_multiple_system_messages() {
    let engine = ChatTemplateEngine::new();

    let session = Session {
        id: SessionId::new(),
        messages: vec![
            Message {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::System,
                content: "Be concise.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        ],
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: None,
    };

    let result = engine.extract_template_components(&session);
    assert!(result.is_ok());

    let (system_prompt, _tools_json) = result.unwrap();
    assert_eq!(system_prompt, "You are a helpful assistant.\nBe concise.");
}

#[tokio::test]
async fn test_generation_helper_template_offset_api() {
    // Test that the template offset methods exist and have correct signatures
    // This test verifies the API surface without requiring a loaded model

    let temp_dir = TempDir::new().unwrap();
    let config = ModelConfig {
        source: ModelSource::Local {
            folder: temp_dir.path().to_path_buf(),
            filename: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 1,
        n_threads_batch: 1,
        use_hf_params: false,
        retry_config: llama_agent::types::RetryConfig::default(),
        debug: false,
    };

    // We can't actually call the methods without a real model,
    // but we can verify the API exists and compiles
    match ModelManager::new(config) {
        Ok(model_manager) => {
            let model_manager = Arc::new(model_manager);

            // Verify the method exists - this will fail to compile if signature changes
            let _test_fn = |model: &llama_cpp_2::model::LlamaModel,
                            ctx: &mut llama_cpp_2::context::LlamaContext,
                            prompt: &str,
                            request: &GenerationRequest,
                            token: &tokio_util::sync::CancellationToken,
                            batch_size: usize,
                            template_token_count: Option<usize>| {
                GenerationHelper::generate_text_with_borrowed_model_and_template_offset(
                    model,
                    ctx,
                    prompt,
                    request,
                    token,
                    batch_size,
                    template_token_count,
                )
            };

            // Verify batch_size getter exists
            let batch_size = model_manager.get_batch_size();
            assert!(batch_size > 0);
        }
        Err(_) => {
            // Expected in test environment without real model
        }
    }
}

#[tokio::test]
async fn test_session_template_token_count_field() {
    // Test that Session has template_token_count field and it works correctly

    let session_without_cache = Session {
        id: SessionId::new(),
        messages: vec![Message {
            role: MessageRole::System,
            content: "You are a helpful assistant.".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }],
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: None,
    };

    assert_eq!(session_without_cache.template_token_count, None);

    let session_with_cache = Session {
        id: SessionId::new(),
        messages: vec![Message {
            role: MessageRole::System,
            content: "You are a helpful assistant.".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }],
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: Some(42),
    };

    assert_eq!(session_with_cache.template_token_count, Some(42));
}

#[tokio::test]
async fn test_generation_request_structure() {
    // Test that GenerationRequest structure is compatible with template offset usage

    let request = GenerationRequest {
        session_id: SessionId::new(),
        max_tokens: Some(100),
        temperature: Some(0.7),
        top_p: Some(0.9),
        stop_tokens: vec!["</s>".to_string()],
        stopping_config: None,
    };

    // Verify all fields are accessible
    assert_eq!(request.max_tokens, Some(100));
    assert_eq!(request.temperature, Some(0.7));
    assert_eq!(request.top_p, Some(0.9));
    assert_eq!(request.stop_tokens.len(), 1);
}

#[tokio::test]
async fn test_template_components_extraction_consistency() {
    // Test that template extraction produces consistent results for same input
    let engine = ChatTemplateEngine::new();

    let session = Session {
        id: SessionId::new(),
        messages: vec![Message {
            role: MessageRole::System,
            content: "You are a helpful assistant.".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }],
        mcp_servers: Vec::new(),
        available_tools: vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            server_name: "test_server".to_string(),
        }],
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: None,
    };

    // Extract twice and verify consistency
    let result1 = engine.extract_template_components(&session);
    let result2 = engine.extract_template_components(&session);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    let (system1, tools1) = result1.unwrap();
    let (system2, tools2) = result2.unwrap();

    assert_eq!(system1, system2);
    assert_eq!(tools1, tools2);
}

#[tokio::test]
async fn test_template_token_count_optional_behavior() {
    // Test that None vs Some(0) are distinct and meaningful

    let session_none = Session {
        id: SessionId::new(),
        messages: vec![],
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: None,
    };

    let session_zero = Session {
        id: SessionId::new(),
        messages: vec![],
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        template_token_count: Some(0),
    };

    // None means no cache
    assert!(session_none.template_token_count.is_none());

    // Some(0) means cache exists but has zero tokens (edge case)
    assert!(session_zero.template_token_count.is_some());
    assert_eq!(session_zero.template_token_count.unwrap(), 0);
}
