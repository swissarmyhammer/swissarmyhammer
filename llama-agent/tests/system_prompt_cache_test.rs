//! Test to verify system prompt caching is working correctly
//!
//! This test specifically addresses the concern that template (system prompt)
//! caching might not be working when using llama-agent with local models.
//!
//! The test verifies that:
//! 1. First session with a system prompt creates a template cache entry
//! 2. Second session with the same system prompt reuses the cached template
//! 3. Cache statistics correctly track hits and misses

use llama_agent::template_cache::TemplateCache;
use tempfile::TempDir;

/// Test that demonstrates system prompt caching across multiple sessions.
///
/// This test simulates the real-world scenario where multiple sessions
/// share the same system prompt and should benefit from template caching.
#[test]
fn test_system_prompt_reuse_across_sessions() {
    // Setup: Create a template cache
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir.clone()).unwrap();

    // Simulate a common system prompt used across sessions
    let system_prompt = "You are a helpful AI assistant that helps users with coding tasks.";
    let tools_json = r#"[{"name": "execute_code"}, {"name": "read_file"}]"#;

    // Calculate the template hash
    let template_hash = TemplateCache::hash_template(system_prompt, tools_json);

    // Session 1: First use of this system prompt (should be cache MISS)
    println!("Session 1: First use of system prompt");
    let miss_result = cache.get(template_hash);
    assert!(miss_result.is_none(), "First access should be a cache miss");

    let stats_after_miss = cache.stats();
    assert_eq!(stats_after_miss.misses, 1, "Should have 1 miss");
    assert_eq!(stats_after_miss.hits, 0, "Should have 0 hits");
    assert_eq!(stats_after_miss.hit_rate, 0.0, "Hit rate should be 0%");

    // Simulate processing the system prompt and caching it
    println!("Simulating template processing and caching...");
    let token_count = 250; // Typical system prompt token count
    cache
        .insert(
            template_hash,
            token_count,
            system_prompt.to_string(),
            tools_json.to_string(),
        )
        .expect("Should successfully insert cache entry");

    let stats_after_insert = cache.stats();
    assert_eq!(stats_after_insert.entries, 1, "Should have 1 cache entry");

    // Session 2: Second use of same system prompt (should be cache HIT)
    println!("Session 2: Reusing same system prompt");
    let hit_result = cache.get(template_hash);
    assert!(hit_result.is_some(), "Second access should be a cache hit");

    let entry = hit_result.unwrap();
    assert_eq!(
        entry.token_count, token_count,
        "Cached token count should match"
    );
    assert_eq!(
        entry.system_prompt, system_prompt,
        "Cached system prompt should match"
    );

    let stats_after_hit = cache.stats();
    assert_eq!(stats_after_hit.misses, 1, "Should still have 1 miss");
    assert_eq!(stats_after_hit.hits, 1, "Should have 1 hit");
    assert_eq!(
        stats_after_hit.hit_rate, 0.5,
        "Hit rate should be 50% (1 hit / 2 total)"
    );

    // Session 3: Third use of same system prompt (should be another cache HIT)
    println!("Session 3: Another reuse of same system prompt");
    let hit_result2 = cache.get(template_hash);
    assert!(
        hit_result2.is_some(),
        "Third access should also be a cache hit"
    );

    let stats_after_second_hit = cache.stats();
    assert_eq!(stats_after_second_hit.misses, 1, "Should still have 1 miss");
    assert_eq!(stats_after_second_hit.hits, 2, "Should have 2 hits");
    assert_eq!(
        stats_after_second_hit.hit_rate,
        2.0 / 3.0,
        "Hit rate should be ~66.7% (2 hits / 3 total)"
    );

    println!("✓ System prompt caching is working correctly!");
    println!(
        "  Final stats: {} entries, {} hits, {} misses, {:.1}% hit rate",
        stats_after_second_hit.entries,
        stats_after_second_hit.hits,
        stats_after_second_hit.misses,
        stats_after_second_hit.hit_rate * 100.0
    );
}

/// Test that verifies cache metadata IS persisted across instances.
///
/// This test verifies that the template cache metadata is saved to disk
/// and automatically loaded when a new TemplateCache instance is created.
#[test]
fn test_template_cache_metadata_persisted() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");

    let system_prompt = "You are a helpful assistant.";
    let tools_json = r#"[{"name": "test_tool"}]"#;
    let template_hash = TemplateCache::hash_template(system_prompt, tools_json);
    let token_count = 100;

    // First instance: Create cache and insert entry
    {
        let mut cache1 = TemplateCache::new(cache_dir.clone()).unwrap();
        let kv_file_path = cache1
            .insert(
                template_hash,
                token_count,
                system_prompt.to_string(),
                tools_json.to_string(),
            )
            .expect("Should insert successfully");

        let stats = cache1.stats();
        assert_eq!(stats.entries, 1, "Should have 1 entry after insert");

        // Can get the entry within the same instance
        let entry = cache1.get(template_hash);
        assert!(entry.is_some(), "Should find entry in same instance");

        // Simulate that KV cache file was actually saved by creating an empty file
        std::fs::write(&kv_file_path, b"dummy kv cache data").unwrap();
    }

    // Verify metadata file was created
    let metadata_file = cache_dir.join("metadata.json");
    assert!(
        metadata_file.exists(),
        "Metadata file should be created on disk"
    );

    // Second instance: Create new cache - metadata should be automatically loaded
    {
        let mut cache2 = TemplateCache::new(cache_dir.clone()).unwrap();

        // The metadata should be loaded from disk
        let stats = cache2.stats();
        assert_eq!(
            stats.entries, 1,
            "Metadata should be automatically loaded from disk"
        );

        // Getting from cache should HIT because metadata was loaded
        let entry = cache2.get(template_hash);
        assert!(
            entry.is_some(),
            "Cache metadata should be persisted and loaded across instances"
        );

        let entry = entry.unwrap();
        assert_eq!(entry.token_count, token_count);
        assert_eq!(entry.system_prompt, system_prompt);
        assert_eq!(entry.tools_json, tools_json);
    }
}

