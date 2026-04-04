//! Comprehensive coverage tests for llama-agent crate.
//!
//! Tests pure logic, type constructors, serialization, error handling,
//! and utility functions that do not require loading actual LLM models.

use llama_agent::acp::translation::{
    acp_to_llama_messages, acp_to_llama_session_id, infer_tool_kind, llama_to_acp_content,
    llama_to_acp_session_id, needs_permission, tool_call_to_acp, tool_definition_to_acp_format,
    tool_definitions_to_acp_format, tool_result_to_acp_update, ToJsonRpcError, TranslationError,
};
use llama_agent::chat_template::{
    BufferedStreamingParser, ChatTemplateEngine, ClaudeToolParser, DefaultToolParser,
    FunctionCallParser, JsonToolCallParser, OpenAIToolParser, Qwen3CoderStreamingParser,
    Qwen3CoderToolParser, StreamingState, StreamingToolCallParser, ToolCallParser,
    ToolParserFactory, ToolParsingStrategy, XmlToolCallParser,
};
use llama_agent::dependency_analysis::{DependencyAnalyzer, ParallelExecutionDecision};
use llama_agent::generation::{GenerationConfig, GenerationError};
use llama_agent::types::*;
use serde_json::{json, Value};
use std::time::{Duration, SystemTime};

// ============================================================================
// Helper functions
// ============================================================================

/// Create a minimal test session for tests that need one.
fn make_session() -> Session {
    Session {
        id: SessionId::new(),
        messages: Vec::new(),
        cwd: std::path::PathBuf::from("/tmp"),
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    }
}

/// Create a test ToolDefinition.
fn make_tool_def(name: &str, desc: &str, server: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: desc.to_string(),
        parameters: json!({"type": "object"}),
        server_name: server.to_string(),
    }
}

/// Create a test ToolCall.
fn make_tool_call(name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: ToolCallId::new(),
        name: name.to_string(),
        arguments: args,
    }
}

// ============================================================================
// ToolParsingStrategy tests
// ============================================================================

#[test]
fn test_tool_parsing_strategy_default() {
    assert_eq!(ToolParsingStrategy::default(), ToolParsingStrategy::Default);
}

#[test]
fn test_detect_from_model_name_qwen3coder() {
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"),
        ToolParsingStrategy::Qwen3Coder
    );
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("QWEN3-CODER-MINI"),
        ToolParsingStrategy::Qwen3Coder
    );
}

#[test]
fn test_detect_from_model_name_openai() {
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("gpt-3.5-turbo"),
        ToolParsingStrategy::OpenAI
    );
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("openai-compat-model"),
        ToolParsingStrategy::OpenAI
    );
}

#[test]
fn test_detect_from_model_name_claude() {
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("claude-3-sonnet"),
        ToolParsingStrategy::Claude
    );
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("anthropic-model"),
        ToolParsingStrategy::Claude
    );
}

#[test]
fn test_detect_from_model_name_default_fallback() {
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("microsoft/Phi-3-mini-4k-instruct"),
        ToolParsingStrategy::Default
    );
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("meta-llama/Llama-3.1"),
        ToolParsingStrategy::Default
    );
}

#[test]
fn test_detect_from_model_name_qwen_not_coder() {
    // Regular Qwen models should NOT match Qwen3Coder
    assert_eq!(
        ToolParsingStrategy::detect_from_model_name("Qwen/Qwen2.5-7B-Instruct"),
        ToolParsingStrategy::Default
    );
}

// ============================================================================
// ChatTemplateEngine tests
// ============================================================================

#[test]
fn test_chat_template_engine_new() {
    let engine = ChatTemplateEngine::new();
    assert!(engine.get_parsing_strategy().is_none());
}

#[test]
fn test_chat_template_engine_default() {
    let engine = ChatTemplateEngine::default();
    assert!(engine.get_parsing_strategy().is_none());
}

#[test]
fn test_chat_template_engine_with_model_strategy() {
    let engine = ChatTemplateEngine::with_model_strategy("gpt-3.5-turbo");
    assert_eq!(
        engine.get_parsing_strategy(),
        Some(&ToolParsingStrategy::OpenAI)
    );
}

#[test]
fn test_chat_template_engine_set_parsing_strategy() {
    let mut engine = ChatTemplateEngine::new();
    engine.set_parsing_strategy(ToolParsingStrategy::Qwen3Coder);
    assert_eq!(
        engine.get_parsing_strategy(),
        Some(&ToolParsingStrategy::Qwen3Coder)
    );
}

#[test]
fn test_chat_template_engine_debug() {
    let engine = ChatTemplateEngine::new();
    let debug_str = format!("{:?}", engine);
    assert!(debug_str.contains("ChatTemplateEngine"));
}

#[test]
fn test_chat_template_engine_register_parser() {
    let mut engine = ChatTemplateEngine::new();
    engine.register_parser("custom".to_string(), Box::new(JsonToolCallParser::new()));
    // Parser should be registered without error
}

// ============================================================================
// JsonToolCallParser tests
// ============================================================================

#[test]
fn test_json_parser_function_name_format() {
    let parser = JsonToolCallParser::new();
    // Format 1: function_name + arguments
    let text = r#"{"function_name": "read_file", "arguments": {"path": "/tmp/test.txt"}}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
    assert_eq!(calls[0].arguments["path"], "/tmp/test.txt");
}

#[test]
fn test_json_parser_tool_parameters_format() {
    let parser = JsonToolCallParser::new();
    // Format 2: tool + parameters
    let text =
        r#"{"tool": "write_file", "parameters": {"path": "/tmp/out.txt", "content": "hello"}}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "write_file");
}

#[test]
fn test_json_parser_name_args_format() {
    let parser = JsonToolCallParser::new();
    // Format 3: name + args
    let text = r#"{"name": "list_files", "args": {"dir": "/home"}}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "list_files");
}

