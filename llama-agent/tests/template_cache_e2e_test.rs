//! End-to-end tests for template caching functionality
//!
//! These tests verify that template caching works correctly across
//! the full system, from session creation through generation. Unlike the
//! other template cache tests, these focus on complete workflows and
//! integration scenarios.
//!
//! # Test Organization
//!
//! - **Basic Integration Tests**: Run without requiring a real model
//! - **Performance Tests**: Marked with `#[ignore]`, require real model
//! - **Stress Tests**: Marked with `#[ignore]`, require real model
//!
//! # No Mocks Policy
//!
//! Following project standards, these tests use real data structures and
//! APIs without any mocks. Tests that require model operations are ignored
//! by default and can be run explicitly when needed.

use llama_agent::{
    template_cache::TemplateCache,
    types::{Message, MessageRole, Session, SessionId, ToolDefinition},
};
use std::time::SystemTime;
use tempfile::TempDir;

// ============================================================================
// Basic Integration Tests (Always Run)
// ============================================================================

/// Test that template cache is created and initialized correctly.
///
/// Verifies:
/// - Cache directory is created
/// - Initial statistics are zero
/// - Cache is ready for use
#[test]
fn test_template_cache_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");

    let cache = TemplateCache::new(cache_dir.clone()).unwrap();

    // Verify cache directory exists
    assert!(cache_dir.exists());

    // Verify initial statistics
    let stats = cache.stats();
    assert_eq!(stats.entries, 0);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.hit_rate, 0.0);
    assert_eq!(stats.total_tokens, 0);
}

/// Test cache miss on first access.
///
/// Verifies:
/// - First access to a template hash returns None
/// - Miss counter is incremented
/// - Hit rate remains 0.0
#[test]
fn test_first_session_creates_cache_miss() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    let system_prompt = "You are a helpful assistant.";
    let tools_json = r#"[{"name": "test_tool"}]"#;
    let hash = TemplateCache::hash_template(system_prompt, tools_json);

    // First access should be a miss
    let result = cache.get(hash);
    assert!(result.is_none());

    // Verify statistics
    let stats = cache.stats();
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.hit_rate, 0.0);
}

/// Test cache hit on second access with same template.
///
/// Verifies:
/// - After insert, get returns Some(entry)
/// - Hit counter is incremented
/// - Entry contains correct data
#[test]
fn test_second_session_cache_hit() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    let system_prompt = "You are a helpful assistant.";
    let tools_json = r#"[{"name": "test_tool"}]"#;
    let hash = TemplateCache::hash_template(system_prompt, tools_json);

    // Insert cache entry
    let token_count = 150;
    let kv_file = cache
        .insert(
            hash,
            token_count,
            system_prompt.to_string(),
            tools_json.to_string(),
        )
        .unwrap();

    // Verify file path is generated correctly
    assert!(kv_file.to_string_lossy().contains("template_"));
    assert!(kv_file.to_string_lossy().ends_with(".kv"));

    // Second access should be a hit
    let entry = cache.get(hash).unwrap();
    assert_eq!(entry.token_count, token_count);
    assert_eq!(entry.system_prompt, system_prompt);
    assert_eq!(entry.tools_json, tools_json);

    // Verify statistics
    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 0);
    assert_eq!(stats.hit_rate, 1.0);
    assert_eq!(stats.entries, 1);
    assert_eq!(stats.total_tokens, token_count);
}

