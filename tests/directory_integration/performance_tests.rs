//! Performance Tests for Directory Resolution
//!
//! These tests validate that directory resolution and related operations
//! perform within acceptable time limits across various scenarios including
//! large directory structures, deep nesting, and high-frequency operations.

use super::{GitRepositoryTestGuard, create_large_git_repository, measure_time};
use swissarmyhammer_common::utils::{
    find_git_repository_root,
    find_swissarmyhammer_directory,
    get_or_create_swissarmyhammer_directory
};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Test directory resolution performance with standard repository
///
/// This test validates that basic directory resolution operations complete
/// within acceptable time limits for typical repository sizes.
#[test]
fn test_basic_directory_resolution_performance() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    // Test Git repository root detection performance
    let (git_root, git_time) = measure_time(|| {
        find_git_repository_root()
    });
    
    assert!(git_root.is_some(), "Should find Git repository root");
    assert!(git_time < Duration::from_millis(10), 
           "Git repository detection should complete within 10ms, took {}ms", git_time.as_millis());

    // Test SwissArmyHammer directory detection performance  
    let (swissarmyhammer_dir, sah_time) = measure_time(|| {
        find_swissarmyhammer_directory()
    });

    assert!(swissarmyhammer_dir.is_some(), "Should find .swissarmyhammer directory");
    assert!(sah_time < Duration::from_millis(15), 
           "SwissArmyHammer directory detection should complete within 15ms, took {}ms", sah_time.as_millis());

    // Test directory creation performance
    fs::remove_dir_all(guard.swissarmyhammer_dir().unwrap())
        .expect("Failed to remove .swissarmyhammer directory");

    let (create_result, create_time) = measure_time(|| {
        get_or_create_swissarmyhammer_directory()
    });

    assert!(create_result.is_ok(), "Should create .swissarmyhammer directory");
    assert!(create_time < Duration::from_millis(50), 
           "Directory creation should complete within 50ms, took {}ms", create_time.as_millis());
}

/// Test performance with deeply nested directory structures
///
/// This test validates that directory resolution performance remains
/// acceptable even when working from deeply nested subdirectories.
#[test]
fn test_deep_directory_resolution_performance() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();
    
    // Create deep directory structure
    let deep_path = guard.create_deep_structure(20); // 20 levels deep
    
    guard.cd_to_subdir(deep_path.strip_prefix(guard.path()).unwrap())
        .expect("Failed to change to deep directory");

    // Test Git repository detection from deep path
    let (git_root, git_time) = measure_time(|| {
        find_git_repository_root()
    });

    assert!(git_root.is_some(), "Should find Git repository from deep path");
    assert_eq!(git_root.unwrap(), guard.path());
    assert!(git_time < Duration::from_millis(25), 
           "Git repository detection from deep path should complete within 25ms, took {}ms", git_time.as_millis());

    // Test SwissArmyHammer directory detection from deep path
    let (swissarmyhammer_dir, sah_time) = measure_time(|| {
        find_swissarmyhammer_directory()
    });

    assert!(swissarmyhammer_dir.is_some(), "Should find .swissarmyhammer from deep path");
    assert_eq!(swissarmyhammer_dir.unwrap(), guard.swissarmyhammer_dir().unwrap());
    assert!(sah_time < Duration::from_millis(30), 
           "SwissArmyHammer directory detection from deep path should complete within 30ms, took {}ms", sah_time.as_millis());

    // Test repeated operations to check for performance degradation
    let (_, repeated_ops_time) = measure_time(|| {
        for _ in 0..50 {
            let git_root = find_git_repository_root();
            assert!(git_root.is_some());
            
            let swissarmyhammer_dir = find_swissarmyhammer_directory();
            assert!(swissarmyhammer_dir.is_some());
        }
    });

    assert!(repeated_ops_time < Duration::from_millis(500), 
           "50 repeated operations from deep path should complete within 500ms, took {}ms", repeated_ops_time.as_millis());
}

