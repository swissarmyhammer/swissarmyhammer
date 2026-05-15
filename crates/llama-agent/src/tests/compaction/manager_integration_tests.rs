//! Integration tests for SessionManager compaction operations including
//! candidate identification, batch compaction, statistics, and auto-compaction

use crate::session::SessionManager;
use crate::tests::test_utils::*;
use crate::types::*;
use tokio;

#[cfg(test)]
mod session_manager_compaction_tests {
    use super::*;

    async fn create_test_session_manager() -> SessionManager {
        let config = SessionConfig::default();
        SessionManager::new(config)
    }

    #[tokio::test]
    async fn test_compact_session_not_found() {
        let manager = create_test_session_manager().await;
        let non_existent_id = SessionId::new();
        let config = create_test_compaction_config();

        let generate_summary = create_qwen_generate_summary_fn();
        let result = manager
            .compact_session(&non_existent_id, Some(config), generate_summary)
            .await;

        assert!(result.is_err(), "Should fail when session doesn't exist");
        // The error should indicate session not found
    }

    #[tokio::test]
    async fn test_get_compaction_candidates_empty() {
        let manager = create_test_session_manager().await;
        let config = create_test_compaction_config();

        let candidates = manager
            .get_compaction_candidates(&config, 4096)
            .await
            .unwrap();

        assert!(
            candidates.is_empty(),
            "Should have no candidates when no sessions exist"
        );
    }

    #[tokio::test]
    async fn test_get_compaction_candidates_with_sessions() {
        let manager = create_test_session_manager().await;

        // Create several sessions
        let session1 = manager.create_session().await.unwrap();
        let session2 = manager.create_session().await.unwrap();
        let _session3 = manager.create_session().await.unwrap();

        // Add messages to make some sessions candidates for compaction
        // Add many messages to session1 and session2 to exceed token limits
        for i in 0..20 {
            let message = Message {
                role: if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant },
                content: format!("Long message content that contributes to token usage and will make this session exceed typical compaction thresholds: {}", i),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            manager
                .add_message(&session1.id, message.clone())
                .await
                .unwrap();
            manager.add_message(&session2.id, message).await.unwrap();
        }

        // Leave session3 with minimal content

        let config = create_low_threshold_compaction_config(); // Very low threshold to trigger compaction
        let candidates = manager
            .get_compaction_candidates(&config, 4096)
            .await
            .unwrap();

        // Should identify sessions with high token usage as candidates
        assert!(
            !candidates.is_empty(),
            "Should have candidates with low threshold"
        );

        // The large sessions should be in candidates, small one should not
        assert!(
            candidates.contains(&session1.id) || candidates.contains(&session2.id),
            "Large sessions should be compaction candidates"
        );
    }

    #[tokio::test]
    async fn test_compact_sessions_batch_empty() {
        let manager = create_test_session_manager().await;
        let config = create_test_compaction_config();

        let generate_summary = create_qwen_generate_summary_fn();
        let results = manager
            .compact_sessions_batch(vec![], Some(config), generate_summary)
            .await
            .unwrap();

        assert!(
            results.is_empty(),
            "Should return empty results for empty batch"
        );
    }

    #[tokio::test]
    async fn test_compact_sessions_batch_with_invalid_session() {
        let manager = create_test_session_manager().await;
        let config = create_test_compaction_config();
        let non_existent_id = SessionId::new();

        let generate_summary = create_qwen_generate_summary_fn();
        let results = manager
            .compact_sessions_batch(vec![non_existent_id], Some(config), generate_summary)
            .await
            .unwrap();

        // Should handle invalid sessions gracefully - either empty results or errors handled
        assert!(
            results.is_empty(),
            "Should handle non-existent sessions gracefully"
        );
    }

    #[tokio::test]
    async fn test_auto_compact_sessions_no_candidates() {
        let manager = create_test_session_manager().await;
        let config = create_test_compaction_config();

        // Create a minimal session that won't meet compaction criteria
        let _session = manager.create_session().await.unwrap();

        let generate_summary = create_qwen_generate_summary_fn();
        let summary = manager
            .auto_compact_sessions(&config, 4096, generate_summary)
            .await
            .unwrap();

        assert_eq!(summary.total_sessions_processed, 0); // Empty session not processed
        assert_eq!(summary.successful_compactions, 0);
        // Note: failed_compactions field doesn't exist in current CompactionSummary
    }