/// Test that different templates create separate cache entries.
///
/// Verifies:
/// - Different system prompts produce different hashes
/// - Different tool configurations produce different hashes
/// - Each unique template gets its own cache entry
#[test]
fn test_different_templates_separate_cache() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    // Template 1: System prompt A + Tools A
    let system_prompt_a = "You are a helpful assistant.";
    let tools_json_a = r#"[{"name": "tool_a"}]"#;
    let hash_a = TemplateCache::hash_template(system_prompt_a, tools_json_a);

    // Template 2: System prompt B + Tools A
    let system_prompt_b = "You are a coding assistant.";
    let tools_json_b = r#"[{"name": "tool_a"}]"#;
    let hash_b = TemplateCache::hash_template(system_prompt_b, tools_json_b);

    // Template 3: System prompt A + Tools B
    let system_prompt_c = "You are a helpful assistant.";
    let tools_json_c = r#"[{"name": "tool_b"}]"#;
    let hash_c = TemplateCache::hash_template(system_prompt_c, tools_json_c);

    // Verify all hashes are different
    assert_ne!(hash_a, hash_b);
    assert_ne!(hash_a, hash_c);
    assert_ne!(hash_b, hash_c);

    // Insert all three templates
    cache
        .insert(
            hash_a,
            100,
            system_prompt_a.to_string(),
            tools_json_a.to_string(),
        )
        .unwrap();
    cache
        .insert(
            hash_b,
            120,
            system_prompt_b.to_string(),
            tools_json_b.to_string(),
        )
        .unwrap();
    cache
        .insert(
            hash_c,
            110,
            system_prompt_c.to_string(),
            tools_json_c.to_string(),
        )
        .unwrap();

    // Verify all three are in cache
    let stats = cache.stats();
    assert_eq!(stats.entries, 3);
    assert_eq!(stats.total_tokens, 330);

    // Verify each can be retrieved
    assert!(cache.get(hash_a).is_some());
    assert!(cache.get(hash_b).is_some());
    assert!(cache.get(hash_c).is_some());
}

/// Test cache statistics tracking across multiple operations.
///
/// Verifies:
/// - Hit and miss counters work correctly
/// - Hit rate is calculated correctly
/// - Entry count and total tokens are accurate
#[test]
fn test_cache_statistics() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    let template1 = ("system1", "tools1");
    let template2 = ("system2", "tools2");
    let hash1 = TemplateCache::hash_template(template1.0, template1.1);
    let hash2 = TemplateCache::hash_template(template2.0, template2.1);

    // Initial state
    let stats = cache.stats();
    assert_eq!(stats.entries, 0);
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.misses, 0);

    // Miss on template1
    assert!(cache.get(hash1).is_none());
    let stats = cache.stats();
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hit_rate, 0.0);

    // Insert template1
    cache
        .insert(hash1, 100, template1.0.to_string(), template1.1.to_string())
        .unwrap();

    // Hit on template1
    assert!(cache.get(hash1).is_some());
    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hit_rate, 0.5); // 1 hit, 1 miss

    // Miss on template2
    assert!(cache.get(hash2).is_none());
    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 2);
    assert_eq!(stats.hit_rate, 1.0 / 3.0); // 1 hit, 2 misses

    // Insert template2
    cache
        .insert(hash2, 200, template2.0.to_string(), template2.1.to_string())
        .unwrap();

    // Hit on template2
    assert!(cache.get(hash2).is_some());
    let stats = cache.stats();
    assert_eq!(stats.hits, 2);
    assert_eq!(stats.misses, 2);
    assert_eq!(stats.hit_rate, 0.5); // 2 hits, 2 misses
    assert_eq!(stats.entries, 2);
    assert_eq!(stats.total_tokens, 300);
}

/// Test that template changes result in new cache entry.
///
/// Verifies:
/// - Modifying system prompt creates new hash
/// - Modifying tools creates new hash
/// - Old cache entry remains valid
#[test]
fn test_template_change_new_cache() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    // Original template
    let system_v1 = "You are a helpful assistant.";
    let tools_v1 = r#"[{"name": "tool1"}]"#;
    let hash_v1 = TemplateCache::hash_template(system_v1, tools_v1);

    cache
        .insert(hash_v1, 100, system_v1.to_string(), tools_v1.to_string())
        .unwrap();

    // Modified system prompt
    let system_v2 = "You are a very helpful assistant."; // Changed
    let tools_v2 = r#"[{"name": "tool1"}]"#;
    let hash_v2 = TemplateCache::hash_template(system_v2, tools_v2);

    // Should be different hash
    assert_ne!(hash_v1, hash_v2);

    // Original entry still exists
    assert!(cache.get(hash_v1).is_some());

    // New template needs new cache entry
    assert!(cache.get(hash_v2).is_none());

    cache
        .insert(hash_v2, 102, system_v2.to_string(), tools_v2.to_string())
        .unwrap();

    // Both entries now exist
    let stats = cache.stats();
    assert_eq!(stats.entries, 2);
    assert!(cache.get(hash_v1).is_some());
    assert!(cache.get(hash_v2).is_some());
}

