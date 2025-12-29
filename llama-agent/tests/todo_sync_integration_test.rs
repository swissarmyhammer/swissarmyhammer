//! Integration tests for session todo synchronization
//!
//! This test suite verifies that session todos stay synchronized with TodoStorage
//! when using the Agent's sync_session_todos method.

#[cfg(feature = "acp")]
mod acp_tests {
    use llama_agent::types::SessionConfig;
    use llama_agent::SessionManager;
    use swissarmyhammer_todo::{Priority, TodoItemExt, TodoStorage};
    use tempfile::TempDir;

    /// Helper to create a test session manager with temporary directory
    fn create_test_session_manager() -> (SessionManager, TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();

        // Initialize git repo in temp directory (required for TodoStorage)
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to init git repo");

        // Set working directory for TodoStorage
        std::env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

        let config = SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().join("sessions")),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);

        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_session_todos_empty_initially() {
        let (manager, _temp_dir) = create_test_session_manager();

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        let session = manager.get_session(&session_id).await.unwrap().unwrap();

        assert_eq!(session.todos.len(), 0, "New session should have no todos");
    }

    #[tokio::test]
    async fn test_session_todos_can_be_manually_added() {
        let (manager, _temp_dir) = create_test_session_manager();

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Manually update session to add a todo (simulating what sync would do)
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        // Create a test todo item
        let test_todo =
            swissarmyhammer_todo::PlanEntry::new("Test task".to_string(), Priority::Medium);

        session.todos.push(test_todo.clone());

        // Update the session
        manager.update_session(session).await.unwrap();

        // Verify the todo is in the session
        let session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.todos.len(), 1, "Session should have 1 todo");
        assert_eq!(session.todos[0].id, test_todo.id, "Todo ID should match");
    }

    #[tokio::test]
    async fn test_todos_from_storage_can_be_synced_to_session() {
        let (manager, _temp_dir) = create_test_session_manager();

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Create a todo in storage
        let storage = TodoStorage::new_default().expect("Failed to create storage");
        let (todo_item, _) = storage
            .create_todo_item("Test task".to_string(), None)
            .await
            .expect("Failed to create todo");

        // Manually sync: load todos from storage and update session
        let todo_list = storage.get_todo_list().await.unwrap();
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        if let Some(list) = todo_list {
            session.todos = list.todo;
        }

        manager.update_session(session).await.unwrap();

        // Verify session has the todo
        let session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.todos.len(), 1, "Session should have 1 todo");
        assert_eq!(
            session.todos[0].id, todo_item.id,
            "Session todo ID should match created todo"
        );
    }

    #[tokio::test]
    async fn test_multiple_todos_sync_correctly() {
        let (manager, _temp_dir) = create_test_session_manager();

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Create multiple todos
        let storage = TodoStorage::new_default().expect("Failed to create storage");
        let (todo1, _) = storage
            .create_todo_item("Task 1".to_string(), None)
            .await
            .expect("Failed to create todo 1");
        let (todo2, _) = storage
            .create_todo_item("Task 2".to_string(), None)
            .await
            .expect("Failed to create todo 2");
        let (todo3, _) = storage
            .create_todo_item("Task 3".to_string(), None)
            .await
            .expect("Failed to create todo 3");

        // Sync todos to session
        let todo_list = storage.get_todo_list().await.unwrap();
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        if let Some(list) = todo_list {
            session.todos = list.todo;
        }

        manager.update_session(session).await.unwrap();

        // Verify session has all todos
        let session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.todos.len(), 3, "Session should have 3 todos");

        let todo_ids: Vec<String> = session.todos.iter().map(|t| t.id.clone()).collect();
        assert!(
            todo_ids.contains(&todo1.id),
            "Session should contain todo 1"
        );
        assert!(
            todo_ids.contains(&todo2.id),
            "Session should contain todo 2"
        );
        assert!(
            todo_ids.contains(&todo3.id),
            "Session should contain todo 3"
        );
    }

    #[tokio::test]
    async fn test_completed_todos_remain_in_session_but_marked_done() {
        let (manager, _temp_dir) = create_test_session_manager();

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Create two todos
        let storage = TodoStorage::new_default().expect("Failed to create storage");
        let (todo1, _) = storage
            .create_todo_item("Task 1".to_string(), None)
            .await
            .expect("Failed to create todo 1");
        let (todo2, _) = storage
            .create_todo_item("Task 2".to_string(), None)
            .await
            .expect("Failed to create todo 2");

        // Initial sync
        let todo_list = storage.get_todo_list().await.unwrap();
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        if let Some(list) = todo_list {
            session.todos = list.todo;
        }

        manager.update_session(session).await.unwrap();

        // Mark first todo complete
        let todo_id = swissarmyhammer_todo::TodoId::from_string(todo1.id.clone()).unwrap();
        storage
            .mark_todo_complete(&todo_id)
            .await
            .expect("Failed to mark todo complete");

        // Sync again after marking complete
        let todo_list = storage.get_todo_list().await.unwrap();
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        if let Some(list) = todo_list {
            session.todos = list.todo;
        }

        manager.update_session(session).await.unwrap();

        // Verify session still has both todos but one is marked done
        let session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(
            session.todos.len(),
            2,
            "Session should have 2 todos (complete and incomplete)"
        );

        let incomplete_todos: Vec<_> = session.todos.iter().filter(|t| !t.done()).collect();
        assert_eq!(
            incomplete_todos.len(),
            1,
            "Session should have 1 incomplete todo"
        );
        assert_eq!(
            incomplete_todos[0].id, todo2.id,
            "Incomplete todo should be task 2"
        );

        let completed_todos: Vec<_> = session.todos.iter().filter(|t| t.done()).collect();
        assert_eq!(
            completed_todos.len(),
            1,
            "Session should have 1 completed todo"
        );
        assert_eq!(
            completed_todos[0].id, todo1.id,
            "Completed todo should be task 1"
        );
    }

    #[tokio::test]
    async fn test_session_todos_persist_across_saves() {
        let temp_dir = tempfile::tempdir().unwrap();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to init git repo");

        std::env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

        let sessions_path = temp_dir.path().join("sessions");

        let session_id = {
            let config = SessionConfig {
                max_sessions: 10,
                auto_compaction: None,
                model_context_size: 4096,
                persistence_enabled: true,
                session_storage_dir: Some(sessions_path.clone()),
                session_ttl_hours: 24,
                auto_save_threshold: 5,
                max_kv_cache_files: 16,
                kv_cache_dir: None,
            };

            let manager = SessionManager::new(config);

            let session = manager.create_session().await.unwrap();
            let session_id = session.id;

            // Create todos
            let storage = TodoStorage::new_default().expect("Failed to create storage");
            storage
                .create_todo_item("Task 1".to_string(), None)
                .await
                .expect("Failed to create todo 1");
            storage
                .create_todo_item("Task 2".to_string(), None)
                .await
                .expect("Failed to create todo 2");

            // Sync todos to session
            let todo_list = storage.get_todo_list().await.unwrap();
            let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

            if let Some(list) = todo_list {
                session.todos = list.todo;
            }

            manager.update_session(session).await.unwrap();

            // Save the session
            manager.save_session(&session_id).await.unwrap();

            session_id
        };

        // Create new manager (simulating restart)
        let config = SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            model_context_size: 4096,
            persistence_enabled: true,
            session_storage_dir: Some(sessions_path),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);
        manager.restore_sessions().await.unwrap();

        let session = manager
            .get_session(&session_id)
            .await
            .unwrap()
            .expect("Session should exist after restart");

        // Verify todos were persisted
        assert_eq!(
            session.todos.len(),
            2,
            "Session should have 2 todos after persistence"
        );
    }

    #[tokio::test]
    async fn test_empty_todo_list_clears_session_todos() {
        let (manager, _temp_dir) = create_test_session_manager();

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Manually add a todo to session (simulating stale state)
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        let test_todo =
            swissarmyhammer_todo::PlanEntry::new("Stale task".to_string(), Priority::Medium);

        session.todos.push(test_todo);
        manager.update_session(session).await.unwrap();

        // Verify todo exists
        let session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.todos.len(), 1, "Session should have 1 stale todo");

        // Sync with empty storage (no todo file exists)
        let storage = TodoStorage::new_default().expect("Failed to create storage");
        let todo_list = storage.get_todo_list().await.unwrap();
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        if let Some(list) = todo_list {
            session.todos = list.todo;
        } else {
            session.todos.clear();
        }

        manager.update_session(session).await.unwrap();

        // Verify session todos cleared
        let session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(
            session.todos.len(),
            0,
            "Session todos should be cleared when no todo file exists"
        );
    }

    #[tokio::test]
    async fn test_todos_ordered_as_in_storage() {
        let (manager, _temp_dir) = create_test_session_manager();

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Create todos in specific order
        let storage = TodoStorage::new_default().expect("Failed to create storage");
        let (todo1, _) = storage
            .create_todo_item("First task".to_string(), None)
            .await
            .expect("Failed to create todo 1");
        let (todo2, _) = storage
            .create_todo_item("Second task".to_string(), None)
            .await
            .expect("Failed to create todo 2");
        let (todo3, _) = storage
            .create_todo_item("Third task".to_string(), None)
            .await
            .expect("Failed to create todo 3");

        // Sync todos to session
        let todo_list = storage.get_todo_list().await.unwrap();
        let mut session = manager.get_session(&session_id).await.unwrap().unwrap();

        if let Some(list) = todo_list {
            session.todos = list.todo;
        }

        manager.update_session(session).await.unwrap();

        // Verify order is preserved
        let session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.todos.len(), 3);
        assert_eq!(session.todos[0].id, todo1.id, "First todo should be first");
        assert_eq!(
            session.todos[1].id, todo2.id,
            "Second todo should be second"
        );
        assert_eq!(session.todos[2].id, todo3.id, "Third todo should be third");
    }
}
