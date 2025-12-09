//! Performance and stress tests for session compaction functionality
//! Tests scalability, memory usage, and performance characteristics of compaction operations

use crate::session::SessionManager;
use crate::tests::test_utils::*;
use crate::types::SessionConfig;
use crate::types::*;
use std::time::{Duration, Instant, SystemTime};

#[cfg(test)]
mod performance {
    use super::*;

    #[tokio::test]
    async fn test_large_session_token_counting_performance() {
        let counter = SimpleTokenCounter;
        let start = Instant::now();

        // Create session with 100 messages, 100 words each
        let large_session = create_large_content_session(100, 100);
        let usage = counter.count_session_tokens(&large_session);

        let duration = start.elapsed();

        assert!(
            usage.total > 5000,
            "Large session should have substantial token count"
        );
        assert_eq!(usage.by_message.len(), 100);
        assert!(
            duration < Duration::from_millis(100),
            "Token counting should be fast: {:?}",
            duration
        );

        println!("Token counting for 100 messages took: {:?}", duration);
        println!("Total tokens: {}", usage.total);
    }

    #[tokio::test]
    async fn test_very_large_session_token_counting() {
        let counter = SimpleTokenCounter;
        let start = Instant::now();

        // Create session with 500 messages, 50 words each
        let very_large_session = create_large_content_session(500, 50);
        let usage = counter.count_session_tokens(&very_large_session);

        let duration = start.elapsed();

        assert!(
            usage.total > 10000,
            "Very large session should have very substantial token count"
        );
        assert_eq!(usage.by_message.len(), 500);
        assert!(
            duration < Duration::from_millis(500),
            "Token counting should scale well: {:?}",
            duration
        );

        println!("Token counting for 500 messages took: {:?}", duration);
        println!("Total tokens: {}", usage.total);
    }

    #[tokio::test]
    async fn test_compaction_validation_performance() {
        let start = Instant::now();

        // Test token usage calculation on various session sizes since validation is private
        let counter = SimpleTokenCounter;
        for size in [10, 50, 100, 200] {
            let session = create_test_session_with_messages(size);
            let _usage = counter.count_session_tokens(&session);
        }

        let duration = start.elapsed();
        assert!(
            duration < Duration::from_millis(50),
            "Token calculation should be very fast: {:?}",
            duration
        );

        println!(
            "Token calculations for multiple sessions took: {:?}",
            duration
        );
    }

    #[tokio::test]
    async fn test_conversation_history_formatting_performance() {
        let start = Instant::now();

        let large_session = create_large_content_session(300, 40);
        let history = large_session.format_conversation_history();

        let duration = start.elapsed();

        assert!(!history.is_empty());
        assert!(
            history.len() > 10000,
            "Large session should produce substantial history"
        );
        assert!(
            duration < Duration::from_millis(200),
            "History formatting should be reasonably fast: {:?}",
            duration
        );

        println!("History formatting for 300 messages took: {:?}", duration);
        println!("History length: {} characters", history.len());
    }

    #[tokio::test]
    async fn test_session_backup_and_restore_performance() {
        let start = Instant::now();

        let mut large_session = create_large_content_session(150, 25);

        // Add compaction history
        for i in 0..10 {
            large_session.compaction_history.push(CompactionMetadata {
                compacted_at: SystemTime::now(),
                original_message_count: 20 + i,
                original_token_count: 2000 + (i * 100),
                compressed_token_count: 400 + (i * 20),
                compression_ratio: 0.2,
                ..Default::default()
            });
        }

        // Test backup
        let backup_start = Instant::now();
        let backup = large_session.create_backup();
        let backup_duration = backup_start.elapsed();

        // Modify session
        large_session.messages.clear();
        large_session.compaction_history.clear();

        // Test restore
        let restore_start = Instant::now();
        large_session.restore_from_backup(backup);
        let restore_duration = restore_start.elapsed();

        let total_duration = start.elapsed();

        assert_eq!(large_session.messages.len(), 150);
        assert_eq!(large_session.compaction_history.len(), 10);

        assert!(
            backup_duration < Duration::from_millis(50),
            "Backup should be fast: {:?}",
            backup_duration
        );
        assert!(
            restore_duration < Duration::from_millis(50),
            "Restore should be fast: {:?}",
            restore_duration
        );
        assert!(
            total_duration < Duration::from_millis(100),
            "Total backup/restore should be fast: {:?}",
            total_duration
        );

        println!(
            "Backup took: {:?}, Restore took: {:?}",
            backup_duration, restore_duration
        );
    }

