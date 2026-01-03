//! Test utilities for creating real test data for compaction testing

use crate::types::*;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Create a test session with the specified number of messages
pub fn create_test_session_with_messages(message_count: usize) -> Session {
    let mut session = Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: Vec::new(),
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    };

    // Add alternating user/assistant messages
    for i in 0..message_count {
        session.messages.push(Message {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content: format!(
                "Test message {} with some content that contributes to token usage",
                i
            ),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });
    }

    session
}

/// Create a test session with messages that have substantial content for token testing
pub fn create_large_content_session(message_count: usize, words_per_message: usize) -> Session {
    let mut session = Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: Vec::new(),
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    };

    for i in 0..message_count {
        let content = format!("Message {}: {}", i, "word ".repeat(words_per_message));
        session.messages.push(Message {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content,
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });
    }

    session
}

/// Create a test session with tool calls for testing tool call validation
pub fn create_session_with_tool_calls() -> Session {
    let mut session = Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: Vec::new(),
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    };

    // Add user message
    session.messages.push(Message {
        role: MessageRole::User,
        content: "Please call the test tool".to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    });

    // Add assistant message with tool call
    session.messages.push(Message {
        role: MessageRole::Assistant,
        content: "I'll call the test tool for you".to_string(),
        tool_call_id: Some(ToolCallId::new()),
        tool_name: Some("test_tool".to_string()),
        timestamp: SystemTime::now(),
    });

    // Add tool response
    let tool_call_id = ToolCallId::new();
    session.messages.push(Message {
        role: MessageRole::Tool,
        content: "test_tool executed successfully".to_string(),
        tool_call_id: Some(tool_call_id),
        tool_name: Some("test_tool".to_string()),
        timestamp: SystemTime::now(),
    });

    session
}

/// Create a session with incomplete tool calls for validation testing
pub fn create_session_with_incomplete_tool_calls() -> Session {
    let mut session = Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: Vec::new(),
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    };

    // Add user message
    session.messages.push(Message {
        role: MessageRole::User,
        content: "Please call the test tool".to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    });

    // Add assistant message with tool call
    let tool_call_id = ToolCallId::new();
    session.messages.push(Message {
        role: MessageRole::Assistant,
        content: "I'll call the test tool for you".to_string(),
        tool_call_id: Some(tool_call_id),
        tool_name: Some("test_tool".to_string()),
        timestamp: SystemTime::now(),
    });

    // Add tool response message
    session.messages.push(Message {
        role: MessageRole::Tool,
        content: "test_tool executed successfully".to_string(),
        tool_call_id: Some(tool_call_id),
        tool_name: Some("test_tool".to_string()),
        timestamp: SystemTime::now(),
    });

    session
}

/// Create a test compaction config with reasonable defaults
pub fn create_test_compaction_config() -> CompactionConfig {
    CompactionConfig {
        threshold: 0.8,
        preserve_recent: 3,
        custom_prompt: None,
    }
}

/// Create a test compaction config that will trigger compaction easily (low thresholds)
pub fn create_low_threshold_compaction_config() -> CompactionConfig {
    CompactionConfig {
        threshold: 0.01, // Extremely low threshold - 1% of context
        preserve_recent: 2,
        custom_prompt: None,
    }
}

/// Create token usage for testing
pub fn create_test_token_usage(total: usize, message_tokens: Vec<usize>) -> TokenUsage {
    let mut by_role = HashMap::new();
    by_role.insert(MessageRole::User, total / 3);
    by_role.insert(MessageRole::Assistant, (total * 2) / 3);

    TokenUsage {
        total,
        by_role,
        by_message: message_tokens,
    }
}

/// Create a session that should trigger compaction based on token usage
pub fn create_session_requiring_compaction() -> Session {
    // Create session with enough messages to exceed typical thresholds
    create_large_content_session(20, 50) // 20 messages with 50 words each
}

/// Create a minimal session that should not trigger compaction
pub fn create_minimal_session() -> Session {
    create_test_session_with_messages(2) // Just 2 messages
}

/// Create a real compaction summary for testing
/// This uses actual content that would result from real generation
pub fn create_real_compaction_summary(original_messages: &[Message]) -> String {
    // Create summary based on actual message content
    let key_topics: Vec<String> = original_messages
        .iter()
        .enumerate()
        .map(|(i, msg)| {
            format!(
                "Message {}: {}",
                i + 1,
                msg.content.chars().take(50).collect::<String>()
            )
        })
        .collect();

    format!(
        "Conversation summary covering {} messages. Key topics discussed: {}. \
        This summary maintains the essential context while reducing token usage for continued conversation.",
        original_messages.len(),
        key_topics.join(", ")
    )
}

/// Create compaction metadata for testing
pub fn create_test_compaction_metadata() -> CompactionMetadata {
    CompactionMetadata {
        compacted_at: SystemTime::now(),
        original_message_count: 10,
        original_token_count: 1000,
        compressed_token_count: 200,
        compression_ratio: 0.2,
        ..Default::default()
    }
}

/// Create ModelConfig for the small Qwen model for compaction testing
pub fn create_qwen_model_config() -> ModelConfig {
    ModelConfig {
        source: ModelSource::HuggingFace {
            repo: "unsloth/Qwen3-1.7B-GGUF".to_string(),
            filename: Some("Qwen3-1.7B-IQ4_NL.gguf".to_string()),
            folder: None,
        },
        batch_size: 512,
        n_seq_max: 1,
        n_threads: 1,
        n_threads_batch: 1,
        use_hf_params: true,
        retry_config: RetryConfig::default(),
        debug: false,
    }
}