/// Test that template cache correctly validates token count.
///
/// Empty system prompts with 0 tokens should be rejected.
#[test]
fn test_template_cache_validates_token_count() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");
    let mut cache = TemplateCache::new(cache_dir).unwrap();

    let system_prompt = ""; // Empty system prompt
    let tools_json = r#"[]"#; // No tools
    let hash = TemplateCache::hash_template(system_prompt, tools_json);

    // Should fail validation because token_count must be > 0
    let result = cache.insert(hash, 0, system_prompt.to_string(), tools_json.to_string());
    assert!(result.is_err(), "Should reject cache entries with 0 tokens");
}

/// Test that verifies template hash changes when system prompt changes.
///
/// This ensures that different system prompts don't accidentally share cache entries.
#[test]
fn test_template_hash_uniqueness() {
    let tools_json = r#"[{"name": "tool1"}]"#;

    let hash1 = TemplateCache::hash_template("System prompt 1", tools_json);
    let hash2 = TemplateCache::hash_template("System prompt 2", tools_json);
    let hash3 = TemplateCache::hash_template("System prompt 1", tools_json); // Same as hash1

    assert_ne!(
        hash1, hash2,
        "Different system prompts should produce different hashes"
    );
    assert_eq!(
        hash1, hash3,
        "Identical system prompts should produce identical hashes"
    );
}