#[test]
fn test_json_parser_no_tool_calls() {
    let parser = JsonToolCallParser::new();
    let text = "Hello, I can help with that. Let me think about your question.";
    let calls = parser.parse_tool_calls(text).unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_json_parser_embedded_in_text() {
    let parser = JsonToolCallParser::new();
    let text = r#"I need to read the file.
{"function_name": "read_file", "arguments": {"path": "/home/user/data.txt"}}
Let me analyze the result."#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
}

#[test]
fn test_json_parser_multiple_tool_calls() {
    let parser = JsonToolCallParser::new();
    let text = r#"{"function_name": "tool_a", "arguments": {"x": 1}}
{"function_name": "tool_b", "arguments": {"y": 2}}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert!(!calls.is_empty());
}

#[test]
fn test_json_parser_unrecognized_format_returns_empty() {
    let parser = JsonToolCallParser::new();
    // JSON that doesn't match any of the 3 recognized formats
    let text = r#"{"status": "ok", "data": "result"}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert!(calls.is_empty());
}

// ============================================================================
// XmlToolCallParser tests
// ============================================================================

#[test]
fn test_xml_parser_basic() {
    let parser = XmlToolCallParser::new();
    // XmlToolCallParser expects <function_call name="...">...</function_call> format
    let text = r#"<function_call name="read_file">{"path": "/tmp/test.txt"}</function_call>"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
}

#[test]
fn test_xml_parser_no_matches() {
    let parser = XmlToolCallParser::new();
    let text = "Just a regular text response with no tool calls.";
    let calls = parser.parse_tool_calls(text).unwrap();
    assert!(calls.is_empty());
}

// ============================================================================
// FunctionCallParser tests
// ============================================================================

#[test]
fn test_function_call_parser_basic() {
    let parser = FunctionCallParser::new();
    // FunctionCallParser expects "Call function_name with arguments {...}" format
    let text = r#"Call read_file with arguments {"path": "/tmp/test.txt"}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
}

#[test]
fn test_function_call_parser_no_matches() {
    let parser = FunctionCallParser::new();
    let text = "I will help you with your request.";
    let calls = parser.parse_tool_calls(text).unwrap();
    assert!(calls.is_empty());
}

// ============================================================================
// DefaultToolParser tests
// ============================================================================

#[test]
fn test_default_parser_tries_all_formats() {
    let parser = DefaultToolParser::new();
    // DefaultToolParser tries JSON, XML, and function call formats in order
    let text = r#"{"function_name": "test_tool", "arguments": {"key": "value"}}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "test_tool");
}

#[test]
fn test_default_parser_empty_text() {
    let parser = DefaultToolParser::new();
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_default_parser_default_impl() {
    let parser = DefaultToolParser::default();
    let calls = parser.parse_tool_calls("no tools here").unwrap();
    assert!(calls.is_empty());
}

// ============================================================================
// Qwen3CoderToolParser tests
// ============================================================================

#[test]
fn test_qwen3coder_parser_xml_format() {
    let parser = Qwen3CoderToolParser::new();
    // Qwen3Coder format: <tool_call><tool_name>parameters</tool_name></tool_call>
    // The first inner tag name becomes the tool name
    let text = r#"<tool_call>
<read_file>
<path>/tmp/test.txt</path>
</read_file>
</tool_call>"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
}

#[test]
fn test_qwen3coder_parser_no_tool_calls() {
    let parser = Qwen3CoderToolParser::new();
    let calls = parser.parse_tool_calls("Let me think about this.").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_qwen3coder_parser_default_impl() {
    let parser = Qwen3CoderToolParser::default();
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_qwen3coder_parser_with_schema() {
    let tools = vec![make_tool_def("test_tool", "A test tool", "test_server")];
    let parser = Qwen3CoderToolParser::new_with_schema(Some(&tools));
    let calls = parser.parse_tool_calls("no tools").unwrap();
    assert!(calls.is_empty());
}

// ============================================================================
// OpenAIToolParser tests
// ============================================================================

#[test]
fn test_openai_parser_function_call_format() {
    let parser = OpenAIToolParser::new();
    let text =
        r#"{"function_call": {"name": "get_weather", "arguments": {"location": "New York"}}}"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "get_weather");
    assert_eq!(calls[0].arguments["location"], "New York");
}

#[test]
fn test_openai_parser_no_matches_falls_back() {
    let parser = OpenAIToolParser::new();
    let text = "I'm just thinking about your question.";
    let calls = parser.parse_tool_calls(text).unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_openai_parser_default_impl() {
    let parser = OpenAIToolParser::default();
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

// ============================================================================
// ClaudeToolParser tests
// ============================================================================

#[test]
fn test_claude_parser_function_calls_format() {
    let parser = ClaudeToolParser::new();
    let text = r#"<function_calls>
<invoke name="read_file">
<parameter name="path">/tmp/test.txt</parameter>
</invoke>
</function_calls>"#;
    let calls = parser.parse_tool_calls(text).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "read_file");
}

#[test]
fn test_claude_parser_no_matches() {
    let parser = ClaudeToolParser::new();
    let calls = parser.parse_tool_calls("Here's what I found:").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_claude_parser_default_impl() {
    let parser = ClaudeToolParser::default();
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

// ============================================================================
// ToolParserFactory tests
// ============================================================================

#[test]
fn test_factory_create_parser_default() {
    let parser = ToolParserFactory::create_parser(ToolParsingStrategy::Default);
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_factory_create_parser_qwen3coder() {
    let parser = ToolParserFactory::create_parser(ToolParsingStrategy::Qwen3Coder);
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_factory_create_parser_openai() {
    let parser = ToolParserFactory::create_parser(ToolParsingStrategy::OpenAI);
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_factory_create_parser_claude() {
    let parser = ToolParserFactory::create_parser(ToolParsingStrategy::Claude);
    let calls = parser.parse_tool_calls("").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_factory_create_streaming_parser() {
    let parser = ToolParserFactory::create_streaming_parser(ToolParsingStrategy::Qwen3Coder);
    assert!(!parser.is_parsing_tool_call());
    let calls = parser.get_completed_tool_calls();
    assert!(calls.is_empty());
}

// ============================================================================
// StreamingState tests
// ============================================================================

#[test]
fn test_streaming_state_new() {
    let state = StreamingState::new();
    assert!(state.buffer().is_empty());
    assert!(!state.in_tool_call());
    assert!(state.current_tool_name().is_none());
    assert!(state.open_tags().is_empty());
    assert!(state.completed_tool_calls().is_empty());
    assert!(!state.is_complete_tool_call());
}

#[test]
fn test_streaming_state_default() {
    let state = StreamingState::default();
    assert!(state.buffer().is_empty());
}

#[test]
fn test_streaming_state_set_in_tool_call() {
    let mut state = StreamingState::new();
    state.set_in_tool_call(true);
    assert!(state.in_tool_call());
    state.set_in_tool_call(false);
    assert!(!state.in_tool_call());
}

#[test]
fn test_streaming_state_set_current_tool_name() {
    let mut state = StreamingState::new();
    state.set_current_tool_name(Some("test_tool".to_string()));
    assert_eq!(state.current_tool_name(), Some("test_tool"));
    state.set_current_tool_name(None);
    assert!(state.current_tool_name().is_none());
}

#[test]
fn test_streaming_state_buffer_mut() {
    let mut state = StreamingState::new();
    state.buffer_mut().push_str("hello");
    assert_eq!(state.buffer(), "hello");
}

#[test]
fn test_streaming_state_open_tags_mut() {
    let mut state = StreamingState::new();
    state.open_tags_mut().push("tool_call".to_string());
    assert_eq!(state.open_tags().len(), 1);
}

#[test]
fn test_streaming_state_add_completed_tool_call() {
    let mut state = StreamingState::new();
    let tc = ToolCall {
        id: ToolCallId::new(),
        name: "test".to_string(),
        arguments: json!({}),
    };
    state.add_completed_tool_call(tc);
    assert_eq!(state.completed_tool_calls().len(), 1);
}

#[test]
fn test_streaming_state_reset() {
    let mut state = StreamingState::new();
    state.buffer_mut().push_str("data");
    state.set_in_tool_call(true);
    state.set_current_tool_name(Some("tool".to_string()));
    state.reset();
    assert!(state.buffer().is_empty());
    assert!(!state.in_tool_call());
    assert!(state.current_tool_name().is_none());
}

// ============================================================================
// Qwen3CoderStreamingParser tests
// ============================================================================

#[test]
fn test_qwen3coder_streaming_parser_new() {
    let parser = Qwen3CoderStreamingParser::new();
    assert!(!parser.is_parsing_tool_call());
    assert!(parser.get_completed_tool_calls().is_empty());
}

#[test]
fn test_qwen3coder_streaming_parser_default() {
    let parser = Qwen3CoderStreamingParser::default();
    assert!(!parser.is_parsing_tool_call());
}

#[test]
fn test_qwen3coder_streaming_parser_process_text() {
    let mut parser = Qwen3CoderStreamingParser::new();
    let calls = parser.process_delta("Hello, world!").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_qwen3coder_streaming_parser_process_tool_call() {
    let mut parser = Qwen3CoderStreamingParser::new();
    let _ = parser.process_delta("<tool_call>").unwrap();
    assert!(parser.is_parsing_tool_call());
}

#[test]
fn test_qwen3coder_streaming_parser_reset() {
    let mut parser = Qwen3CoderStreamingParser::new();
    let _ = parser.process_delta("<tool_call>").unwrap();
    parser.reset();
    assert!(!parser.is_parsing_tool_call());
}

#[test]
fn test_qwen3coder_streaming_parser_get_state() {
    let parser = Qwen3CoderStreamingParser::new();
    let state = parser.get_state();
    assert!(state.buffer().is_empty());
}

#[test]
fn test_qwen3coder_streaming_parser_with_schema() {
    let tools = vec![make_tool_def("my_tool", "desc", "srv")];
    let parser = Qwen3CoderStreamingParser::new_with_schema(Some(&tools));
    assert!(!parser.is_parsing_tool_call());
}

// ============================================================================
// BufferedStreamingParser tests
// ============================================================================

#[test]
fn test_buffered_streaming_parser_new() {
    let base = Box::new(JsonToolCallParser::new());
    let parser = BufferedStreamingParser::new(base);
    assert!(!parser.is_parsing_tool_call());
    assert!(parser.get_completed_tool_calls().is_empty());
}

#[test]
fn test_buffered_streaming_parser_process_text() {
    let base = Box::new(JsonToolCallParser::new());
    let mut parser = BufferedStreamingParser::new(base);
    let calls = parser.process_delta("just text").unwrap();
    assert!(calls.is_empty());
}

#[test]
fn test_buffered_streaming_parser_reset() {
    let base = Box::new(JsonToolCallParser::new());
    let mut parser = BufferedStreamingParser::new(base);
    let _ = parser.process_delta("some data").unwrap();
    parser.reset();
    assert!(parser.get_completed_tool_calls().is_empty());
}

// ============================================================================
// acp/translation tests
// ============================================================================

#[test]
fn test_acp_to_llama_messages_text() {
    use agent_client_protocol::ContentBlock;
    let content = vec![ContentBlock::from("Hello, world!")];
    let messages = acp_to_llama_messages(content).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello, world!");
    assert_eq!(messages[0].role, MessageRole::User);
}

#[test]
fn test_acp_to_llama_messages_empty() {
    let content = vec![];
    let messages = acp_to_llama_messages(content).unwrap();
    assert!(messages.is_empty());
}

#[test]
fn test_acp_to_llama_messages_multiple_text() {
    use agent_client_protocol::ContentBlock;
    let content = vec![ContentBlock::from("First"), ContentBlock::from("Second")];
    let messages = acp_to_llama_messages(content).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "First");
    assert_eq!(messages[1].content, "Second");
}

#[test]
fn test_acp_to_llama_session_id_valid() {
    use agent_client_protocol::SessionId as AcpSessionId;
    let llama_id = SessionId::new();
    let acp_id = AcpSessionId::new(llama_id.to_string());
    let result = acp_to_llama_session_id(acp_id);
    assert!(result.is_ok());
}

#[test]
fn test_acp_to_llama_session_id_invalid() {
    use agent_client_protocol::SessionId as AcpSessionId;
    let acp_id = AcpSessionId::new("not-a-valid-ulid");
    let result = acp_to_llama_session_id(acp_id);
    assert!(result.is_err());
}

#[test]
fn test_llama_to_acp_session_id() {
    let llama_id = SessionId::new();
    let acp_id = llama_to_acp_session_id(llama_id);
    assert_eq!(acp_id.0.as_ref(), &llama_id.to_string());
}

#[test]
fn test_llama_to_acp_content() {
    let messages = vec![
        Message {
            role: MessageRole::Assistant,
            content: "Hello!".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        },
        Message {
            role: MessageRole::User,
            content: "World!".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        },
    ];
    let content = llama_to_acp_content(messages);
    assert_eq!(content.len(), 2);
}

#[test]
fn test_llama_to_acp_content_empty() {
    let content = llama_to_acp_content(vec![]);
    assert!(content.is_empty());
}

#[test]
fn test_tool_definition_to_acp_format() {
    let tool = make_tool_def("test_tool", "A test", "server1");
    let json = tool_definition_to_acp_format(&tool);
    assert_eq!(json["name"], "test_tool");
    assert_eq!(json["description"], "A test");
    assert_eq!(json["server"], "server1");
}

#[test]
fn test_tool_definitions_to_acp_format() {
    let tools = vec![
        make_tool_def("tool_a", "Tool A", "s1"),
        make_tool_def("tool_b", "Tool B", "s2"),
    ];
    let json = tool_definitions_to_acp_format(&tools);
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[test]
fn test_tool_definitions_to_acp_format_empty() {
    let json = tool_definitions_to_acp_format(&[]);
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[test]
fn test_tool_call_to_acp_with_def() {
    let tc = make_tool_call("fs_read", json!({"path": "/tmp"}));
    let def = make_tool_def("fs_read", "Read a file", "filesystem");
    let acp_call = tool_call_to_acp(tc, Some(&def));
    // Verify it was created without panic
    let _ = format!("{:?}", acp_call);
}

#[test]
fn test_tool_call_to_acp_without_def() {
    let tc = make_tool_call("unknown_tool", json!({"x": 1}));
    let acp_call = tool_call_to_acp(tc, None);
    let _ = format!("{:?}", acp_call);
}

#[test]
fn test_tool_result_to_acp_update_success() {
    let result = ToolResult {
        call_id: ToolCallId::new(),
        result: json!({"status": "ok"}),
        error: None,
    };
    let update = tool_result_to_acp_update(result);
    let _ = format!("{:?}", update);
}

#[test]
fn test_tool_result_to_acp_update_error() {
    let result = ToolResult {
        call_id: ToolCallId::new(),
        result: Value::Null,
        error: Some("File not found".to_string()),
    };
    let update = tool_result_to_acp_update(result);
    let _ = format!("{:?}", update);
}

// ============================================================================
// infer_tool_kind tests
// ============================================================================

#[test]
fn test_infer_tool_kind_read() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("fs_read"), ToolKind::Read);
    assert_eq!(infer_tool_kind("get_file"), ToolKind::Read);
    assert_eq!(infer_tool_kind("list_files"), ToolKind::Read);
    assert_eq!(infer_tool_kind("show_content"), ToolKind::Read);
    assert_eq!(infer_tool_kind("view_data"), ToolKind::Read);
    assert_eq!(infer_tool_kind("load_config"), ToolKind::Read);
    assert_eq!(infer_tool_kind("glob_files"), ToolKind::Read);
}

#[test]
fn test_infer_tool_kind_edit() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("fs_write"), ToolKind::Edit);
    assert_eq!(infer_tool_kind("create_file"), ToolKind::Edit);
    assert_eq!(infer_tool_kind("update_record"), ToolKind::Edit);
    assert_eq!(infer_tool_kind("edit_document"), ToolKind::Edit);
    assert_eq!(infer_tool_kind("modify_settings"), ToolKind::Edit);
}

#[test]
fn test_infer_tool_kind_delete() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("delete_file"), ToolKind::Delete);
    assert_eq!(infer_tool_kind("remove_item"), ToolKind::Delete);
}

#[test]
fn test_infer_tool_kind_execute() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("execute_command"), ToolKind::Execute);
    assert_eq!(infer_tool_kind("shell"), ToolKind::Execute);
    assert_eq!(infer_tool_kind("terminal"), ToolKind::Execute);
    assert_eq!(infer_tool_kind("run_script"), ToolKind::Execute);
    assert_eq!(infer_tool_kind("bash_cmd"), ToolKind::Execute);
}

#[test]
fn test_infer_tool_kind_think() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("think"), ToolKind::Think);
    assert_eq!(infer_tool_kind("plan_work"), ToolKind::Think);
    assert_eq!(infer_tool_kind("reason_about"), ToolKind::Think);
    assert_eq!(infer_tool_kind("analyze_code"), ToolKind::Think);
}

#[test]
fn test_infer_tool_kind_search() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("search_files"), ToolKind::Search);
    assert_eq!(infer_tool_kind("grep_code"), ToolKind::Search);
    assert_eq!(infer_tool_kind("find_symbol"), ToolKind::Search);
}

#[test]
fn test_infer_tool_kind_move() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("move_file"), ToolKind::Move);
    assert_eq!(infer_tool_kind("rename_item"), ToolKind::Move);
}

