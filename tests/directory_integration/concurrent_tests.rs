//! Concurrent Access Tests
//!
//! These tests validate that directory resolution and SwissArmyHammer operations
//! work correctly under concurrent access patterns, including thread safety,
//! race condition prevention, and data consistency.

use super::GitRepositoryTestGuard;
use swissarmyhammer_common::utils::{find_git_repository_root_from, get_or_create_swissarmyhammer_directory_from};
use std::fs;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

/// Helper function to find swissarmyhammer directory
fn find_swissarmyhammer_directory() -> Option<std::path::PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    find_git_repository_root_from(&current_dir).and_then(|git_root| {
        let swissarmyhammer_dir = git_root.join(".swissarmyhammer");
        if swissarmyhammer_dir.exists() && swissarmyhammer_dir.is_dir() {
            Some(swissarmyhammer_dir)
        } else {
            None
        }
    })
}

/// Helper function to find git repository root
fn find_git_repository_root() -> Option<std::path::PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    find_git_repository_root_from(&current_dir)
}

/// Helper function to get or create swissarmyhammer directory
fn get_or_create_swissarmyhammer_directory() -> swissarmyhammer_common::error::Result<std::path::PathBuf> {
    let current_dir = std::env::current_dir().map_err(|e| swissarmyhammer_common::error::SwissArmyHammerError::directory_creation(e))?;
    get_or_create_swissarmyhammer_directory_from(&current_dir)
}
use std::time::{Duration, Instant};

/// Test concurrent directory resolution operations
///
/// This test validates that multiple threads can concurrently perform
/// directory resolution operations without conflicts or race conditions.
#[test]
fn test_concurrent_directory_resolution() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();
    
    let project_root = Arc::new(guard.path().to_path_buf());
    let expected_swissarmyhammer = Arc::new(guard.swissarmyhammer_dir().unwrap());
    let barrier = Arc::new(Barrier::new(8));
    
    let mut handles = vec![];
    let results = Arc::new(Mutex::new(Vec::new()));

    for thread_id in 0..8 {
        let project_root = Arc::clone(&project_root);
        let expected_swissarmyhammer = Arc::clone(&expected_swissarmyhammer);
        let barrier = Arc::clone(&barrier);
        let results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            std::env::set_current_dir(&*project_root)
                .expect("Failed to change to project directory");

            // Wait for all threads to start
            barrier.wait();
            let start_time = Instant::now();

            let mut thread_results = Vec::new();

            // Perform many concurrent directory resolution operations
            for i in 0..100 {
                let git_root = find_git_repository_root();
                assert!(git_root.is_some(), "Thread {}: Should find Git repository (iteration {})", thread_id, i);
                assert_eq!(git_root.unwrap(), *project_root, "Thread {}: Git root should be correct", thread_id);

                let swissarmyhammer_dir = find_swissarmyhammer_directory();
                assert!(swissarmyhammer_dir.is_some(), "Thread {}: Should find .swissarmyhammer directory (iteration {})", thread_id, i);
                assert_eq!(swissarmyhammer_dir.unwrap(), *expected_swissarmyhammer, 
                          "Thread {}: SwissArmyHammer directory should be correct", thread_id);

                thread_results.push((git_root.unwrap(), swissarmyhammer_dir.unwrap()));
            }

            let elapsed = start_time.elapsed();

            // Store results for analysis
            {
                let mut results_guard = results.lock().unwrap();
                results_guard.push((thread_id, elapsed, thread_results));
            }

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let completed_threads: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(completed_threads, vec![0, 1, 2, 3, 4, 5, 6, 7]);

    // Analyze results
    let results_guard = results.lock().unwrap();
    assert_eq!(results_guard.len(), 8);

    for (thread_id, elapsed, thread_results) in results_guard.iter() {
        assert_eq!(thread_results.len(), 100, "Thread {} should have completed 100 operations", thread_id);
        
        // All results should be consistent
        for (git_root, swissarmyhammer_dir) in thread_results {
            assert_eq!(git_root, &*project_root, "Thread {}: Git root should be consistent", thread_id);
            assert_eq!(swissarmyhammer_dir, &*expected_swissarmyhammer, "Thread {}: SwissArmyHammer dir should be consistent", thread_id);
        }

        // Performance should be reasonable
        assert!(elapsed < &Duration::from_millis(2000), 
               "Thread {} should complete within 2 seconds, took {}ms", thread_id, elapsed.as_millis());
    }
}