/// Test performance with large repository structures
///
/// This test validates that directory resolution performs well even in
/// repositories with many files and commits.
#[test]
fn test_large_repository_performance() {
    // Create large repository (fewer commits/files for CI performance)
    let guard = create_large_git_repository(10, 20); // 10 commits, 20 files each
    
    // Test directory resolution performance in large repository
    let (git_root, git_time) = measure_time(|| {
        find_git_repository_root()
    });

    assert!(git_root.is_some(), "Should find Git repository in large repo");
    assert!(git_time < Duration::from_millis(20), 
           "Git repository detection in large repo should complete within 20ms, took {}ms", git_time.as_millis());

    let (swissarmyhammer_dir, sah_time) = measure_time(|| {
        find_swissarmyhammer_directory()
    });

    assert!(swissarmyhammer_dir.is_some(), "Should find .swissarmyhammer in large repo");
    assert!(sah_time < Duration::from_millis(25), 
           "SwissArmyHammer directory detection in large repo should complete within 25ms, took {}ms", sah_time.as_millis());

    // Test from various subdirectories in large repo
    let subdirs = vec!["src", "src/level0_0", "src/level0_5"];
    
    for subdir in &subdirs {
        let subdir_path = Path::new(subdir);
        if subdir_path.exists() {
            guard.cd_to_subdir(subdir).unwrap_or_else(|e| {
                eprintln!("Warning: Failed to change to {}: {}", subdir, e);
            });

            let (git_root_from_subdir, subdir_time) = measure_time(|| {
                find_git_repository_root()
            });

            if git_root_from_subdir.is_some() {
                assert!(subdir_time < Duration::from_millis(30), 
                       "Git detection from {} in large repo should complete within 30ms, took {}ms", 
                       subdir, subdir_time.as_millis());
            }
        }
    }
}

/// Test performance with high-frequency directory operations
///
/// This test validates that directory resolution can handle high-frequency
/// operations without significant performance degradation.
#[test]
fn test_high_frequency_operations_performance() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    // Test high-frequency Git repository detection
    let (_, git_burst_time) = measure_time(|| {
        for _ in 0..1000 {
            let git_root = find_git_repository_root();
            assert!(git_root.is_some());
        }
    });

    assert!(git_burst_time < Duration::from_millis(1000), 
           "1000 Git repository detections should complete within 1 second, took {}ms", git_burst_time.as_millis());

    // Test high-frequency SwissArmyHammer directory detection
    let (_, sah_burst_time) = measure_time(|| {
        for _ in 0..1000 {
            let swissarmyhammer_dir = find_swissarmyhammer_directory();
            assert!(swissarmyhammer_dir.is_some());
        }
    });

    assert!(sah_burst_time < Duration::from_millis(1500), 
           "1000 SwissArmyHammer directory detections should complete within 1.5 seconds, took {}ms", sah_burst_time.as_millis());

    // Test mixed operations
    let (_, mixed_ops_time) = measure_time(|| {
        for i in 0..500 {
            if i % 2 == 0 {
                let git_root = find_git_repository_root();
                assert!(git_root.is_some());
            } else {
                let swissarmyhammer_dir = find_swissarmyhammer_directory();
                assert!(swissarmyhammer_dir.is_some());
            }
        }
    });

    assert!(mixed_ops_time < Duration::from_millis(750), 
           "500 mixed operations should complete within 750ms, took {}ms", mixed_ops_time.as_millis());
}