/// Test cache file path generation and persistence checking.
///
/// Verifies:
/// - Cache files have correct naming format
/// - File existence can be checked
/// - Path is within cache directory
#[test]
fn test_cache_file_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir.clone()).unwrap();

    let system_prompt = "You are a helpful assistant.";
    let tools_json = r#"[{"name": "test_tool"}]"#;
    let hash = TemplateCache::hash_template(system_prompt, tools_json);

    // Insert and get file path
    let kv_file = cache
        .insert(hash, 100, system_prompt.to_string(), tools_json.to_string())
        .unwrap();

    // Verify path format
    let filename = kv_file.file_name().unwrap().to_string_lossy();
    assert!(filename.starts_with("template_"));
    assert!(filename.ends_with(".kv"));

    // Verify path is in cache directory
    assert!(kv_file.starts_with(&cache_dir));

    // File doesn't exist yet (not created in this test)
    assert!(!kv_file.exists());

    // But cache metadata exists
    assert!(cache.get(hash).is_some());

    // Create a dummy file to test persistence
    std::fs::write(&kv_file, b"dummy kv cache data").unwrap();
    assert!(kv_file.exists());

    // Verify has_kv_cache returns true now
    assert!(cache.has_kv_cache(hash));
}

/// Test Session structure has template_token_count field.
///
/// Verifies:
/// - Session has template_token_count field
/// - Field can be set to None and Some(count)
/// - Field is properly initialized
#[test]
fn test_session_template_token_count_field() {
    let session = create_test_session();

    // Initially None
    assert!(session.template_token_count.is_none());

    // Can be set
    let mut session_with_cache = session.clone();
    session_with_cache.template_token_count = Some(150);
    assert_eq!(session_with_cache.template_token_count, Some(150));
}

/// Test template hash computation with Session data.
///
/// Verifies:
/// - Can extract system prompt from Session
/// - Can serialize tools to JSON
/// - Hash computation works with Session data
#[test]
fn test_template_hash_with_session_data() {
    let session1 = create_test_session();
    let session2 = create_test_session();

    // Extract template components
    let system_prompt1 = extract_system_prompt(&session1);
    let tools_json1 = serialize_tools(&session1.available_tools);

    let system_prompt2 = extract_system_prompt(&session2);
    let tools_json2 = serialize_tools(&session2.available_tools);

    // Same sessions should produce same hash
    let hash1 = TemplateCache::hash_template(&system_prompt1, &tools_json1);
    let hash2 = TemplateCache::hash_template(&system_prompt2, &tools_json2);
    assert_eq!(hash1, hash2);

    // Different session should produce different hash
    let session3 = create_session_with_different_system_prompt("Different prompt");
    let system_prompt3 = extract_system_prompt(&session3);
    let tools_json3 = serialize_tools(&session3.available_tools);
    let hash3 = TemplateCache::hash_template(&system_prompt3, &tools_json3);
    assert_ne!(hash1, hash3);
}

/// Test cache entry verification functionality.
///
/// Verifies:
/// - Cache can verify entry content matches expected template
/// - Verification fails for mismatched content
/// - Verification fails for non-existent entries
#[test]
fn test_cache_entry_verification() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    let system_prompt = "You are a helpful assistant.";
    let tools_json = r#"[{"name": "test_tool"}]"#;
    let hash = TemplateCache::hash_template(system_prompt, tools_json);

    // Insert entry
    cache
        .insert(hash, 100, system_prompt.to_string(), tools_json.to_string())
        .unwrap();

    // Verify with correct content
    assert!(cache.verify(hash, system_prompt, tools_json));

    // Verify with incorrect system prompt
    assert!(!cache.verify(hash, "Different prompt", tools_json));

    // Verify with incorrect tools
    assert!(!cache.verify(hash, system_prompt, "[]"));

    // Verify non-existent hash
    let other_hash = TemplateCache::hash_template("other", "other");
    assert!(!cache.verify(other_hash, "other", "other"));
}