    #[tokio::test]
    async fn test_concurrent_session_operations_performance() {
        let manager = std::sync::Arc::new({
            let config = SessionConfig::default();
            SessionManager::new(config)
        });

        let start = Instant::now();
        let mut handles = Vec::new();

        // Create 50 sessions concurrently
        for task_id in 0..50 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                let session = manager_clone.create_session().await.unwrap();

                // Add messages to each session
                for msg_id in 0..20 {
                    let message = Message {
                        role: if msg_id % 2 == 0 {
                            MessageRole::User
                        } else {
                            MessageRole::Assistant
                        },
                        content: format!(
                            "Performance test message {} from task {}",
                            msg_id, task_id
                        ),
                        tool_call_id: None,
                        tool_name: None,
                        timestamp: SystemTime::now(),
                    };
                    manager_clone
                        .add_message(&session.id, message)
                        .await
                        .unwrap();
                }

                session.id
            });
            handles.push(handle);
        }

        let session_ids: Vec<SessionId> = futures::future::try_join_all(handles).await.unwrap();

        let duration = start.elapsed();

        assert_eq!(session_ids.len(), 50);
        assert!(
            duration < Duration::from_secs(5),
            "Concurrent operations should complete within 5 seconds: {:?}",
            duration
        );

        // Verify all sessions exist
        for session_id in &session_ids {
            let session = manager.get_session(session_id).await.unwrap().unwrap();
            assert_eq!(session.messages.len(), 20);
        }

        println!(
            "Created 50 sessions with 20 messages each in: {:?}",
            duration
        );
    }

    #[tokio::test]
    async fn test_compaction_candidate_identification_performance() {
        let manager = {
            let config = SessionConfig::default();
            SessionManager::new(config)
        };

        // Create many sessions with varying sizes
        let mut session_ids = Vec::new();
        for i in 0..100 {
            let session = manager.create_session().await.unwrap();

            // Create sessions with different message counts to test filtering
            let message_count = (i % 20) + 5; // 5-24 messages
            for j in 0..message_count {
                let message = Message {
                    role: if j % 2 == 0 { MessageRole::User } else { MessageRole::Assistant },
                    content: format!("Long message content that contributes to token usage and will make this session exceed typical compaction thresholds: {}", j),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                };
                manager.add_message(&session.id, message).await.unwrap();
            }

            session_ids.push(session.id);
        }

        let start = Instant::now();

        // Test candidate identification performance
        let config = create_low_threshold_compaction_config();
        let candidates = manager.get_compaction_candidates(&config).await.unwrap();

        let duration = start.elapsed();

        println!(
            "Performance test found {} candidates from 100 sessions",
            candidates.len()
        );
        assert!(!candidates.is_empty(), "Should identify some candidates");
        assert!(
            duration < Duration::from_millis(1000),
            "Candidate identification should be fast: {:?}",
            duration
        );

        println!(
            "Identified {} candidates from 100 sessions in: {:?}",
            candidates.len(),
            duration
        );
    }

    #[tokio::test]
    async fn test_compaction_statistics_performance() {
        let manager = {
            let config = SessionConfig::default();
            SessionManager::new(config)
        };

        // Create sessions with test compaction history
        for _i in 0..50 {
            let session = manager.create_session().await.unwrap();

            // Add some messages
            for j in 0..10 {
                let message = Message {
                    role: MessageRole::User,
                    content: format!("Message {} for stats testing", j),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                };
                manager.add_message(&session.id, message).await.unwrap();
            }
        }

        let start = Instant::now();

        // Test statistics calculation performance
        let stats = manager.get_compaction_stats().await.unwrap();

        let duration = start.elapsed();

        assert_eq!(stats.total_sessions, 50);
        assert!(
            duration < Duration::from_millis(200),
            "Statistics calculation should be fast: {:?}",
            duration
        );

        println!("Calculated stats for 50 sessions in: {:?}", duration);
    }

    #[tokio::test]
    async fn test_memory_usage_with_large_sessions() {
        // This test monitors that memory usage doesn't grow excessively
        let counter = SimpleTokenCounter;

        // Create progressively larger sessions and monitor performance
        for size in [100, 500, 1000] {
            let start = Instant::now();

            let session = create_large_content_session(size, 30);
            let _usage = counter.count_session_tokens(&session);
            let _history = session.format_conversation_history();
            let _backup = session.create_backup();

            let duration = start.elapsed();

            // Performance should scale reasonably (not exponentially)
            let expected_max = Duration::from_millis((size as u64) * 2); // 2ms per message max
            assert!(
                duration < expected_max,
                "Session size {} took {:?}, expected < {:?}",
                size,
                duration,
                expected_max
            );

            println!("Session size {} operations took: {:?}", size, duration);
        }
    }

    #[tokio::test]
    async fn test_prompt_rendering_performance() {
        let prompt = CompactionPrompt {
            system_instructions: "Test system instructions".to_string(),
            user_prompt_template: "Please summarize: {conversation_history}".to_string(),
            user_template: "Please summarize: {conversation_history}".to_string(),
        };

        // Test with various history sizes
        for word_count in [100, 1000, 5000, 10000] {
            let large_history = "word ".repeat(word_count);

            let start = Instant::now();
            let _rendered = prompt.render_user_prompt(&large_history);
            let duration = start.elapsed();

            assert!(
                duration < Duration::from_millis(50),
                "Rendering {} words took {:?}, expected < 50ms",
                word_count,
                duration
            );

            println!("Rendering {} words took: {:?}", word_count, duration);
        }
    }

    #[tokio::test]
    async fn test_prompt_estimated_tokens_performance() {
        let prompt = CompactionPrompt {
            system_instructions: "Test instructions".to_string(),
            user_prompt_template: "Template: {conversation_history}".to_string(),
            user_template: "Template: {conversation_history}".to_string(),
        };

        let start = Instant::now();

        // Test token estimation with various sizes
        for size in [100, 1000, 5000] {
            let _history = "token ".repeat(size);
            let _estimated = prompt.estimated_prompt_tokens(size);
        }

        let duration = start.elapsed();

        assert!(
            duration < Duration::from_millis(100),
            "Token estimation should be fast: {:?}",
            duration
        );

        println!("Token estimation for multiple sizes took: {:?}", duration);
    }

    #[tokio::test]
    async fn test_session_id_operations_performance() {
        let start = Instant::now();

        // Create many session IDs and test operations
        let mut session_ids = Vec::new();
        for _ in 0..10000 {
            let id = SessionId::new();
            session_ids.push(id);
        }

        // Test string conversion performance
        let conversion_start = Instant::now();
        for id in &session_ids {
            let _string_repr = id.to_string();
        }
        let conversion_duration = conversion_start.elapsed();

        let total_duration = start.elapsed();

        assert!(
            conversion_duration < Duration::from_millis(100),
            "String conversion should be fast: {:?}",
            conversion_duration
        );
        assert!(
            total_duration < Duration::from_millis(500),
            "Total ID operations should be fast: {:?}",
            total_duration
        );

        println!(
            "Created and converted 10000 session IDs in: {:?}",
            total_duration
        );
    }

    #[tokio::test]
    async fn test_compaction_metadata_operations_performance() {
        let start = Instant::now();

        let mut session = create_test_session_with_messages(10);

        // Add many compaction metadata entries
        for i in 0..1000 {
            session.compaction_history.push(CompactionMetadata {
                compacted_at: SystemTime::now(),
                original_message_count: 10 + i,
                original_token_count: 1000 + (i * 10),
                compressed_token_count: 200 + (i * 2),
                compression_ratio: 0.2,
                ..Default::default()
            });
        }

        // Test was_recently_compacted performance
        let recent_check_start = Instant::now();
        let _was_recent = session.was_recently_compacted(5);
        let recent_check_duration = recent_check_start.elapsed();

        let total_duration = start.elapsed();

        assert!(
            recent_check_duration < Duration::from_millis(10),
            "Recent compaction check should be fast: {:?}",
            recent_check_duration
        );
        assert!(
            total_duration < Duration::from_millis(100),
            "Total metadata operations should be fast: {:?}",
            total_duration
        );

        println!(
            "Processed 1000 compaction metadata entries in: {:?}",
            total_duration
        );
    }

    #[tokio::test]
    async fn test_stress_concurrent_token_counting() {
        let start = Instant::now();

        let mut handles = Vec::new();

        // Run token counting concurrently across multiple sessions
        for task_id in 0..20 {
            let handle = tokio::spawn(async move {
                let counter = SimpleTokenCounter;
                let session = create_large_content_session(100, 25);
                let _usage = counter.count_session_tokens(&session);
                task_id
            });
            handles.push(handle);
        }

        let results: Vec<usize> = futures::future::try_join_all(handles).await.unwrap();

        let duration = start.elapsed();

        assert_eq!(results.len(), 20);
        assert!(
            duration < Duration::from_secs(2),
            "Concurrent token counting should complete quickly: {:?}",
            duration
        );

        println!(
            "Concurrent token counting for 20 large sessions took: {:?}",
            duration
        );
    }

    #[tokio::test]
    async fn test_extreme_session_size_handling() {
        // Test that the system handles very large sessions gracefully
        let start = Instant::now();

        // Create extremely large session (2000 messages)
        let extreme_session = create_large_content_session(2000, 20);

        let counter = SimpleTokenCounter;
        let usage = counter.count_session_tokens(&extreme_session);

        let duration = start.elapsed();

        assert!(
            usage.total > 20000,
            "Extreme session should have very high token count"
        );
        assert_eq!(usage.by_message.len(), 2000);
        assert!(
            duration < Duration::from_secs(2),
            "Even extreme sessions should be processed reasonably quickly: {:?}",
            duration
        );

        println!(
            "Processed extreme session (2000 messages) in: {:?}",
            duration
        );
        println!("Total tokens: {}", usage.total);
    }
}