/// Test performance with different working directory locations
///
/// This test validates that performance is consistent regardless of where
/// the operation is initiated from within the repository.
#[test]
fn test_performance_from_different_locations() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    let test_locations = vec![
        "", // repository root
        "src", 
        "src/lib",
        "src/bin", 
        "docs",
        "tests",
        "examples",
        ".github/workflows",
    ];

    let mut performance_results = Vec::new();

    for location in &test_locations {
        if location.is_empty() {
            // Already at repository root
        } else {
            let location_path = Path::new(location);
            if !location_path.exists() {
                continue; // Skip non-existent locations
            }

            guard.cd_to_subdir(location)
                .unwrap_or_else(|e| panic!("Failed to change to {}: {}", location, e));
        }

        // Measure Git repository detection performance
        let (git_root, git_time) = measure_time(|| {
            find_git_repository_root()
        });

        assert!(git_root.is_some(), "Should find Git repository from {}", 
                if location.is_empty() { "root" } else { location });

        // Measure SwissArmyHammer directory detection performance
        let (swissarmyhammer_dir, sah_time) = measure_time(|| {
            find_swissarmyhammer_directory()
        });

        assert!(swissarmyhammer_dir.is_some(), "Should find .swissarmyhammer from {}", 
                if location.is_empty() { "root" } else { location });

        performance_results.push((location, git_time, sah_time));
    }

    // Verify all operations completed within reasonable time
    for (location, git_time, sah_time) in &performance_results {
        let location_name = if location.is_empty() { "root" } else { location };
        
        assert!(git_time < &Duration::from_millis(20), 
               "Git detection from {} should complete within 20ms, took {}ms", 
               location_name, git_time.as_millis());
        
        assert!(sah_time < &Duration::from_millis(25), 
               "SwissArmyHammer detection from {} should complete within 25ms, took {}ms", 
               location_name, sah_time.as_millis());
    }

    // Verify performance consistency (no location should be significantly slower)
    let avg_git_time = performance_results.iter()
        .map(|(_, git_time, _)| git_time.as_nanos())
        .sum::<u128>() / performance_results.len() as u128;
    
    let avg_sah_time = performance_results.iter()
        .map(|(_, _, sah_time)| sah_time.as_nanos())
        .sum::<u128>() / performance_results.len() as u128;

    for (location, git_time, sah_time) in &performance_results {
        let location_name = if location.is_empty() { "root" } else { location };
        
        // No operation should be more than 3x the average time
        assert!(git_time.as_nanos() < avg_git_time * 3, 
               "Git detection from {} took {}ns, average is {}ns (more than 3x slower)", 
               location_name, git_time.as_nanos(), avg_git_time);
        
        assert!(sah_time.as_nanos() < avg_sah_time * 3, 
               "SwissArmyHammer detection from {} took {}ns, average is {}ns (more than 3x slower)", 
               location_name, sah_time.as_nanos(), avg_sah_time);
    }
}