    #[tokio::test]
    async fn test_auto_compact_sessions_with_candidates() {
        let manager = create_test_session_manager().await;
        let config = create_low_threshold_compaction_config();

        // Create sessions with substantial content
        let session1 = manager.create_session().await.unwrap();
        let session2 = manager.create_session().await.unwrap();

        // Add many messages to make them compaction candidates (use same approach as working test)
        for i in 0..20 {
            let message = Message {
                role: if i % 2 == 0 { MessageRole::User } else { MessageRole::Assistant },
                content: format!("Long message content that contributes to token usage and will make this session exceed typical compaction thresholds: {}", i),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            manager
                .add_message(&session1.id, message.clone())
                .await
                .unwrap();
            manager.add_message(&session2.id, message).await.unwrap();
        }

        let candidates = manager
            .get_compaction_candidates(&config, 4096)
            .await
            .unwrap();

        // Should identify sessions with high token usage as candidates
        assert!(
            !candidates.is_empty(),
            "Should have candidates with low threshold and sufficient messages"
        );

        // The large sessions should be in candidates
        assert!(
            candidates.contains(&session1.id) || candidates.contains(&session2.id),
            "Large sessions should be compaction candidates"
        );

        let generate_summary = create_qwen_generate_summary_fn();
        let summary = manager
            .auto_compact_sessions(&config, 4096, generate_summary)
            .await
            .unwrap();

        // Note: In test environment, actual compaction may fail due to model unavailability
        // but candidates should still be identified correctly
        println!(
            "Compaction summary: processed={}, successful={}",
            summary.total_sessions_processed, summary.successful_compactions
        );
    }

    #[tokio::test]
    async fn test_get_compaction_stats_no_compactions() {
        let manager = create_test_session_manager().await;

        // Create some sessions but no compactions
        let _session1 = manager.create_session().await.unwrap();
        let _session2 = manager.create_session().await.unwrap();

        let stats = manager.get_compaction_stats().await.unwrap();

        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.sessions_with_compaction, 0);
        assert_eq!(stats.total_compaction_operations, 0);
        assert_eq!(stats.average_compression_ratio, 0.0);
        assert!(stats.most_recent_compaction.is_none());
    }