/// Test cache entry deletion functionality.
///
/// Verifies:
/// - Can delete cache entries
/// - Deletion removes both metadata and file
/// - Deletion of non-existent entry returns false
#[test]
fn test_cache_entry_deletion() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    let system_prompt = "You are a helpful assistant.";
    let tools_json = r#"[{"name": "test_tool"}]"#;
    let hash = TemplateCache::hash_template(system_prompt, tools_json);

    // Insert entry
    let kv_file = cache
        .insert(hash, 100, system_prompt.to_string(), tools_json.to_string())
        .unwrap();

    // Create dummy file
    std::fs::write(&kv_file, b"dummy").unwrap();

    // Verify entry exists
    assert!(cache.get(hash).is_some());
    assert!(kv_file.exists());

    // Delete entry
    let deleted = cache.delete(hash).unwrap();
    assert!(deleted);

    // Verify entry and file are gone
    assert!(cache.get(hash).is_none());
    assert!(!kv_file.exists());

    // Delete again should return false
    let deleted_again = cache.delete(hash).unwrap();
    assert!(!deleted_again);
}

// ============================================================================
// Performance Tests (Require Real Model - Ignored by Default)
// ============================================================================

/// Performance test: Verify cache provides speedup.
///
/// This test requires a real model to run and measures actual performance.
/// Expected: First session ~450-500ms, second session ~10-20ms
///
/// To run: cargo nextest run --ignored template_cache_e2e
#[tokio::test]
#[ignore = "Requires real model file to measure actual performance"]
async fn test_cache_performance_benefit() {
    // This test would:
    // 1. Load a real model
    // 2. Create first session with template (cache miss)
    // 3. Measure time to process template
    // 4. Create second session with same template (cache hit)
    // 5. Measure time to load from cache
    // 6. Verify second session is significantly faster (>90% speedup)
    //
    // Implementation requires:
    // - Real model file path
    // - ModelManager integration
    // - Context creation
    // - Actual KV cache save/load operations
}

/// Stress test: Multiple concurrent sessions with shared template.
///
/// This test requires a real model and measures concurrent performance.
/// Expected: 10 sessions complete much faster with caching than without.
///
/// To run: cargo nextest run --ignored template_cache_e2e
#[tokio::test]
#[ignore = "Requires real model file and tests concurrent behavior"]
async fn test_concurrent_sessions_cache_sharing() {
    // This test would:
    // 1. Load a real model
    // 2. Create 10 sessions concurrently with same template
    // 3. Verify only first session processes template (cache miss)
    // 4. Verify other 9 sessions load from cache (cache hits)
    // 5. Measure total time and verify significant speedup
    //
    // Expected results:
    // - Cache stats show 1 miss, 9 hits
    // - Total time for 10 sessions < 1000ms (vs ~4500ms without cache)
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test session with standard configuration.
fn create_test_session() -> Session {
    Session {
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
    }
}

/// Create a session with a different system prompt.
fn create_session_with_different_system_prompt(content: &str) -> Session {
    let mut session = create_test_session();
    session.messages[0].content = content.to_string();
    session
}

/// Extract system prompt from session messages.
fn extract_system_prompt(session: &Session) -> String {
    session
        .messages
        .iter()
        .filter(|m| matches!(m.role, MessageRole::System))
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Serialize tools to JSON string for hashing.
fn serialize_tools(tools: &[ToolDefinition]) -> String {
    if tools.is_empty() {
        String::new()
    } else {
        serde_json::to_string(tools).unwrap_or_default()
    }
}