#[test]
fn test_infer_tool_kind_fetch() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("http_get"), ToolKind::Fetch);
    assert_eq!(infer_tool_kind("web_request"), ToolKind::Fetch);
    assert_eq!(infer_tool_kind("url_open"), ToolKind::Fetch);
}

#[test]
fn test_infer_tool_kind_other() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("some_custom_tool"), ToolKind::Other);
    assert_eq!(infer_tool_kind("misc_operation"), ToolKind::Other);
}

#[test]
fn test_infer_tool_kind_rm_as_word() {
    use agent_client_protocol::ToolKind;
    assert_eq!(infer_tool_kind("fs_rm"), ToolKind::Delete);
    // "rm" inside another word should NOT match delete
    // (but "mv" contains "mv" and matches Move)
}

// ============================================================================
// needs_permission tests
// ============================================================================

#[test]
fn test_needs_permission_read_operations() {
    assert!(!needs_permission("fs_read"));
    assert!(!needs_permission("get_file"));
    assert!(!needs_permission("list_files"));
    assert!(!needs_permission("show_content"));
    assert!(!needs_permission("view_data"));
    assert!(!needs_permission("load_config"));
    assert!(!needs_permission("search_code"));
    assert!(!needs_permission("grep_files"));
    assert!(!needs_permission("find_symbol"));
    assert!(!needs_permission("glob_match"));
    // Note: "fetch_url" contains "url" which is a network indicator, so it requires permission
    assert!(needs_permission("fetch_url"));
}

#[test]
fn test_needs_permission_write_operations() {
    assert!(needs_permission("fs_write"));
    assert!(needs_permission("create_file"));
    assert!(needs_permission("update_record"));
    assert!(needs_permission("edit_document"));
    assert!(needs_permission("modify_settings"));
}

#[test]
fn test_needs_permission_delete_operations() {
    assert!(needs_permission("delete_file"));
    assert!(needs_permission("remove_item"));
}

#[test]
fn test_needs_permission_execute_operations() {
    assert!(needs_permission("execute_command"));
    assert!(needs_permission("shell_run"));
    assert!(needs_permission("terminal_create"));
    assert!(needs_permission("run_script"));
    assert!(needs_permission("bash_exec"));
}

#[test]
fn test_needs_permission_network_operations() {
    assert!(needs_permission("http_get"));
    assert!(needs_permission("web_fetch"));
    assert!(needs_permission("url_open"));
}