/// Test concurrent directory creation operations
///
/// This test validates that multiple threads can concurrently attempt to
/// create .swissarmyhammer directories without conflicts or corruption.
#[test]
fn test_concurrent_directory_creation() {
    let guard = GitRepositoryTestGuard::new(); // No .swissarmyhammer initially
    let project_root = Arc::new(guard.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(6));
    
    let mut handles = vec![];
    let results = Arc::new(Mutex::new(Vec::new()));

    for thread_id in 0..6 {
        let project_root = Arc::clone(&project_root);
        let barrier = Arc::clone(&barrier);
        let results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            std::env::set_current_dir(&*project_root)
                .expect("Failed to change to project directory");

            // Wait for all threads to start
            barrier.wait();

            // All threads attempt to create .swissarmyhammer directory
            let create_result = get_or_create_swissarmyhammer_directory();
            
            // All should succeed (idempotent operation)
            assert!(create_result.is_ok(), "Thread {}: Should successfully create/find .swissarmyhammer", thread_id);
            
            let swissarmyhammer_dir = create_result.unwrap();
            let expected_dir = project_root.join(".swissarmyhammer");
            assert_eq!(swissarmyhammer_dir, expected_dir, "Thread {}: Directory should be in correct location", thread_id);

            // Verify directory exists and is functional
            assert!(swissarmyhammer_dir.exists(), "Thread {}: Directory should exist", thread_id);
            assert!(swissarmyhammer_dir.is_dir(), "Thread {}: Should be a directory", thread_id);

            // Try to create subdirectories
            let memos_dir = swissarmyhammer_dir.join("memos");
            let todo_dir = swissarmyhammer_dir.join("todo");
            
            let memos_result = fs::create_dir_all(&memos_dir);
            let todo_result = fs::create_dir_all(&todo_dir);

            {
                let mut results_guard = results.lock().unwrap();
                results_guard.push((thread_id, swissarmyhammer_dir, memos_result.is_ok(), todo_result.is_ok()));
            }

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let completed_threads: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(completed_threads, vec![0, 1, 2, 3, 4, 5]);

    // Analyze results
    let results_guard = results.lock().unwrap();
    assert_eq!(results_guard.len(), 6);

    let expected_dir = guard.path().join(".swissarmyhammer");

    for (thread_id, swissarmyhammer_dir, memos_success, todo_success) in results_guard.iter() {
        assert_eq!(swissarmyhammer_dir, &expected_dir, "Thread {}: Directory location should be consistent", thread_id);
        assert!(memos_success, "Thread {}: Should successfully create memos subdirectory", thread_id);
        assert!(todo_success, "Thread {}: Should successfully create todo subdirectory", thread_id);
    }

    // Verify final directory structure
    assert!(expected_dir.exists());
    assert!(expected_dir.join("memos").exists());
    assert!(expected_dir.join("todo").exists());
}

/// Test concurrent file operations within .swissarmyhammer directory
///
/// This test validates that multiple threads can concurrently create and
/// modify files within the .swissarmyhammer directory without conflicts.
#[test]
fn test_concurrent_file_operations() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();
    
    let project_root = Arc::new(guard.path().to_path_buf());
    let swissarmyhammer_dir = Arc::new(guard.swissarmyhammer_dir().unwrap());
    let barrier = Arc::new(Barrier::new(5));
    
    let mut handles = vec![];
    let results = Arc::new(Mutex::new(Vec::new()));

    for thread_id in 0..5 {
        let project_root = Arc::clone(&project_root);
        let swissarmyhammer_dir = Arc::clone(&swissarmyhammer_dir);
        let barrier = Arc::clone(&barrier);
        let results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            std::env::set_current_dir(&*project_root)
                .expect("Failed to change to project directory");

            barrier.wait();

            let mut created_files = Vec::new();

            // Each thread creates multiple files
            for i in 0..20 {
                // Verify directory resolution still works
                let found_dir = find_swissarmyhammer_directory();
                assert!(found_dir.is_some(), "Thread {}: Should find directory (iteration {})", thread_id, i);
                assert_eq!(found_dir.unwrap(), *swissarmyhammer_dir, "Thread {}: Directory should be consistent", thread_id);

                // Create memo file
                let memo_content = format!("# Thread {} Memo {}\n\nCreated by thread {} in iteration {}.", 
                                         thread_id, i, thread_id, i);
                let memo_file = swissarmyhammer_dir.join("memos").join(format!("thread_{}_memo_{:02}.md", thread_id, i));
                
                let memo_result = fs::write(&memo_file, memo_content);
                assert!(memo_result.is_ok(), "Thread {}: Should write memo {} successfully", thread_id, i);
                
                created_files.push(memo_file);

                // Create todo file
                let todo_content = format!(r#"todo:
  - id: 01H8XYZ123ABC456DEF789T{}I{}
    task: "Task {} for thread {}"
    context: "Concurrent testing task"
    done: false
"#, thread_id, i, i, thread_id);
                
                let todo_file = swissarmyhammer_dir.join("todo").join(format!("thread_{}_todo_{:02}.yaml", thread_id, i));
                
                let todo_result = fs::write(&todo_file, todo_content);
                assert!(todo_result.is_ok(), "Thread {}: Should write todo {} successfully", thread_id, i);
                
                created_files.push(todo_file);

                // Small delay to create interleaving
                thread::sleep(Duration::from_millis(1));
            }

            // Verify all created files exist and have correct content
            for file in &created_files {
                assert!(file.exists(), "Thread {}: File {} should exist", thread_id, file.display());
                
                let content = fs::read_to_string(file)
                    .unwrap_or_else(|e| panic!("Thread {}: Failed to read {}: {}", thread_id, file.display(), e));
                
                assert!(content.contains(&format!("thread {}", thread_id)) || 
                       content.contains(&format!("Thread {}", thread_id)), 
                       "Thread {}: File {} should contain thread identifier", thread_id, file.display());
            }

            {
                let mut results_guard = results.lock().unwrap();
                results_guard.push((thread_id, created_files.len()));
            }

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let completed_threads: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(completed_threads, vec![0, 1, 2, 3, 4]);

    // Analyze results
    let results_guard = results.lock().unwrap();
    assert_eq!(results_guard.len(), 5);

    let mut total_files_created = 0;
    for (thread_id, files_created) in results_guard.iter() {
        assert_eq!(*files_created, 40, "Thread {}: Should have created 40 files (20 memos + 20 todos)", thread_id);
        total_files_created += files_created;
    }

    assert_eq!(total_files_created, 200, "Should have created 200 total files");

    // Verify directory contents
    let memos_count = fs::read_dir(swissarmyhammer_dir.join("memos"))
        .expect("Failed to read memos directory")
        .count();
    let todos_count = fs::read_dir(swissarmyhammer_dir.join("todo"))
        .expect("Failed to read todo directory")
        .count();

    assert_eq!(memos_count, 100, "Should have 100 memo files");
    assert_eq!(todos_count, 100, "Should have 100 todo files");
}

/// Test concurrent operations from different subdirectories
///
/// This test validates that threads working from different subdirectories
/// within the repository can all correctly resolve and access the same
/// .swissarmyhammer directory.
#[test]
fn test_concurrent_operations_from_different_subdirectories() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();
    
    let project_root = Arc::new(guard.path().to_path_buf());
    let expected_swissarmyhammer = Arc::new(guard.swissarmyhammer_dir().unwrap());
    
    // Create additional subdirectories for test
    for i in 0..4 {
        let subdir = guard.path().join(format!("test_subdir_{}", i));
        fs::create_dir_all(&subdir.join("nested")).expect("Failed to create test subdirectory");
    }

    let test_directories = vec![
        "", // repository root
        "src",
        "src/lib", 
        "docs",
        "test_subdir_0",
        "test_subdir_1/nested",
        "test_subdir_2",
        "examples",
    ];

    let barrier = Arc::new(Barrier::new(test_directories.len()));
    let mut handles = vec![];
    let results = Arc::new(Mutex::new(Vec::new()));

    for (thread_id, working_dir) in test_directories.iter().enumerate() {
        let project_root = Arc::clone(&project_root);
        let expected_swissarmyhammer = Arc::clone(&expected_swissarmyhammer);
        let barrier = Arc::clone(&barrier);
        let results = Arc::clone(&results);
        let working_dir = working_dir.to_string();

        let handle = thread::spawn(move || {
            // Change to specific subdirectory
            let target_dir = if working_dir.is_empty() {
                project_root.as_ref().clone()
            } else {
                project_root.join(&working_dir)
            };

            if !target_dir.exists() {
                // Skip non-existent directories
                return (thread_id, false, 0);
            }

            std::env::set_current_dir(&target_dir)
                .unwrap_or_else(|e| panic!("Thread {}: Failed to change to {}: {}", thread_id, working_dir, e));

            barrier.wait();

            let mut successful_operations = 0;

            // Perform many operations from this subdirectory
            for i in 0..50 {
                // Test Git repository detection
                let git_root = find_git_repository_root();
                assert!(git_root.is_some(), "Thread {} ({}): Should find Git root (iteration {})", 
                       thread_id, working_dir, i);
                assert_eq!(git_root.unwrap(), *project_root, "Thread {} ({}): Git root should be correct", 
                          thread_id, working_dir);

                // Test SwissArmyHammer directory detection
                let swissarmyhammer_dir = find_swissarmyhammer_directory();
                assert!(swissarmyhammer_dir.is_some(), "Thread {} ({}): Should find .swissarmyhammer (iteration {})", 
                       thread_id, working_dir, i);
                assert_eq!(swissarmyhammer_dir.unwrap(), *expected_swissarmyhammer, 
                          "Thread {} ({}): SwissArmyHammer directory should be correct", thread_id, working_dir);

                // Create a file unique to this thread and iteration
                let memo_content = format!("# Thread {} Memo from {}\n\nIteration: {}\nWorking directory: {}", 
                                         thread_id, working_dir, i, working_dir);
                let memo_file = swissarmyhammer_dir.unwrap()
                    .join("memos")
                    .join(format!("thread_{}_from_{}_iter_{:02}.md", 
                                 thread_id, 
                                 working_dir.replace(['/', '\\'], "_"),
                                 i));

                let write_result = fs::write(&memo_file, memo_content);
                if write_result.is_ok() {
                    successful_operations += 1;
                }
            }

            {
                let mut results_guard = results.lock().unwrap();
                results_guard.push((thread_id, working_dir.clone(), successful_operations));
            }

            (thread_id, true, successful_operations)
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let completed_threads: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    // Analyze results
    let results_guard = results.lock().unwrap();
    
    for (thread_id, working_dir, successful_operations) in results_guard.iter() {
        assert_eq!(*successful_operations, 50, 
                  "Thread {} ({}): Should have completed all 50 operations successfully", 
                  thread_id, working_dir);
    }

    // Verify all memo files were created correctly
    let memos_dir = guard.swissarmyhammer_dir().unwrap().join("memos");
    let memo_files: Vec<_> = fs::read_dir(&memos_dir)
        .expect("Failed to read memos directory")
        .collect();

    // Should have files from all threads that completed successfully
    let active_threads = completed_threads.iter().filter(|(_, success, _)| *success).count();
    let expected_file_count = active_threads * 50;

    // At least the files from successful threads should exist
    // (there might be additional files from previous tests)
    let actual_file_count = memo_files.len();
    assert!(actual_file_count >= expected_file_count, 
           "Should have at least {} memo files, found {}", expected_file_count, actual_file_count);
}

/// Test concurrent operations with rapid directory changes
///
/// This test validates that directory resolution remains consistent even
/// when threads rapidly change working directories.
#[test] 
fn test_concurrent_rapid_directory_changes() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();
    
    // Create deep directory structure for rapid changes
    let deep_path = guard.create_deep_structure(10);
    
    let project_root = Arc::new(guard.path().to_path_buf());
    let expected_swissarmyhammer = Arc::new(guard.swissarmyhammer_dir().unwrap());
    let barrier = Arc::new(Barrier::new(4));
    
    let mut handles = vec![];
    let results = Arc::new(Mutex::new(Vec::new()));

    for thread_id in 0..4 {
        let project_root = Arc::clone(&project_root);
        let expected_swissarmyhammer = Arc::clone(&expected_swissarmyhammer);
        let barrier = Arc::clone(&barrier);
        let results = Arc::clone(&results);
        let deep_path = deep_path.clone();

        let handle = thread::spawn(move || {
            barrier.wait();

            let mut successful_operations = 0;
            let mut directory_changes = 0;

            // Rapidly change directories and perform operations
            for i in 0..100 {
                // Alternate between different directory levels
                let target_dir = match i % 4 {
                    0 => project_root.as_ref().clone(),
                    1 => project_root.join("src"),
                    2 => project_root.join("docs"), 
                    3 => {
                        // Use part of deep path
                        let relative_deep = deep_path.strip_prefix(&*project_root).unwrap();
                        let components: Vec<_> = relative_deep.components().take(3).collect();
                        let partial_deep = components.iter().fold(project_root.as_ref().clone(), |acc, comp| {
                            acc.join(comp.as_os_str())
                        });
                        partial_deep
                    }
                    _ => unreachable!(),
                };

                if target_dir.exists() {
                    let change_result = std::env::set_current_dir(&target_dir);
                    if change_result.is_ok() {
                        directory_changes += 1;

                        // Perform directory resolution operations
                        let git_root = find_git_repository_root();
                        let swissarmyhammer_dir = find_swissarmyhammer_directory();

                        if git_root.is_some() && 
                           git_root.unwrap() == *project_root &&
                           swissarmyhammer_dir.is_some() && 
                           swissarmyhammer_dir.unwrap() == *expected_swissarmyhammer {
                            successful_operations += 1;
                        }

                        // Small delay to increase chance of interleaving
                        thread::sleep(Duration::from_micros(100));
                    }
                }
            }

            {
                let mut results_guard = results.lock().unwrap();
                results_guard.push((thread_id, successful_operations, directory_changes));
            }

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let completed_threads: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(completed_threads, vec![0, 1, 2, 3]);

    // Analyze results
    let results_guard = results.lock().unwrap();
    assert_eq!(results_guard.len(), 4);

    for (thread_id, successful_operations, directory_changes) in results_guard.iter() {
        assert!(*directory_changes > 0, "Thread {}: Should have made directory changes", thread_id);
        assert_eq!(*successful_operations, *directory_changes, 
                  "Thread {}: All directory resolution operations should have succeeded ({}/{})", 
                  thread_id, successful_operations, directory_changes);
    }

    let total_operations: i32 = results_guard.iter().map(|(_, ops, _)| *ops).sum();
    assert!(total_operations >= 200, "Should have performed at least 200 successful operations total");
}

/// Test concurrent stress scenario with all operation types
///
/// This test validates system behavior under high concurrent load with
/// mixed operation types (resolution, creation, file I/O).
#[test]
fn test_concurrent_stress_scenario() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();
    
    let project_root = Arc::new(guard.path().to_path_buf());
    let swissarmyhammer_dir = Arc::new(guard.swissarmyhammer_dir().unwrap());
    let barrier = Arc::new(Barrier::new(6));
    
    let mut handles = vec![];
    let results = Arc::new(Mutex::new(Vec::new()));

    for thread_id in 0..6 {
        let project_root = Arc::clone(&project_root);
        let swissarmyhammer_dir = Arc::clone(&swissarmyhammer_dir);
        let barrier = Arc::clone(&barrier);
        let results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            std::env::set_current_dir(&*project_root)
                .expect("Failed to change to project directory");

            barrier.wait();
            let start_time = Instant::now();

            let mut operations_completed = 0;
            let mut directory_resolutions = 0;
            let mut file_operations = 0;
            let mut directory_creations = 0;

            // Perform mixed operations under stress
            for i in 0..200 {
                match i % 6 {
                    0 | 1 => {
                        // Directory resolution operations
                        let git_root = find_git_repository_root();
                        let swissarmyhammer_found = find_swissarmyhammer_directory();
                        
                        if git_root.is_some() && swissarmyhammer_found.is_some() {
                            directory_resolutions += 1;
                            operations_completed += 1;
                        }
                    }
                    2 | 3 => {
                        // File operations
                        let memo_content = format!("# Stress Test Memo {}\n\nThread: {}\nIteration: {}\nTimestamp: {:?}", 
                                                 i, thread_id, i, Instant::now());
                        let memo_file = swissarmyhammer_dir.join("memos")
                            .join(format!("stress_thread_{}_memo_{:03}.md", thread_id, i));
                        
                        if fs::write(&memo_file, memo_content).is_ok() {
                            file_operations += 1;
                            operations_completed += 1;
                        }
                    }
                    4 => {
                        // Directory creation operations (subdirectories)
                        let subdir = swissarmyhammer_dir.join(format!("temp_subdir_{}_{}", thread_id, i));
                        if fs::create_dir_all(&subdir).is_ok() {
                            directory_creations += 1;
                            operations_completed += 1;
                            
                            // Clean up immediately to avoid too many directories
                            let _ = fs::remove_dir(&subdir);
                        }
                    }
                    5 => {
                        // Mixed operation: resolve directory then create file
                        let swissarmyhammer_found = find_swissarmyhammer_directory();
                        if let Some(dir) = swissarmyhammer_found {
                            let todo_content = format!(r#"todo:
  - id: 01H8XYZ123ABC456DEF789S{}{}
    task: "Stress test task {} from thread {}"
    context: "Generated during concurrent stress test"
    done: false
"#, thread_id, i, i, thread_id);

                            let todo_file = dir.join("todo")
                                .join(format!("stress_thread_{}_todo_{:03}.yaml", thread_id, i));
                            
                            if fs::write(&todo_file, todo_content).is_ok() {
                                file_operations += 1;
                                directory_resolutions += 1;
                                operations_completed += 1;
                            }
                        }
                    }
                    _ => unreachable!(),
                }

                // Micro-sleep to increase interleaving
                if i % 10 == 0 {
                    thread::sleep(Duration::from_micros(50));
                }
            }

            let elapsed = start_time.elapsed();

            {
                let mut results_guard = results.lock().unwrap();
                results_guard.push((thread_id, elapsed, operations_completed, 
                                  directory_resolutions, file_operations, directory_creations));
            }

            thread_id
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    let completed_threads: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(completed_threads, vec![0, 1, 2, 3, 4, 5]);

    // Analyze stress test results
    let results_guard = results.lock().unwrap();
    assert_eq!(results_guard.len(), 6);

    let mut total_operations = 0;
    let mut max_time = Duration::from_millis(0);

    for (thread_id, elapsed, operations_completed, directory_resolutions, 
         file_operations, directory_creations) in results_guard.iter() {
        
        assert!(*operations_completed > 150, 
               "Thread {}: Should complete most operations under stress (completed {})", 
               thread_id, operations_completed);
        
        assert!(*directory_resolutions > 0, 
               "Thread {}: Should perform directory resolutions", thread_id);
        
        assert!(*file_operations > 0, 
               "Thread {}: Should perform file operations", thread_id);

        assert!(elapsed < &Duration::from_millis(10000), 
               "Thread {}: Should complete within 10 seconds under stress, took {}ms", 
               thread_id, elapsed.as_millis());

        total_operations += operations_completed;
        max_time = max_time.max(*elapsed);
    }

    assert!(total_operations >= 900, "Should complete at least 900 total operations under stress");
    assert!(max_time < Duration::from_millis(8000), 
           "Maximum thread time should be under 8 seconds, was {}ms", max_time.as_millis());

    // Verify system is still consistent after stress test
    let final_git_root = find_git_repository_root();
    assert!(final_git_root.is_some(), "Git repository should still be detectable after stress test");
    assert_eq!(final_git_root.unwrap(), *project_root);

    let final_swissarmyhammer = find_swissarmyhammer_directory();
    assert!(final_swissarmyhammer.is_some(), ".swissarmyhammer should still be detectable after stress test");
    assert_eq!(final_swissarmyhammer.unwrap(), *swissarmyhammer_dir);
}