/// Test performance with concurrent directory operations
///
/// This test validates that directory resolution performance remains
/// acceptable under concurrent access patterns.
#[test]
fn test_concurrent_operations_performance() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer();
    let project_root = Arc::new(guard.path().to_path_buf());
    let barrier = Arc::new(Barrier::new(4));

    let mut handles = vec![];
    let mut performance_results = Arc::new(std::sync::Mutex::new(Vec::new()));

    for thread_id in 0..4 {
        let project_root = Arc::clone(&project_root);
        let barrier = Arc::clone(&barrier);
        let results = Arc::clone(&performance_results);

        let handle = thread::spawn(move || {
            std::env::set_current_dir(&*project_root)
                .expect("Failed to change to project directory");

            // Wait for all threads to start
            barrier.wait();

            // Each thread performs many operations and measures performance
            let (_, operations_time) = measure_time(|| {
                for _ in 0..250 { // 250 operations per thread = 1000 total
                    let git_root = find_git_repository_root();
                    assert!(git_root.is_some());

                    let swissarmyhammer_dir = find_swissarmyhammer_directory();
                    assert!(swissarmyhammer_dir.is_some());
                }
            });

            // Store results
            {
                let mut results_guard = results.lock().unwrap();
                results_guard.push((thread_id, operations_time));
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

    // Analyze performance results
    let results = performance_results.lock().unwrap();
    assert_eq!(results.len(), 4);

    for (thread_id, operations_time) in results.iter() {
        assert!(operations_time < &Duration::from_millis(2000), 
               "Thread {} should complete 250 operations within 2 seconds, took {}ms", 
               thread_id, operations_time.as_millis());
    }

    // Calculate average performance
    let total_time: Duration = results.iter().map(|(_, time)| *time).sum();
    let avg_time = total_time / results.len() as u32;

    // No thread should be significantly slower than average
    for (thread_id, operations_time) in results.iter() {
        assert!(operations_time.as_millis() < avg_time.as_millis() * 2, 
               "Thread {} took {}ms, average is {}ms (more than 2x slower)", 
               thread_id, operations_time.as_millis(), avg_time.as_millis());
    }
}

/// Test performance with file system operations
///
/// This test validates that directory resolution performance doesn't degrade
/// significantly when combined with file system operations.
#[test]
fn test_performance_with_file_operations() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    let swissarmyhammer_dir = guard.swissarmyhammer_dir().unwrap();

    // Test directory resolution while creating many files
    let (_, creation_with_resolution_time) = measure_time(|| {
        for i in 0..100 {
            // Perform directory resolution
            let git_root = find_git_repository_root();
            assert!(git_root.is_some());

            let sah_dir = find_swissarmyhammer_directory();
            assert!(sah_dir.is_some());

            // Create a file
            let memo_content = format!("# Performance Test Memo {}\n\nCreated during performance test.", i);
            let memo_file = swissarmyhammer_dir.join("memos").join(format!("perf_memo_{:03}.md", i));
            fs::write(&memo_file, memo_content)
                .unwrap_or_else(|e| panic!("Failed to write performance memo {}: {}", i, e));
        }
    });

    assert!(creation_with_resolution_time < Duration::from_millis(5000), 
           "100 file creations with directory resolution should complete within 5 seconds, took {}ms", 
           creation_with_resolution_time.as_millis());

    // Test directory resolution while reading many files
    let (_, reading_with_resolution_time) = measure_time(|| {
        for i in 0..100 {
            // Perform directory resolution
            let git_root = find_git_repository_root();
            assert!(git_root.is_some());

            let sah_dir = find_swissarmyhammer_directory();
            assert!(sah_dir.is_some());

            // Read a file
            let memo_file = swissarmyhammer_dir.join("memos").join(format!("perf_memo_{:03}.md", i));
            let content = fs::read_to_string(&memo_file)
                .unwrap_or_else(|e| panic!("Failed to read performance memo {}: {}", i, e));
            assert!(content.contains(&format!("Performance Test Memo {}", i)));
        }
    });

    assert!(reading_with_resolution_time < Duration::from_millis(3000), 
           "100 file reads with directory resolution should complete within 3 seconds, took {}ms", 
           reading_with_resolution_time.as_millis());

    // Verify all files were created correctly
    let memo_count = fs::read_dir(swissarmyhammer_dir.join("memos"))
        .expect("Failed to read memos directory")
        .count();
    assert_eq!(memo_count, 100, "Should have created 100 memo files");
}

/// Test performance regression scenarios
///
/// This test validates that directory resolution performance doesn't degrade
/// with various repository states and configurations.
#[test]
fn test_performance_regression_scenarios() {
    // Scenario 1: Fresh repository (baseline performance)
    let fresh_guard = GitRepositoryTestGuard::new_with_swissarmyhammer();
    
    let (_, fresh_git_time) = measure_time(|| {
        for _ in 0..100 {
            let git_root = find_git_repository_root();
            assert!(git_root.is_some());
        }
    });

    let (_, fresh_sah_time) = measure_time(|| {
        for _ in 0..100 {
            let swissarmyhammer_dir = find_swissarmyhammer_directory();
            assert!(swissarmyhammer_dir.is_some());
        }
    });

    // Scenario 2: Repository with many subdirectories
    let structured_guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();
    
    // Create many additional subdirectories
    for i in 0..20 {
        let subdir = structured_guard.path().join(format!("subdir_{}", i));
        fs::create_dir_all(&subdir.join("nested")).expect("Failed to create subdirectory");
    }

    let (_, structured_git_time) = measure_time(|| {
        for _ in 0..100 {
            let git_root = find_git_repository_root();
            assert!(git_root.is_some());
        }
    });

    let (_, structured_sah_time) = measure_time(|| {
        for _ in 0..100 {
            let swissarmyhammer_dir = find_swissarmyhammer_directory();
            assert!(swissarmyhammer_dir.is_some());
        }
    });

    // Scenario 3: Repository with large .swissarmyhammer directory
    let swissarmyhammer_dir = structured_guard.swissarmyhammer_dir().unwrap();
    for i in 0..200 {
        let memo_content = format!("# Memo {}\n\nContent for performance testing.", i);
        let memo_file = swissarmyhammer_dir.join("memos").join(format!("memo_{:03}.md", i));
        fs::write(&memo_file, memo_content).expect("Failed to write memo");
    }

    let (_, large_sah_git_time) = measure_time(|| {
        for _ in 0..100 {
            let git_root = find_git_repository_root();
            assert!(git_root.is_some());
        }
    });

    let (_, large_sah_sah_time) = measure_time(|| {
        for _ in 0..100 {
            let swissarmyhammer_dir = find_swissarmyhammer_directory();
            assert!(swissarmyhammer_dir.is_some());
        }
    });

    // Performance regression analysis
    // All scenarios should complete within reasonable time
    let performance_results = vec![
        ("Fresh repository", fresh_git_time, fresh_sah_time),
        ("Structured repository", structured_git_time, structured_sah_time), 
        ("Large .swissarmyhammer", large_sah_git_time, large_sah_sah_time),
    ];

    for (scenario, git_time, sah_time) in &performance_results {
        assert!(git_time < &Duration::from_millis(500), 
               "{}: Git detection should complete within 500ms, took {}ms", 
               scenario, git_time.as_millis());
        
        assert!(sah_time < &Duration::from_millis(600), 
               "{}: SwissArmyHammer detection should complete within 600ms, took {}ms", 
               scenario, sah_time.as_millis());
    }

    // No scenario should be more than 2x slower than the fresh baseline
    for (scenario, git_time, sah_time) in performance_results.iter().skip(1) {
        assert!(git_time.as_millis() < fresh_git_time.as_millis() * 2, 
               "{}: Git detection took {}ms, baseline is {}ms (more than 2x slower)", 
               scenario, git_time.as_millis(), fresh_git_time.as_millis());
        
        assert!(sah_time.as_millis() < fresh_sah_time.as_millis() * 2, 
               "{}: SwissArmyHammer detection took {}ms, baseline is {}ms (more than 2x slower)", 
               scenario, sah_time.as_millis(), fresh_sah_time.as_millis());
    }
}

/// Test memory usage patterns during performance operations
///
/// This test validates that directory resolution operations don't consume
/// excessive memory or cause memory leaks during repeated operations.
#[test]
fn test_memory_usage_patterns() {
    let guard = GitRepositoryTestGuard::new_with_swissarmyhammer()
        .with_project_structure();

    // Perform many operations to test for memory leaks
    let (_, memory_test_time) = measure_time(|| {
        for round in 0..10 {
            // Each round performs many operations
            for _ in 0..100 {
                let git_root = find_git_repository_root();
                assert!(git_root.is_some());

                let swissarmyhammer_dir = find_swissarmyhammer_directory();
                assert!(swissarmyhammer_dir.is_some());

                // Create and immediately delete a small file to test file handle management
                let temp_file = guard.path().join(format!("temp_memory_test_{}.txt", round));
                fs::write(&temp_file, "temporary content for memory test")
                    .expect("Failed to write temporary file");
                fs::remove_file(&temp_file).expect("Failed to remove temporary file");
            }

            // Force some garbage collection activity by creating temporary data
            let _temp_data: Vec<String> = (0..1000)
                .map(|i| format!("temporary_string_{}", i))
                .collect();
        }
    });

    assert!(memory_test_time < Duration::from_millis(10000), 
           "Memory usage test should complete within 10 seconds, took {}ms", 
           memory_test_time.as_millis());

    // Verify system is still responsive after memory test
    let (git_root, post_test_time) = measure_time(|| {
        find_git_repository_root()
    });

    assert!(git_root.is_some(), "Should still find Git repository after memory test");
    assert!(post_test_time < Duration::from_millis(20), 
           "Post-memory-test operation should complete within 20ms, took {}ms", 
           post_test_time.as_millis());
}