#[test]
fn test_needs_permission_move_operations() {
    assert!(needs_permission("move_file"));
    assert!(needs_permission("rename_item"));
}

#[test]
fn test_needs_permission_unknown_defaults_to_true() {
    assert!(needs_permission("some_unknown_tool"));
    assert!(needs_permission("custom_operation"));
}

#[test]
fn test_needs_permission_rm_as_word() {
    assert!(needs_permission("fs_rm"));
    // "rm" inside "swissarmyhammer" should NOT trigger delete detection
    assert!(!needs_permission("swissarmyhammer_read"));
}

// ============================================================================
// TranslationError tests
// ============================================================================

#[test]
fn test_translation_error_display() {
    let err = TranslationError::UnsupportedContent("test".to_string());
    assert!(err.to_string().contains("Unsupported content type"));

    let err = TranslationError::InvalidFormat("bad format".to_string());
    assert!(err.to_string().contains("Invalid content format"));

    let err = TranslationError::InvalidSessionId("bad id".to_string());
    assert!(err.to_string().contains("Invalid session ID"));
}

#[test]
fn test_translation_error_to_json_rpc() {
    let err = TranslationError::UnsupportedContent("test".to_string());
    assert_eq!(err.to_json_rpc_code(), -32602);

    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "unsupported_content_type");

    let rpc_err = err.to_json_rpc_error();
    assert_eq!(rpc_err.code, -32602);
    assert!(rpc_err.data.is_some());
}

#[test]
fn test_translation_error_invalid_format_to_json_rpc() {
    let err = TranslationError::InvalidFormat("bad".to_string());
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "invalid_content_format");
}

#[test]
fn test_translation_error_invalid_session_id_to_json_rpc() {
    let err = TranslationError::InvalidSessionId("bad".to_string());
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "invalid_session_id");
}

// ============================================================================
// ToJsonRpcError for various error types
// ============================================================================

#[test]
fn test_agent_error_to_json_rpc() {
    let err = AgentError::Timeout {
        timeout: Duration::from_secs(30),
    };
    assert_eq!(err.to_json_rpc_code(), -32000);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "request_timeout");
    assert_eq!(data["timeoutSeconds"], 30);

    let err = AgentError::QueueFull { capacity: 100 };
    assert_eq!(err.to_json_rpc_code(), -32000);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "queue_overloaded");
    assert_eq!(data["capacity"], 100);
}

#[test]
fn test_queue_error_to_json_rpc() {
    let err = QueueError::Full;
    assert_eq!(err.to_json_rpc_code(), -32000);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "queue_full");

    let err = QueueError::WorkerError("test failure".to_string());
    assert_eq!(err.to_json_rpc_code(), -32603);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "worker_error");
    assert_eq!(data["details"], "test failure");
}

#[test]
fn test_session_error_to_json_rpc() {
    let err = SessionError::NotFound("test-id".to_string());
    assert_eq!(err.to_json_rpc_code(), -32602);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "session_not_found");

    let err = SessionError::LimitExceeded;
    assert_eq!(err.to_json_rpc_code(), -32000);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "session_limit_exceeded");

    let err = SessionError::Timeout;
    assert_eq!(err.to_json_rpc_code(), -32000);

    let err = SessionError::InvalidState("bad state".to_string());
    assert_eq!(err.to_json_rpc_code(), -32602);
}

#[test]
fn test_mcp_error_to_json_rpc() {
    let variants = vec![
        (
            MCPError::ServerNotFound("srv".to_string()),
            -32602,
            "mcp_server_not_found",
        ),
        (
            MCPError::ToolCallFailed("fail".to_string()),
            -32000,
            "mcp_tool_call_failed",
        ),
        (
            MCPError::Connection("conn".to_string()),
            -32000,
            "mcp_connection_error",
        ),
        (
            MCPError::Protocol("proto".to_string()),
            -32600,
            "mcp_protocol_error",
        ),
        (
            MCPError::HttpUrlInvalid("url".to_string()),
            -32602,
            "mcp_http_url_invalid",
        ),
        (
            MCPError::HttpTimeout("timeout".to_string()),
            -32000,
            "mcp_http_timeout",
        ),
        (
            MCPError::HttpConnection("conn".to_string()),
            -32000,
            "mcp_http_connection_failed",
        ),
        (
            MCPError::Timeout("timeout".to_string()),
            -32000,
            "mcp_timeout",
        ),
    ];

    for (err, expected_code, expected_error) in variants {
        assert_eq!(err.to_json_rpc_code(), expected_code);
        let data = err.to_error_data().unwrap();
        assert_eq!(data["error"], expected_error);
    }
}

#[test]
fn test_template_error_to_json_rpc() {
    let err = TemplateError::RenderingFailed("render fail".to_string());
    assert_eq!(err.to_json_rpc_code(), -32602);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "template_rendering_failed");

    let err = TemplateError::ToolCallParsing("parse fail".to_string());
    assert_eq!(err.to_json_rpc_code(), -32602);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "template_tool_parsing_failed");

    let err = TemplateError::Invalid("invalid".to_string());
    assert_eq!(err.to_json_rpc_code(), -32602);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "template_invalid");
}

#[test]
fn test_generation_error_to_json_rpc() {
    let err = GenerationError::InvalidConfig("bad config".to_string());
    assert_eq!(err.to_json_rpc_code(), -32602);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "invalid_generation_config");

    let err = GenerationError::Cancelled;
    assert_eq!(err.to_json_rpc_code(), -32000);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "generation_cancelled");

    let err = GenerationError::StreamClosed;
    assert_eq!(err.to_json_rpc_code(), -32000);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "stream_closed");

    let err = GenerationError::Stopped("max tokens".to_string());
    assert_eq!(err.to_json_rpc_code(), -32000);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "generation_stopped");

    // Errors that return None for error_data
    let err = GenerationError::TokenizationFailed("tokenize".to_string());
    assert_eq!(err.to_json_rpc_code(), -32603);
    assert!(err.to_error_data().is_none());

    let err = GenerationError::BatchFailed("batch".to_string());
    assert_eq!(err.to_json_rpc_code(), -32603);

    let err = GenerationError::ContextLock;
    assert_eq!(err.to_json_rpc_code(), -32603);
}

#[test]
fn test_validation_error_to_json_rpc() {
    use llama_agent::validation::ValidationError;

    let err = ValidationError::security_violation("test");
    assert_eq!(err.to_json_rpc_code(), -32602);
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "security_violation");

    let err = ValidationError::parameter_bounds("test");
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "parameter_out_of_bounds");

    let err = ValidationError::invalid_state("test");
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "invalid_state");

    let err = ValidationError::content_validation("test");
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "content_validation_failed");

    let err = ValidationError::schema_validation("test");
    let data = err.to_error_data().unwrap();
    assert_eq!(data["error"], "schema_validation_failed");
}

#[test]
fn test_validation_error_multiple_to_json_rpc() {
    use llama_agent::validation::ValidationError;

    let combined = ValidationError::multiple(vec![
        ValidationError::security_violation("sec"),
        ValidationError::parameter_bounds("param"),
    ]);
    let data = combined.to_error_data().unwrap();
    assert_eq!(data["error"], "multiple_validation_errors");
    assert!(data["errors"].is_array());
    assert_eq!(data["errors"].as_array().unwrap().len(), 2);
}

// ============================================================================
// llama_chunk_to_acp_notification tests
// ============================================================================

#[test]
fn test_llama_chunk_to_acp_notification() {
    use llama_agent::acp::translation::llama_chunk_to_acp_notification;

    let session_id = agent_client_protocol::SessionId::new("test-session-id");
    let chunk = StreamChunk {
        text: "Hello, world!".to_string(),
        is_complete: false,
        token_count: 3,
        finish_reason: None,
    };
    let notification = llama_chunk_to_acp_notification(session_id, chunk);
    let _ = format!("{:?}", notification);
}

// ============================================================================
// dependency_analysis tests
// ============================================================================

#[test]
fn test_dependency_analyzer_single_tool() {
    let analyzer = DependencyAnalyzer::default();
    let calls = vec![make_tool_call("tool_a", json!({"x": 1}))];
    let decision = analyzer.analyze_parallel_execution(&calls);
    assert!(matches!(decision, ParallelExecutionDecision::Sequential(_)));
}

#[test]
fn test_dependency_analyzer_parallel_safe() {
    let analyzer = DependencyAnalyzer::default();
    let calls = vec![
        make_tool_call("tool_a", json!({"x": 1})),
        make_tool_call("tool_b", json!({"y": 2})),
    ];
    let decision = analyzer.analyze_parallel_execution(&calls);
    assert!(matches!(decision, ParallelExecutionDecision::Parallel));
}