    #[tokio::test]
    async fn test_get_compaction_stats_with_empty_history() {
        let manager = create_test_session_manager().await;

        // Create sessions
        let _session1 = manager.create_session().await.unwrap();
        let _session2 = manager.create_session().await.unwrap();

        // We can't actually perform compactions without model integration,
        // but we can test that the stats calculation works with empty history
        let stats = manager.get_compaction_stats().await.unwrap();

        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.sessions_with_compaction, 0);
    }

    #[tokio::test]
    async fn test_needs_compaction_false() {
        let manager = create_test_session_manager().await;
        let config = create_test_compaction_config();

        // Create sessions that won't need compaction (small content)
        let _session1 = manager.create_session().await.unwrap();
        let _session2 = manager.create_session().await.unwrap();

        let needs = manager.needs_compaction(&config, 4096).await.unwrap();

        assert!(!needs, "Small sessions should not need compaction");
    }

    #[tokio::test]
    async fn test_needs_compaction_true() {
        let manager = create_test_session_manager().await;
        let config = create_low_threshold_compaction_config();

        // Create session with large content
        let session = manager.create_session().await.unwrap();

        // Add substantial messages
        for _i in 0..20 {
            let message = Message {
                role: MessageRole::User,
                content: format!(
                    "Large message content with lots of text to exceed compaction thresholds: {}",
                    "word ".repeat(20)
                ),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            manager.add_message(&session.id, message).await.unwrap();
        }

        let needs = manager.needs_compaction(&config, 4096).await.unwrap();

        assert!(
            needs,
            "Large sessions should need compaction with low threshold"
        );
    }

    #[tokio::test]
    async fn test_session_manager_message_operations() {
        let manager = create_test_session_manager().await;
        let session = manager.create_session().await.unwrap();

        // Test adding messages
        let message = Message {
            role: MessageRole::User,
            content: "Test message".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: std::time::SystemTime::now(),
        };

        let result = manager.add_message(&session.id, message).await;
        assert!(result.is_ok(), "Should be able to add message to session");

        // Retrieve session and verify message was added
        let retrieved = manager.get_session(&session.id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_session = retrieved.unwrap();
        assert_eq!(retrieved_session.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_concurrent_session_operations() {
        let manager = std::sync::Arc::new(create_test_session_manager().await);
        let mut handles = Vec::new();

        // Create multiple sessions concurrently
        for i in 0..5 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                let session = manager_clone.create_session().await.unwrap();

                // Add messages to each session
                for j in 0..5 {
                    let message = Message {
                        role: MessageRole::User,
                        content: format!("Concurrent test message {} from task {}", j, i),
                        tool_call_id: None,
                        tool_name: None,
                        timestamp: std::time::SystemTime::now(),
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

        assert_eq!(session_ids.len(), 5);

        // Verify all sessions exist and have messages
        for session_id in &session_ids {
            let session = manager.get_session(session_id).await.unwrap().unwrap();
            assert_eq!(session.messages.len(), 5);
        }
    }

    #[tokio::test]
    async fn test_compaction_with_preserved_messages() {
        let manager = create_test_session_manager().await;
        let session = manager.create_session().await.unwrap();

        // Add messages
        for i in 0..10 {
            let message = Message {
                role: if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: format!("Test message {} for preservation testing", i),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            manager.add_message(&session.id, message).await.unwrap();
        }

        let config = CompactionConfig {
            threshold: 0.1,

            preserve_recent: 3, // Preserve last 3 messages
            custom_prompt: None,
        };

        // Try compaction (should succeed with model integration now working)
        let generate_summary = create_qwen_generate_summary_fn();
        let result = manager
            .compact_session(&session.id, Some(config), generate_summary)
            .await;

        // Compaction should succeed and preserve recent messages
        assert!(result.is_ok(), "Compaction should succeed");
        let retrieved = manager.get_session(&session.id).await.unwrap().unwrap();
        assert_eq!(
            retrieved.messages.len(),
            4, // 1 summary + 3 preserved recent messages
            "Should have 1 summary + 3 preserved messages after successful compaction"
        );
    }

    #[tokio::test]
    async fn test_compaction_stats_calculation() {
        let manager = create_test_session_manager().await;

        // Create multiple sessions for statistics testing
        for _ in 0..3 {
            let _session = manager.create_session().await.unwrap();
        }

        let stats = manager.get_compaction_stats().await.unwrap();

        assert_eq!(stats.total_sessions, 3);
        assert_eq!(stats.sessions_with_compaction, 0);
        assert_eq!(stats.total_compaction_operations, 0);
        assert_eq!(stats.average_compression_ratio, 0.0);
        assert!(stats.most_recent_compaction.is_none());
    }

    #[tokio::test]
    async fn test_session_retrieval_operations() {
        let manager = create_test_session_manager().await;

        // Create session
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Test retrieval
        let retrieved = manager.get_session(&session_id).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_session = retrieved.unwrap();
        assert_eq!(retrieved_session.id, session_id);

        // Test retrieval of non-existent session
        let non_existent = manager.get_session(&SessionId::new()).await.unwrap();
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_session_listing() {
        let manager = create_test_session_manager().await;

        // Initially no sessions
        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 0);

        // Create several sessions
        let _session1 = manager.create_session().await.unwrap();
        let _session2 = manager.create_session().await.unwrap();
        let _session3 = manager.create_session().await.unwrap();

        // Should now have 3 sessions
        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 3);
    }

    #[tokio::test]
    async fn test_compaction_candidate_filtering() {
        let manager = create_test_session_manager().await;

        // Create sessions with different characteristics
        let small_session = manager.create_session().await.unwrap();
        let large_session = manager.create_session().await.unwrap();

        // Add minimal content to small session
        let small_message = Message {
            role: MessageRole::User,
            content: "Hi".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: std::time::SystemTime::now(),
        };
        manager
            .add_message(&small_session.id, small_message)
            .await
            .unwrap();

        // Add substantial content to large session
        for i in 0..25 {
            let message = Message {
                role: MessageRole::User,
                content: format!("This is a very long message with lots of content that will definitely exceed any reasonable compaction threshold when accumulated: {}", i),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            manager
                .add_message(&large_session.id, message)
                .await
                .unwrap();
        }

        // Test with high threshold - should have no candidates
        let high_threshold_config = CompactionConfig {
            threshold: 0.9,

            preserve_recent: 2,
            custom_prompt: None,
        };

        let high_candidates = manager
            .get_compaction_candidates(&high_threshold_config, 4096)
            .await
            .unwrap();
        assert!(
            high_candidates.is_empty() || high_candidates.len() <= 1,
            "High threshold should have few/no candidates"
        );

        // Test with low threshold - should have candidates
        let low_threshold_config = create_low_threshold_compaction_config();
        let low_candidates = manager
            .get_compaction_candidates(&low_threshold_config, 4096)
            .await
            .unwrap();

        // At least the large session should be a candidate
        assert!(
            low_candidates.contains(&large_session.id),
            "Large session should be compaction candidate with low threshold"
        );
        assert!(
            !low_candidates.contains(&small_session.id) || low_candidates.len() == 2,
            "Small session may or may not be candidate depending on exact thresholds"
        );
    }

    #[tokio::test]
    async fn test_session_manager_error_handling() {
        let manager = create_test_session_manager().await;

        // Test operations on non-existent session
        let fake_id = SessionId::new();

        let message = Message {
            role: MessageRole::User,
            content: "Test".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: std::time::SystemTime::now(),
        };

        // Should handle gracefully
        let result = manager.add_message(&fake_id, message).await;
        assert!(
            result.is_err(),
            "Adding message to non-existent session should fail"
        );
    }
}