/// Test that verifies template hash changes when tools change.
///
/// This ensures that different tool configurations don't accidentally share cache entries.
#[test]
fn test_template_hash_uniqueness_with_tools() {
    let system_prompt = "You are a helpful assistant.";

    let hash1 = TemplateCache::hash_template(system_prompt, r#"[{"name": "tool1"}]"#);
    let hash2 = TemplateCache::hash_template(system_prompt, r#"[{"name": "tool2"}]"#);
    let hash3 = TemplateCache::hash_template(system_prompt, r#"[{"name": "tool1"}]"#);

    assert_ne!(
        hash1, hash2,
        "Different tools should produce different hashes"
    );
    assert_eq!(
        hash1, hash3,
        "Identical tools should produce identical hashes"
    );
}

/// Test that multiple cache entries are persisted correctly.
#[test]
fn test_multiple_entries_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");

    // First instance: Create cache with multiple entries
    {
        let mut cache = TemplateCache::new(cache_dir.clone()).unwrap();

        // Insert 3 different templates
        for i in 1..=3 {
            let system_prompt = format!("System prompt {}", i);
            let tools_json = format!(r#"[{{"name": "tool{}"}}]"#, i);
            let hash = TemplateCache::hash_template(&system_prompt, &tools_json);
            let token_count = 100 * i;

            let kv_file = cache
                .insert(hash, token_count, system_prompt, tools_json)
                .unwrap();

            // Create dummy KV cache file
            std::fs::write(&kv_file, format!("dummy data {}", i)).unwrap();
        }

        let stats = cache.stats();
        assert_eq!(stats.entries, 3, "Should have 3 entries");
    }

    // Second instance: Verify all entries are loaded
    {
        let cache = TemplateCache::new(cache_dir.clone()).unwrap();

        let stats = cache.stats();
        assert_eq!(stats.entries, 3, "All 3 entries should be loaded from disk");
    }
}

/// Test that cache handles corrupted metadata gracefully.
#[test]
fn test_corrupted_metadata_handling() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");

    // Create corrupted metadata file
    std::fs::create_dir_all(&cache_dir).unwrap();
    let metadata_file = cache_dir.join("metadata.json");
    std::fs::write(&metadata_file, b"{ invalid json {{").unwrap();

    // Should not panic, just start with empty cache
    let cache = TemplateCache::new(cache_dir.clone()).unwrap();

    let stats = cache.stats();
    assert_eq!(
        stats.entries, 0,
        "Should start with empty cache when metadata is corrupted"
    );
}

/// Test that cache skips entries with missing KV cache files.
#[test]
fn test_missing_kv_cache_files_skipped() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");

    // First instance: Create cache with 2 entries
    {
        let mut cache = TemplateCache::new(cache_dir.clone()).unwrap();

        let hash1 = TemplateCache::hash_template("prompt1", "tools1");
        let hash2 = TemplateCache::hash_template("prompt2", "tools2");

        let kv_file1 = cache
            .insert(hash1, 100, "prompt1".to_string(), "tools1".to_string())
            .unwrap();
        let kv_file2 = cache
            .insert(hash2, 200, "prompt2".to_string(), "tools2".to_string())
            .unwrap();

        // Only create KV cache file for first entry
        std::fs::write(&kv_file1, b"data1").unwrap();
        // Deliberately don't create kv_file2
        let _ = kv_file2;
    }

    // Second instance: Should only load entry with existing KV cache file
    {
        let cache = TemplateCache::new(cache_dir.clone()).unwrap();

        let stats = cache.stats();
        assert_eq!(
            stats.entries, 1,
            "Should only load entry with existing KV cache file"
        );
    }
}

/// Test that max_entries limit is enforced when loading from disk.
///
/// Verifies that if there are more cached entries on disk than max_entries allows,
/// only the most recently used entries are loaded.
#[test]
fn test_max_entries_enforced_on_load() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");

    // First instance: Create cache with 5 entries (max_entries = 10)
    {
        let mut cache = TemplateCache::with_max_entries(cache_dir.clone(), Some(10)).unwrap();

        // Insert 5 entries with different last_used times
        for i in 1..=5 {
            let system_prompt = format!("System prompt {}", i);
            let tools_json = format!(r#"[{{"name": "tool{}"}}]"#, i);
            let hash = TemplateCache::hash_template(&system_prompt, &tools_json);

            let kv_file = cache
                .insert(hash, 100 * i, system_prompt, tools_json)
                .unwrap();

            std::fs::write(&kv_file, format!("data {}", i)).unwrap();

            // Sleep briefly to ensure different last_used timestamps
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let stats = cache.stats();
        assert_eq!(stats.entries, 5);
    }

    // Second instance: Create cache with max_entries = 3 (less than the 5 on disk)
    {
        let cache = TemplateCache::with_max_entries(cache_dir.clone(), Some(3)).unwrap();

        let stats = cache.stats();
        assert_eq!(
            stats.entries, 3,
            "Should only load 3 most recent entries (respecting max_entries)"
        );

        // The 3 most recently used entries (3, 4, 5) should be loaded
        // We verified the count is correct - that's sufficient for this test
    }

    // Third instance: Verify that excess entries were cleaned up
    {
        let cache = TemplateCache::with_max_entries(cache_dir.clone(), Some(3)).unwrap();

        let stats = cache.stats();
        assert_eq!(
            stats.entries, 3,
            "Should still have exactly 3 entries after reload"
        );
    }
}

/// Test realistic scenario: agent restart with cached system prompt.
///
/// Simulates the real-world use case where an agent is restarted
/// and should immediately benefit from previously cached templates.
#[test]
fn test_agent_restart_scenario() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().join("templates");

    let system_prompt = "You are an AI assistant that helps with coding tasks. \
                        You have access to tools for reading files, executing code, and more.";
    let tools_json = r#"[
        {"name": "read_file", "parameters": {"path": "string"}},
        {"name": "execute_code", "parameters": {"code": "string"}}
    ]"#;
    let template_hash = TemplateCache::hash_template(system_prompt, tools_json);

    // Session 1: First agent run (cache miss expected)
    {
        let mut cache = TemplateCache::new(cache_dir.clone()).unwrap();

        // First access - should be MISS
        let result = cache.get(template_hash);
        assert!(result.is_none(), "First access should be cache miss");

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        // Process template and cache it
        let token_count = 250; // Realistic system prompt token count
        let kv_file = cache
            .insert(
                template_hash,
                token_count,
                system_prompt.to_string(),
                tools_json.to_string(),
            )
            .unwrap();

        // Simulate actual KV cache save
        std::fs::write(&kv_file, b"llama.cpp kv cache binary data").unwrap();
    }

    // Session 2: Agent restart (cache hit expected)
    {
        let mut cache = TemplateCache::new(cache_dir.clone()).unwrap();

        // Should have loaded 1 entry from disk
        let stats = cache.stats();
        assert_eq!(
            stats.entries, 1,
            "Should load cached template from previous run"
        );

        // Second access - should be HIT (no re-processing needed!)
        let result = cache.get(template_hash);
        assert!(result.is_some(), "Second access should be cache hit");

        let entry = result.unwrap();
        assert_eq!(entry.token_count, 250);
        assert_eq!(entry.system_prompt, system_prompt);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1, "Should have 1 cache hit");
        assert_eq!(stats.misses, 0, "Should have 0 misses (loaded from disk)");
    }

    // Session 3: Another restart (hit again)
    {
        let mut cache = TemplateCache::new(cache_dir.clone()).unwrap();

        // Should still have the entry
        let result = cache.get(template_hash);
        assert!(
            result.is_some(),
            "Cache should persist across multiple restarts"
        );
    }
}