#[test]
fn test_dependency_analyzer_duplicate_names() {
    let analyzer = DependencyAnalyzer::default();
    let calls = vec![
        make_tool_call("same_tool", json!({"x": 1})),
        make_tool_call("same_tool", json!({"x": 2})),
    ];
    let decision = analyzer.analyze_parallel_execution(&calls);
    assert!(matches!(decision, ParallelExecutionDecision::Sequential(_)));
}

#[test]
fn test_dependency_analyzer_parameter_dependencies() {
    let analyzer = DependencyAnalyzer::default();
    let calls = vec![
        make_tool_call("tool_a", json!({"x": 1})),
        make_tool_call("tool_b", json!({"input": "${tool_a}"})),
    ];
    let decision = analyzer.analyze_parallel_execution(&calls);
    assert!(matches!(decision, ParallelExecutionDecision::Sequential(_)));
}

#[test]
fn test_dependency_analyzer_configured_conflicts() {
    use llama_agent::types::{ConflictType, ToolConflict};

    let config = ParallelConfig {
        tool_conflicts: vec![ToolConflict {
            tool1: "write_file".to_string(),
            tool2: "read_file".to_string(),
            conflict_type: ConflictType::ResourceConflict,
            description: "File access conflict".to_string(),
        }],
        ..Default::default()
    };
    let analyzer = DependencyAnalyzer::new(config);
    let calls = vec![
        make_tool_call("write_file", json!({"path": "/tmp/a.txt"})),
        make_tool_call("read_file", json!({"path": "/tmp/a.txt"})),
    ];
    let decision = analyzer.analyze_parallel_execution(&calls);
    assert!(matches!(decision, ParallelExecutionDecision::Sequential(_)));
}

#[test]
fn test_dependency_analyzer_never_parallel() {
    let config = ParallelConfig {
        never_parallel: vec![("tool_x".to_string(), "tool_y".to_string())],
        ..Default::default()
    };
    let analyzer = DependencyAnalyzer::new(config);
    let calls = vec![
        make_tool_call("tool_x", json!({})),
        make_tool_call("tool_y", json!({})),
    ];
    let decision = analyzer.analyze_parallel_execution(&calls);
    assert!(matches!(decision, ParallelExecutionDecision::Sequential(_)));
}

#[test]
fn test_dependency_analyzer_empty_calls() {
    let analyzer = DependencyAnalyzer::default();
    let calls: Vec<ToolCall> = vec![];
    let decision = analyzer.analyze_parallel_execution(&calls);
    assert!(matches!(decision, ParallelExecutionDecision::Sequential(_)));
}

#[test]
fn test_dependency_analyzer_resource_conflict_file_operations() {
    let analyzer = DependencyAnalyzer::default();
    let calls = vec![
        make_tool_call("file_write", json!({"path": "/tmp/data.json"})),
        make_tool_call("file_read", json!({"path": "/tmp/data.json"})),
    ];
    let decision = analyzer.analyze_parallel_execution(&calls);
    // File operations with shared paths should detect conflict
    assert!(matches!(decision, ParallelExecutionDecision::Sequential(_)));
}

// ============================================================================
// GenerationConfig tests
// ============================================================================

#[test]
fn test_generation_config_default() {
    let config = GenerationConfig::default();
    assert_eq!(config.max_tokens, 4096);
    assert!((config.temperature - 0.7).abs() < f32::EPSILON);
    assert!((config.top_p - 0.9).abs() < f32::EPSILON);
    assert!(config.stop_tokens.is_empty());
    assert_eq!(config.seed, 1234);
    assert!(config.use_greedy);
}

#[test]
fn test_generation_config_for_batch() {
    let config = GenerationConfig::for_batch_generation();
    assert!(!config.use_greedy);
}

#[test]
fn test_generation_config_for_streaming() {
    let config = GenerationConfig::for_streaming();
    assert!(!config.use_greedy);
}

#[test]
fn test_generation_config_for_compaction() {
    let config = GenerationConfig::for_compaction();
    assert_eq!(config.max_tokens, 512);
    assert!((config.temperature - 0.0).abs() < f32::EPSILON);
    assert!(config.use_greedy);
}