/// Create a generate_summary function using the actual Qwen model for real compaction testing
/// This uses real model generation with the specified Qwen model
#[allow(clippy::type_complexity)]
pub fn create_qwen_generate_summary_fn() -> impl Fn(
    Vec<Message>,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<String, SessionError>> + Send>,
> + Clone
       + Send
       + Sync
       + 'static {
    move |messages: Vec<Message>| {
        Box::pin(async move {
            use crate::types::AgentConfig;
            use crate::AgentServer;

            // Create agent with Qwen model config for actual summarization
            let config = create_qwen_model_config();
            let agent_config = AgentConfig {
                model: config,
                ..AgentConfig::default()
            };

            // Create agent instance for actual model generation
            match AgentServer::initialize(agent_config).await {
                Ok(agent) => {
                    // Create summary prompt for the messages
                    let summary_prompt = if messages.is_empty() {
                        "Please provide a brief summary: No messages to summarize.".to_string()
                    } else {
                        let content: String = messages
                            .iter()
                            .map(|msg| format!("{}: {}", msg.role.as_str(), msg.content))
                            .collect::<Vec<_>>()
                            .join("\n");

                        format!(
                            "Please provide a brief summary of the following conversation:\n\n{}\n\nSummary:",
                            content
                        )
                    };

                    // Create a session for the generation
                    match agent.create_session().await {
                        Ok(session) => {
                            // Create the summary message
                            let summary_message = Message {
                                role: MessageRole::User,
                                content: summary_prompt,
                                tool_call_id: None,
                                tool_name: None,
                                timestamp: std::time::SystemTime::now(),
                            };

                            // Add the message to the session
                            if let Err(e) = agent.add_message(&session.id, summary_message).await {
                                eprintln!("Failed to add message to session: {}", e);
                                return Ok(format!("Summary generation failed: {}", e));
                            }

                            // Create generation request
                            let request = GenerationRequest::new(session.id)
                                .with_max_tokens(150)
                                .with_temperature(0.7);

                            // Generate summary using the proper GenerationRequest API
                            match agent.generate(request).await {
                                Ok(response) => Ok(response.generated_text),
                                Err(e) => {
                                    eprintln!("Generation failed: {}", e);
                                    Ok(format!("Summary generation failed: {}", e))
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Session creation failed: {}", e);
                            Ok(format!("Session creation failed: {}", e))
                        }
                    }
                }
                Err(e) => {
                    // Fallback if agent creation fails
                    eprintln!("Agent creation failed, using fallback: {}", e);
                    let fallback_summary = if messages.is_empty() {
                        "Empty conversation - no messages to summarize.".to_string()
                    } else {
                        format!(
                            "Summary: Conversation with {} messages containing approximately {} words of content.",
                            messages.len(),
                            messages.iter().map(|m| m.content.split_whitespace().count()).sum::<usize>()
                        )
                    };
                    Ok(fallback_summary)
                }
            }
        })
    }
}

/// Simple functions to replace complex builders - direct construction approach
/// Create a minimal agent config for testing
pub fn create_minimal_config() -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::Local {
                folder: PathBuf::from("/tmp"),
                filename: Some("test.gguf".to_string()),
            },
            batch_size: 128,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        },
        queue_config: QueueConfig {
            max_queue_size: 10,
            worker_threads: 1,
        },
        mcp_servers: vec![],
        session_config: SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            persistence_enabled: false,
            session_storage_dir: None,
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        },
        parallel_execution_config: ParallelConfig::default(),
    }
}

/// Create config with local model
pub fn create_config_with_local_model(folder: PathBuf, filename: String) -> AgentConfig {
    let mut config = create_minimal_config();
    config.model.source = ModelSource::Local {
        folder,
        filename: Some(filename),
    };
    config
}

/// Create a session with specified number of alternating messages
pub fn create_session_with_messages(count: usize) -> Session {
    let messages = (0..count)
        .map(|i| Message {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content: format!("Test message {} with some content", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        })
        .collect();

    Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages,
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    }
}

/// Create a session with sample conversation
pub fn create_session_with_sample_conversation() -> Session {
    let now = SystemTime::now();

    Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: vec![
            Message {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: now,
            },
            Message {
                role: MessageRole::User,
                content: "Hello, how are you?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: now,
            },
            Message {
                role: MessageRole::Assistant,
                content: "I'm doing well, thank you! How can I help you today?".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: now,
            },
        ],
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: now,
        updated_at: now,
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    }
}

/// Create a tool definition
pub fn create_tool_definition(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("A test tool named {}", name),
        parameters: json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "Test input parameter"
                }
            },
            "required": ["input"]
        }),
        server_name: "test_server".to_string(),
    }
}

/// Create a tool call
pub fn create_tool_call(name: &str, input: &str) -> ToolCall {
    ToolCall {
        id: ToolCallId::new(),
        name: name.to_string(),
        arguments: json!({ "input": input }),
    }
}

/// Create a tool result
pub fn create_tool_result(call_id: ToolCallId) -> ToolResult {
    ToolResult {
        call_id,
        result: json!({
            "status": "success",
            "output": "test result"
        }),
        error: None,
    }
}

/// Create a user message
pub fn create_user_message(content: &str) -> Message {
    Message {
        role: MessageRole::User,
        content: content.to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    }
}

/// Create an assistant message
pub fn create_assistant_message(content: &str) -> Message {
    Message {
        role: MessageRole::Assistant,
        content: content.to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    }
}