#[test]
fn test_generation_config_validate_valid() {
    let config = GenerationConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_generation_config_validate_zero_tokens() {
    let config = GenerationConfig {
        max_tokens: 0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
    assert!(config.validate().unwrap_err().contains("max_tokens"));
}

#[test]
fn test_generation_config_validate_too_many_tokens() {
    let config = GenerationConfig {
        max_tokens: 200_000,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_generation_config_validate_bad_temperature() {
    let config = GenerationConfig {
        temperature: -0.1,
        ..Default::default()
    };
    assert!(config.validate().is_err());

    let config = GenerationConfig {
        temperature: 2.1,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_generation_config_validate_bad_top_p() {
    let config = GenerationConfig {
        top_p: -0.1,
        ..Default::default()
    };
    assert!(config.validate().is_err());

    let config = GenerationConfig {
        top_p: 1.1,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_generation_config_validate_too_many_stop_tokens() {
    let config = GenerationConfig {
        stop_tokens: (0..11).map(|i| format!("stop{}", i)).collect(),
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_generation_config_validate_empty_stop_token() {
    let config = GenerationConfig {
        stop_tokens: vec!["".to_string()],
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_generation_config_validate_long_stop_token() {
    let config = GenerationConfig {
        stop_tokens: vec!["a".repeat(51)],
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

// ============================================================================
// GenerationError tests
// ============================================================================

#[test]
fn test_generation_error_constructors() {
    let err = GenerationError::tokenization(std::io::Error::other("test"));
    assert!(err.to_string().contains("tokenize"));

    let err = GenerationError::batch(std::io::Error::other("test"));
    assert!(err.to_string().contains("Batch"));

    let err = GenerationError::decoding(std::io::Error::other("test"));
    assert!(err.to_string().contains("decoding"));

    let err = GenerationError::token_conversion(std::io::Error::other("test"));
    assert!(err.to_string().contains("conversion"));

    let err = GenerationError::context(std::io::Error::other("test"));
    assert!(err.to_string().contains("Context"));

    let err = GenerationError::generation(std::io::Error::other("test"));
    assert!(err.to_string().contains("Generation error"));
}

#[test]
fn test_generation_error_from_string() {
    let err: GenerationError = "config problem".to_string().into();
    assert!(matches!(err, GenerationError::InvalidConfig(_)));
}

// ============================================================================
// ID types tests
// ============================================================================

#[test]
fn test_session_id_new_and_display() {
    let id = SessionId::new();
    let s = id.to_string();
    assert!(!s.is_empty());
    let parsed: SessionId = s.parse().unwrap();
    assert_eq!(parsed, id);
}

#[test]
fn test_session_id_from_ulid() {
    let ulid = ulid::Ulid::new();
    let id = SessionId::from_ulid(ulid);
    assert_eq!(id.as_ulid(), ulid);
}

#[test]
fn test_session_id_default() {
    let id = SessionId::default();
    assert!(!id.to_string().is_empty());
}

#[test]
fn test_session_id_parse_invalid() {
    let result: Result<SessionId, _> = "invalid-ulid".parse();
    assert!(result.is_err());
}

#[test]
fn test_tool_call_id_new_and_display() {
    let id = ToolCallId::new();
    let s = id.to_string();
    let parsed: ToolCallId = s.parse().unwrap();
    assert_eq!(parsed, id);
}

#[test]
fn test_tool_call_id_default() {
    let id = ToolCallId::default();
    assert!(!id.to_string().is_empty());
}

#[test]
fn test_prompt_id_new_and_display() {
    use llama_agent::types::ids::PromptId;
    let id = PromptId::new();
    let s = id.to_string();
    let parsed: PromptId = s.parse().unwrap();
    assert_eq!(parsed, id);
}

// ============================================================================
// MessageRole tests
// ============================================================================

#[test]
fn test_message_role_as_str() {
    assert_eq!(MessageRole::System.as_str(), "system");
    assert_eq!(MessageRole::User.as_str(), "user");
    assert_eq!(MessageRole::Assistant.as_str(), "assistant");
    assert_eq!(MessageRole::Tool.as_str(), "tool");
}

#[test]
fn test_message_role_equality() {
    assert_eq!(MessageRole::System, MessageRole::System);
    assert_ne!(MessageRole::System, MessageRole::User);
}

// ============================================================================
// TokenUsage tests
// ============================================================================

#[test]
fn test_token_usage_new() {
    let usage = TokenUsage::new();
    assert_eq!(usage.total, 0);
    assert!(usage.by_role.is_empty());
    assert!(usage.by_message.is_empty());
}

#[test]
fn test_token_usage_default() {
    let usage = TokenUsage::default();
    assert_eq!(usage.total, 0);
}

// ============================================================================
// SimpleTokenCounter tests
// ============================================================================

#[test]
fn test_simple_token_counter() {
    let counter = SimpleTokenCounter::new();
    let count = counter.count_tokens("hello world foo bar");
    assert!(count > 0);
}

// ============================================================================
// ModelConfig tests
// ============================================================================

#[test]
fn test_model_config_default() {
    let config = ModelConfig::default();
    assert_eq!(config.batch_size, 512);
    assert_eq!(config.n_seq_max, 8);
    assert!(!config.debug);
}

#[test]
fn test_model_config_validate_valid() {
    let config = ModelConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_model_config_validate_zero_batch_size() {
    let config = ModelConfig {
        batch_size: 0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_model_config_validate_large_batch_size() {
    let config = ModelConfig {
        batch_size: 9000,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_model_config_compute_model_hash() {
    let config = ModelConfig::default();
    let hash = config.compute_model_hash();
    assert!(!hash.is_empty());
}

#[test]
fn test_model_config_compute_model_hash_deterministic() {
    let config = ModelConfig::default();
    assert_eq!(config.compute_model_hash(), config.compute_model_hash());
}

#[test]
fn test_model_config_resolver_config() {
    let config = ModelConfig::default();
    let resolver = config.resolver_config();
    assert_eq!(resolver.debug, config.debug);
}

// ============================================================================
// QueueConfig tests
// ============================================================================

#[test]
fn test_queue_config_default() {
    let config = QueueConfig::default();
    assert_eq!(config.max_queue_size, 100);
    assert_eq!(config.worker_threads, 1);
}

#[test]
fn test_queue_config_validate_valid() {
    let config = QueueConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_queue_config_validate_zero_queue_size() {
    let config = QueueConfig {
        max_queue_size: 0,
        worker_threads: 1,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_queue_config_validate_zero_workers() {
    let config = QueueConfig {
        max_queue_size: 10,
        worker_threads: 0,
    };
    assert!(config.validate().is_err());
}

// ============================================================================
// SessionConfig tests
// ============================================================================

#[test]
fn test_session_config_default() {
    let config = SessionConfig::default();
    assert_eq!(config.max_sessions, 1000);
    assert!(!config.persistence_enabled);
}

#[test]
fn test_session_config_validate_valid() {
    let config = SessionConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_session_config_validate_zero_max_sessions() {
    let config = SessionConfig {
        max_sessions: 0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_session_config_validate_persistence_with_zero_threshold() {
    let config = SessionConfig {
        persistence_enabled: true,
        auto_save_threshold: 0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_session_config_validate_empty_storage_dir() {
    let config = SessionConfig {
        session_storage_dir: Some(std::path::PathBuf::from("")),
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_session_config_get_model_kv_cache_dir() {
    let config = SessionConfig::default();
    let model_config = ModelConfig::default();
    let dir = config.get_model_kv_cache_dir(&model_config);
    // The directory should end with the model hash
    let hash = model_config.compute_model_hash();
    assert!(dir.to_string_lossy().contains(&hash));
}

// ============================================================================
// AgentConfig tests
// ============================================================================

#[test]
fn test_agent_config_default() {
    let config = AgentConfig::default();
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_agent_config_validate_valid() {
    let config = AgentConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_agent_config_validate_duplicate_mcp_servers() {
    let config = AgentConfig {
        mcp_servers: vec![
            MCPServerConfig::InProcess(ProcessServerConfig {
                name: "server-a".to_string(),
                command: "python".to_string(),
                args: vec![],
                timeout_secs: Some(30),
            }),
            MCPServerConfig::InProcess(ProcessServerConfig {
                name: "server-a".to_string(),
                command: "node".to_string(),
                args: vec![],
                timeout_secs: Some(30),
            }),
        ],
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

// ============================================================================
// StoppingConfig tests
// ============================================================================

#[test]
fn test_stopping_config_default() {
    let config = StoppingConfig::default();
    assert!(config.max_tokens.is_none());
    assert!(config.eos_detection);
}

#[test]
fn test_stopping_config_validate_valid() {
    let config = StoppingConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_stopping_config_validate_zero_tokens() {
    let config = StoppingConfig {
        max_tokens: Some(0),
        eos_detection: true,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_stopping_config_validate_too_many_tokens() {
    let config = StoppingConfig {
        max_tokens: Some(200_000),
        eos_detection: true,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_stopping_config_new_validated() {
    let config = StoppingConfig::new_validated(Some(100), true);
    assert!(config.is_ok());

    let config = StoppingConfig::new_validated(Some(0), true);
    assert!(config.is_err());
}

// ============================================================================
// GenerationRequest tests
// ============================================================================

#[test]
fn test_generation_request_new() {
    let id = SessionId::new();
    let req = GenerationRequest::new(id);
    assert!(req.max_tokens.is_none());
    assert!(req.temperature.is_none());
    assert!(req.top_p.is_none());
    assert!(req.stop_tokens.is_empty());
    assert!(req.stopping_config.is_none());
}

#[test]
fn test_generation_request_builder() {
    let id = SessionId::new();
    let req = GenerationRequest::new(id)
        .with_max_tokens(100)
        .with_temperature(0.5)
        .with_top_p(0.9)
        .with_stop_tokens(vec!["<|end|>".to_string()]);

    assert_eq!(req.max_tokens, Some(100));
    assert_eq!(req.temperature, Some(0.5));
    assert_eq!(req.top_p, Some(0.9));
    assert_eq!(req.stop_tokens.len(), 1);
}

#[test]
fn test_generation_request_with_default_stopping() {
    let id = SessionId::new();
    let req = GenerationRequest::new(id).with_default_stopping();
    assert!(req.stopping_config.is_some());
}

#[test]
fn test_generation_request_with_stopping_config() {
    let id = SessionId::new();
    let config = StoppingConfig {
        max_tokens: Some(50),
        eos_detection: false,
    };
    let req = GenerationRequest::new(id).with_stopping_config(config);
    assert_eq!(req.stopping_config.unwrap().max_tokens, Some(50));
}

#[test]
fn test_generation_request_effective_max_tokens() {
    let id = SessionId::new();

    // Direct max_tokens takes priority
    let req = GenerationRequest::new(id).with_max_tokens(100);
    assert_eq!(req.effective_max_tokens(), Some(100));

    // Stopping config max_tokens used as fallback
    let id = SessionId::new();
    let config = StoppingConfig {
        max_tokens: Some(50),
        eos_detection: true,
    };
    let req = GenerationRequest::new(id).with_stopping_config(config);
    assert_eq!(req.effective_max_tokens(), Some(50));

    // Neither set
    let id = SessionId::new();
    let req = GenerationRequest::new(id);
    assert_eq!(req.effective_max_tokens(), None);
}

#[test]
fn test_generation_request_migrate_max_tokens() {
    let id = SessionId::new();
    let req = GenerationRequest::new(id)
        .with_max_tokens(100)
        .migrate_max_tokens_to_stopping_config();
    assert!(req.max_tokens.is_none());
    assert_eq!(req.stopping_config.as_ref().unwrap().max_tokens, Some(100));
}

#[test]
fn test_generation_request_with_validated_stopping_config() {
    let id = SessionId::new();
    let config = StoppingConfig {
        max_tokens: Some(100),
        eos_detection: true,
    };
    let result = GenerationRequest::new(id).with_validated_stopping_config(config);
    assert!(result.is_ok());

    let id = SessionId::new();
    let config = StoppingConfig {
        max_tokens: Some(0),
        eos_detection: true,
    };
    let result = GenerationRequest::new(id).with_validated_stopping_config(config);
    assert!(result.is_err());
}

// ============================================================================
// ContextState tests
// ============================================================================

#[test]
fn test_context_state_new() {
    let state = llama_agent::types::sessions::ContextState::new();
    assert!(state.is_empty());
    assert_eq!(state.current_position, 0);
    assert_eq!(state.last_prompt_hash, 0);
    assert!(state.last_prompt_text.is_empty());
}

#[test]
fn test_context_state_update_and_match() {
    let mut state = llama_agent::types::sessions::ContextState::new();
    state.update(vec![1, 2, 3], "test prompt");
    assert!(!state.is_empty());
    assert_eq!(state.current_position, 3);
    assert!(state.matches_prompt("test prompt"));
    assert!(!state.matches_prompt("different prompt"));
}

#[test]
fn test_context_state_reset() {
    let mut state = llama_agent::types::sessions::ContextState::new();
    state.update(vec![1, 2, 3], "test");
    state.reset();
    assert!(state.is_empty());
    assert_eq!(state.current_position, 0);
}

// ============================================================================
// CompactionConfig tests
// ============================================================================

#[test]
fn test_compaction_config_default() {
    let config = CompactionConfig::default();
    assert!((config.threshold - 0.8).abs() < f32::EPSILON);
    assert_eq!(config.preserve_recent, 0);
    assert!(config.custom_prompt.is_none());
}

#[test]
fn test_compaction_config_validate_valid() {
    let config = CompactionConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_compaction_config_validate_bad_threshold() {
    let config = CompactionConfig {
        threshold: 1.5,
        ..Default::default()
    };
    assert!(config.validate().is_err());

    let config = CompactionConfig {
        threshold: -0.1,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_compaction_config_validate_too_many_preserved() {
    let config = CompactionConfig {
        preserve_recent: 1001,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

// ============================================================================
// CompactionMetadata tests
// ============================================================================

#[test]
fn test_compaction_metadata_default() {
    let meta = CompactionMetadata::default();
    assert_eq!(meta.original_message_count, 0);
    assert!((meta.compression_ratio - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_compaction_metadata_new() {
    let meta = CompactionMetadata::new(1000, 200, 10);
    assert_eq!(meta.original_token_count, 1000);
    assert_eq!(meta.compressed_token_count, 200);
    assert_eq!(meta.original_message_count, 10);
    assert!((meta.compression_ratio - 0.2).abs() < 0.01);
}

#[test]
fn test_compaction_metadata_new_zero_tokens() {
    let meta = CompactionMetadata::new(0, 0, 0);
    assert!((meta.compression_ratio - 1.0).abs() < f32::EPSILON);
}

// ============================================================================
// CompactionPrompt tests
// ============================================================================

#[test]
fn test_compaction_prompt_default() {
    let prompt = CompactionPrompt::default();
    assert!(!prompt.system_instructions.is_empty());
    assert!(prompt.user_template.contains("{conversation_history}"));
}

#[test]
fn test_compaction_prompt_render() {
    let prompt = CompactionPrompt::default();
    let rendered = prompt.render_user_prompt("user: hi\nassistant: hello");
    assert!(rendered.contains("user: hi"));
    assert!(rendered.contains("assistant: hello"));
}

#[test]
fn test_compaction_prompt_load_default() {
    let prompt = CompactionPrompt::load_default().unwrap();
    assert!(!prompt.system_instructions.is_empty());
    assert!(prompt.user_template.contains("{conversation_history}"));
}

#[test]
fn test_compaction_prompt_from_resource() {
    let resource = "# System Instructions\nYou are a summarizer.\n\n# User Template\nSummarize: {conversation_history}\n";
    let prompt = CompactionPrompt::from_resource(resource).unwrap();
    assert!(prompt.system_instructions.contains("summarizer"));
    assert!(prompt.user_template.contains("{conversation_history}"));
}

#[test]
fn test_compaction_prompt_from_resource_missing_system() {
    let resource = "# User Template\nSummarize: {conversation_history}\n";
    let result = CompactionPrompt::from_resource(resource);
    assert!(result.is_err());
}

#[test]
fn test_compaction_prompt_from_resource_missing_user() {
    let resource = "# System Instructions\nYou are a summarizer.\n";
    let result = CompactionPrompt::from_resource(resource);
    assert!(result.is_err());
}

#[test]
fn test_compaction_prompt_create_messages() {
    let prompt = CompactionPrompt::default();
    let messages = prompt.create_messages("user: hello\nassistant: hi");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::System);
    assert_eq!(messages[1].role, MessageRole::User);
    assert!(messages[1].content.contains("user: hello"));
}

// ============================================================================
// Error type tests (LlamaError trait)
// ============================================================================

#[test]
fn test_agent_error_categories() {
    use llama_common::error::{ErrorCategory, LlamaError};

    let err = AgentError::Timeout {
        timeout: Duration::from_secs(10),
    };
    assert_eq!(err.category(), ErrorCategory::System);
    assert_eq!(err.error_code(), "AGENT_TIMEOUT");

    let err = AgentError::QueueFull { capacity: 10 };
    assert_eq!(err.category(), ErrorCategory::System);
    assert_eq!(err.error_code(), "AGENT_QUEUE_FULL");
}

#[test]
fn test_queue_error_categories() {
    use llama_common::error::{ErrorCategory, LlamaError};

    let err = QueueError::Full;
    assert_eq!(err.category(), ErrorCategory::System);
    assert_eq!(err.error_code(), "QUEUE_FULL");

    let err = QueueError::WorkerError("test".to_string());
    assert_eq!(err.error_code(), "QUEUE_WORKER");
}

#[test]
fn test_session_error_categories() {
    use llama_common::error::{ErrorCategory, LlamaError};

    let err = SessionError::NotFound("id".to_string());
    assert_eq!(err.category(), ErrorCategory::User);
    assert!(!err.is_retriable());

    let err = SessionError::Timeout;
    assert_eq!(err.category(), ErrorCategory::System);
    assert!(err.is_retriable());

    let err = SessionError::LimitExceeded;
    assert!(!err.is_retriable());

    let err = SessionError::InvalidState("state".to_string());
    assert!(!err.is_retriable());
}

#[test]
fn test_mcp_error_categories() {
    use llama_common::error::{ErrorCategory, LlamaError};

    let err = MCPError::ServerNotFound("srv".to_string());
    assert_eq!(err.category(), ErrorCategory::User);

    let err = MCPError::Connection("conn".to_string());
    assert_eq!(err.category(), ErrorCategory::External);
}

#[test]
fn test_template_error_categories() {
    use llama_common::error::{ErrorCategory, LlamaError};

    let err = TemplateError::RenderingFailed("err".to_string());
    assert_eq!(err.category(), ErrorCategory::User);
    assert_eq!(err.error_code(), "TEMPLATE_RENDERING");
}

#[test]
fn test_error_user_friendly_messages() {
    use llama_common::error::LlamaError;

    let err = QueueError::Full;
    let msg = err.user_friendly_message();
    assert!(msg.contains("Queue is full"));

    let err = SessionError::NotFound("123".to_string());
    let msg = err.user_friendly_message();
    assert!(msg.contains("123"));
}

// ============================================================================
// MCP server config tests
// ============================================================================

#[test]
fn test_mcp_server_config_name() {
    let config = MCPServerConfig::InProcess(ProcessServerConfig {
        name: "my-server".to_string(),
        command: "python".to_string(),
        args: vec![],
        timeout_secs: Some(30),
    });
    assert_eq!(config.name(), "my-server");

    let config = MCPServerConfig::Http(HttpServerConfig {
        name: "http-server".to_string(),
        url: "http://localhost:8080".to_string(),
        timeout_secs: Some(30),
        sse_keep_alive_secs: None,
        stateful_mode: false,
    });
    assert_eq!(config.name(), "http-server");
}

#[test]
fn test_mcp_server_config_validate_empty_name() {
    let config = MCPServerConfig::InProcess(ProcessServerConfig {
        name: "".to_string(),
        command: "python".to_string(),
        args: vec![],
        timeout_secs: Some(30),
    });
    assert!(config.validate().is_err());
}

#[test]
fn test_mcp_server_config_validate_empty_command() {
    let config = MCPServerConfig::InProcess(ProcessServerConfig {
        name: "server".to_string(),
        command: "".to_string(),
        args: vec![],
        timeout_secs: Some(30),
    });
    assert!(config.validate().is_err());
}

#[test]
fn test_http_server_config_validate_empty_url() {
    let config = HttpServerConfig {
        name: "server".to_string(),
        url: "".to_string(),
        timeout_secs: Some(30),
        sse_keep_alive_secs: None,
        stateful_mode: false,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_server_config_validate_invalid_scheme() {
    let config = HttpServerConfig {
        name: "server".to_string(),
        url: "ftp://example.com".to_string(),
        timeout_secs: Some(30),
        sse_keep_alive_secs: None,
        stateful_mode: false,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_server_config_to_streamable() {
    let config = HttpServerConfig {
        name: "server".to_string(),
        url: "http://localhost:8080".to_string(),
        timeout_secs: Some(30),
        sse_keep_alive_secs: Some(60),
        stateful_mode: true,
    };
    let streamable = config.to_streamable_config();
    assert!(streamable.stateful_mode);
    assert_eq!(streamable.sse_keep_alive, Some(Duration::from_secs(60)));
}

#[test]
fn test_http_server_config_from_streamable() {
    let streamable = rmcp::transport::StreamableHttpServerConfig {
        sse_keep_alive: Some(Duration::from_secs(45)),
        stateful_mode: true,
        ..Default::default()
    };

    let config = HttpServerConfig::from_streamable_config(
        "test".to_string(),
        "http://localhost:8080".to_string(),
        Some(30),
        &streamable,
    );
    assert_eq!(config.name, "test");
    assert_eq!(config.sse_keep_alive_secs, Some(45));
    assert!(config.stateful_mode);
}

// ============================================================================
// ParallelConfig tests
// ============================================================================

#[test]
fn test_parallel_config_default() {
    let config = ParallelConfig::default();
    assert_eq!(config.max_parallel_tools, 4);
    assert!(config.conflict_detection);
    assert!(config.resource_analysis);
    assert!(config.never_parallel.is_empty());
    assert!(config.tool_conflicts.is_empty());
}

// ============================================================================
// FinishReason tests
// ============================================================================

#[test]
fn test_finish_reason_eq() {
    let a = FinishReason::Stopped("max_tokens".to_string());
    let b = FinishReason::Stopped("max_tokens".to_string());
    assert_eq!(a, b);

    let c = FinishReason::Stopped("eos".to_string());
    assert_ne!(a, c);
}

#[test]
fn test_finish_reason_serialize() {
    let reason = FinishReason::Stopped("test".to_string());
    let json = serde_json::to_string(&reason).unwrap();
    assert!(json.contains("Stopped"));
    let deserialized: FinishReason = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, reason);
}

// ============================================================================
// StreamChunk tests
// ============================================================================

#[test]
fn test_stream_chunk_basic() {
    let chunk = StreamChunk {
        text: "hello".to_string(),
        is_complete: false,
        token_count: 1,
        finish_reason: None,
    };
    assert_eq!(chunk.text, "hello");
    assert!(!chunk.is_complete);
    assert!(chunk.finish_reason.is_none());
}

#[test]
fn test_stream_chunk_complete() {
    let chunk = StreamChunk {
        text: "done".to_string(),
        is_complete: true,
        token_count: 5,
        finish_reason: Some(FinishReason::Stopped("eos".to_string())),
    };
    assert!(chunk.is_complete);
    assert!(chunk.finish_reason.is_some());
}

// ============================================================================
// ToolDefinition serialization tests
// ============================================================================

#[test]
fn test_tool_definition_serialize() {
    let tool = make_tool_def("test", "A test tool", "server");
    let json = serde_json::to_value(&tool).unwrap();
    assert_eq!(json["name"], "test");
    assert_eq!(json["description"], "A test tool");
    assert_eq!(json["server_name"], "server");
}

#[test]
fn test_tool_call_serialize() {
    let tc = make_tool_call("test", json!({"key": "value"}));
    let json = serde_json::to_value(&tc).unwrap();
    assert_eq!(json["name"], "test");
    assert_eq!(json["arguments"]["key"], "value");
}

#[test]
fn test_tool_result_serialize() {
    let result = ToolResult {
        call_id: ToolCallId::new(),
        result: json!({"status": "ok"}),
        error: None,
    };
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["result"]["status"], "ok");
    assert!(json["error"].is_null());
}

// ============================================================================
// HealthStatus tests
// ============================================================================

#[test]
fn test_health_status_serialize() {
    let status = HealthStatus {
        status: "healthy".to_string(),
        model_loaded: true,
        queue_size: 5,
        active_sessions: 2,
        uptime: Duration::from_secs(3600),
    };
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["status"], "healthy");
    assert_eq!(json["model_loaded"], true);
}

// ============================================================================
// QueueMetrics tests
// ============================================================================

#[test]
fn test_queue_metrics_new() {
    let metrics = llama_agent::queue::QueueMetrics::new();
    assert_eq!(
        metrics
            .total_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_queue_metrics_record_request_submitted() {
    let metrics = llama_agent::queue::QueueMetrics::new();
    metrics.record_request_submitted();
    assert_eq!(
        metrics
            .total_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        metrics
            .current_queue_size
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

#[test]
fn test_queue_metrics_record_request_completed() {
    let metrics = llama_agent::queue::QueueMetrics::new();
    metrics.record_request_submitted();
    metrics.record_request_completed(Duration::from_millis(100), 10);
    assert_eq!(
        metrics
            .completed_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(
        metrics
            .current_queue_size
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[test]
fn test_queue_metrics_record_request_failed() {
    let metrics = llama_agent::queue::QueueMetrics::new();
    metrics.record_request_submitted();
    metrics.record_request_failed();
    assert_eq!(
        metrics
            .failed_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

#[test]
fn test_queue_metrics_record_request_cancelled() {
    let metrics = llama_agent::queue::QueueMetrics::new();
    metrics.record_request_cancelled();
    assert_eq!(
        metrics
            .cancelled_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

#[test]
fn test_queue_metrics_peak_queue_size() {
    let metrics = llama_agent::queue::QueueMetrics::new();
    metrics.record_request_submitted();
    metrics.record_request_submitted();
    metrics.record_request_submitted();
    assert_eq!(
        metrics
            .peak_queue_size
            .load(std::sync::atomic::Ordering::Relaxed),
        3
    );
    metrics.record_request_completed(Duration::from_millis(10), 5);
    // Peak should still be 3 after completion
    assert_eq!(
        metrics
            .peak_queue_size
            .load(std::sync::atomic::Ordering::Relaxed),
        3
    );
}

#[test]
fn test_queue_metrics_throughput() {
    let metrics = llama_agent::queue::QueueMetrics::new();
    metrics.record_request_submitted();
    metrics.record_request_completed(Duration::from_millis(1000), 100);
    let throughput = metrics
        .last_throughput_tokens_per_second
        .load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(throughput, 100); // 100 tokens / 1 second = 100 t/s
}

// ============================================================================
// FileSessionStorage tests
// ============================================================================

#[test]
fn test_file_session_storage_new() {
    let _storage = llama_agent::storage::FileSessionStorage::new(std::path::PathBuf::from(
        "/tmp/test-sessions",
    ));
}

#[test]
fn test_file_session_storage_default() {
    let _storage = llama_agent::storage::FileSessionStorage::default();
}

// ============================================================================
// ResourceType and AccessType tests
// ============================================================================

#[test]
fn test_resource_type_eq() {
    assert_eq!(
        ResourceType::File("a.txt".to_string()),
        ResourceType::File("a.txt".to_string())
    );
    assert_ne!(
        ResourceType::File("a.txt".to_string()),
        ResourceType::File("b.txt".to_string())
    );
    assert_eq!(ResourceType::Memory, ResourceType::Memory);
    assert_eq!(ResourceType::System, ResourceType::System);
}

#[test]
fn test_conflict_type_eq() {
    use llama_agent::types::ConflictType;
    assert_eq!(
        ConflictType::ResourceConflict,
        ConflictType::ResourceConflict
    );
    assert_ne!(
        ConflictType::ResourceConflict,
        ConflictType::OrderDependency
    );
}

// ============================================================================
// Session construction test
// ============================================================================

#[test]
fn test_session_construction() {
    let session = make_session();
    assert!(session.messages.is_empty());
    assert!(session.available_tools.is_empty());
    assert!(session.context_state.is_none());
    assert!(session.current_mode.is_none());
}

#[test]
fn test_session_serialize_deserialize() {
    let session = make_session();
    let json = serde_json::to_string(&session).unwrap();
    let deserialized: Session = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, session.id);
}

// ============================================================================
// ToolCallError tests
// ============================================================================

#[test]
fn test_tool_call_error_display() {
    use llama_agent::acp::translation::ToolCallError;

    let err = ToolCallError::PermissionDenied("test".to_string());
    assert!(err.to_string().contains("Permission denied"));

    let err = ToolCallError::ExecutionFailed("exec fail".to_string());
    assert!(err.to_string().contains("execution failed"));
}

// ============================================================================
// ACP config error tests
// ============================================================================

#[test]
fn test_acp_config_error_display() {
    use llama_agent::acp::error::ConfigError;

    let err = ConfigError::FileReadError("test".to_string());
    assert!(err.to_string().contains("test"));

    let err = ConfigError::ParseError("parse".to_string());
    assert!(err.to_string().contains("parse"));

    let err = ConfigError::FileWriteError("write".to_string());
    assert!(err.to_string().contains("write"));

    let err = ConfigError::SerializationError("serial".to_string());
    assert!(err.to_string().contains("serial"));
}
