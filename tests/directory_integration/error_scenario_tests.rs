//! Error Scenario and Edge Case Tests
//!
//! These tests validate error handling, edge cases, and boundary conditions
//! in the Git repository-centric directory system. They ensure robust
//! behavior under adverse conditions and proper error reporting.

use super::{GitRepositoryTestGuard, create_corrupt_git_repository};
use swissarmyhammer::directory_utils::{
    find_git_repository_root,
    find_swissarmyhammer_directory,
    get_or_create_swissarmyhammer_directory
};
use swissarmyhammer::error::SwissArmyHammerError;
use swissarmyhammer::security::MAX_DIRECTORY_DEPTH;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Test behavior when not in a Git repository
///
/// This test validates that directory resolution functions fail gracefully
/// and provide clear error messages when not in a Git repository.
#[test]
fn test_not_in_git_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let non_git_dir = temp_dir.path().join("regular-directory");
    fs::create_dir_all(&non_git_dir).expect("Failed to create non-Git directory");

    // Create .swissarmyhammer directory in non-Git location
    let swissarmyhammer_dir = non_git_dir.join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir).expect("Failed to create .swissarmyhammer");

    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&non_git_dir).expect("Failed to change to non-Git directory");

    // Git repository root should not be found
    let git_root = find_git_repository_root();
    assert!(git_root.is_none(), "Should not find Git repository root in non-Git directory");

    // SwissArmyHammer directory should not be found (requires Git repository)
    let swissarmyhammer_found = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_found.is_none(), 
           "Should not find .swissarmyhammer directory outside Git repository");

    // Creating SwissArmyHammer directory should fail with proper error
    let create_result = get_or_create_swissarmyhammer_directory();
    assert!(create_result.is_err(), "Should fail to create .swissarmyhammer outside Git repository");

    match create_result.unwrap_err() {
        SwissArmyHammerError::NotInGitRepository => {
            // Expected error type
        }
        other => panic!("Expected NotInGitRepository error, got {:?}", other),
    }

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test behavior with corrupt Git repository
///
/// This test validates behavior when Git repository structure is corrupted
/// or incomplete, ensuring graceful degradation.
#[test]
fn test_corrupt_git_repository() {
    let temp_dir = create_corrupt_git_repository();
    let project_root = temp_dir.path();

    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(project_root).expect("Failed to change to corrupt Git directory");

    // Git repository detection should still work (presence of .git directory is sufficient)
    let git_root = find_git_repository_root();
    assert!(git_root.is_some(), "Should detect Git repository even if corrupt");
    assert_eq!(git_root.unwrap(), project_root);

    // SwissArmyHammer directory operations should work normally
    // (they don't depend on Git repository validity, just presence of .git)
    let create_result = get_or_create_swissarmyhammer_directory();
    assert!(create_result.is_ok(), "Should create .swissarmyhammer even with corrupt Git repository");

    let swissarmyhammer_dir = create_result.unwrap();
    assert!(swissarmyhammer_dir.exists());
    assert_eq!(swissarmyhammer_dir, project_root.join(".swissarmyhammer"));

    // Directory resolution should work
    let found_dir = find_swissarmyhammer_directory();
    assert!(found_dir.is_some());
    assert_eq!(found_dir.unwrap(), swissarmyhammer_dir);

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test behavior with .swissarmyhammer as a file instead of directory
///
/// This test validates error handling when .swissarmyhammer exists as a file
/// rather than the expected directory.
#[test]
fn test_swissarmyhammer_as_file() {
    let guard = GitRepositoryTestGuard::new();
    let swissarmyhammer_file = guard.path().join(".swissarmyhammer");

    // Create .swissarmyhammer as a file
    fs::write(&swissarmyhammer_file, "This is a file, not a directory")
        .expect("Failed to create .swissarmyhammer file");

    // find_swissarmyhammer_directory should return None
    let found_dir = find_swissarmyhammer_directory();
    assert!(found_dir.is_none(), 
           "Should not find .swissarmyhammer when it exists as a file");

    // get_or_create_swissarmyhammer_directory should fail appropriately
    let create_result = get_or_create_swissarmyhammer_directory();
    assert!(create_result.is_err(), 
           "Should fail to create .swissarmyhammer over existing file");

    match create_result.unwrap_err() {
        SwissArmyHammerError::DirectoryCreation(_) => {
            // Expected error type when trying to create directory over file
        }
        other => panic!("Expected DirectoryCreation error, got {:?}", other),
    }

    // Verify file still exists and wasn't modified
    assert!(swissarmyhammer_file.exists());
    assert!(swissarmyhammer_file.is_file());
    let content = fs::read_to_string(&swissarmyhammer_file)
        .expect("Failed to read .swissarmyhammer file");
    assert_eq!(content, "This is a file, not a directory");
}

/// Test behavior with read-only file system
///
/// This test validates behavior when file system is read-only or when
/// permissions prevent directory creation.
#[test]
fn test_readonly_filesystem_scenarios() {
    let guard = GitRepositoryTestGuard::new();

    // Create .swissarmyhammer directory first
    let swissarmyhammer_dir = guard.path().join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir).expect("Failed to create .swissarmyhammer");

    // Test 1: Existing readonly .swissarmyhammer directory should be found
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        
        // Make directory readonly
        let mut perms = fs::metadata(&swissarmyhammer_dir)
            .expect("Failed to get permissions")
            .permissions();
        perms.set_mode(0o555); // readonly, executable
        fs::set_permissions(&swissarmyhammer_dir, perms)
            .expect("Failed to set readonly permissions");

        // Should still find the directory
        let found_dir = find_swissarmyhammer_directory();
        assert!(found_dir.is_some(), "Should find readonly .swissarmyhammer directory");
        assert_eq!(found_dir.unwrap(), swissarmyhammer_dir);

        // get_or_create should succeed (directory already exists)
        let create_result = get_or_create_swissarmyhammer_directory();
        assert!(create_result.is_ok(), "Should succeed when directory exists even if readonly");

        // Restore permissions for cleanup
        let mut restore_perms = perms;
        restore_perms.set_mode(0o755);
        fs::set_permissions(&swissarmyhammer_dir, restore_perms)
            .expect("Failed to restore permissions");
    }

    // Test 2: Create subdirectory in readonly parent
    fs::remove_dir_all(&swissarmyhammer_dir).expect("Failed to remove .swissarmyhammer");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Make parent directory readonly
        let mut parent_perms = fs::metadata(guard.path())
            .expect("Failed to get parent permissions")
            .permissions();
        parent_perms.set_mode(0o555); // readonly
        fs::set_permissions(guard.path(), parent_perms)
            .expect("Failed to set parent readonly permissions");

        // Should fail to create .swissarmyhammer
        let create_result = get_or_create_swissarmyhammer_directory();
        assert!(create_result.is_err(), "Should fail to create directory in readonly parent");

        // Restore permissions
        let mut restore_perms = parent_perms;
        restore_perms.set_mode(0o755);
        fs::set_permissions(guard.path(), restore_perms)
            .expect("Failed to restore parent permissions");
    }
}

/// Test behavior at maximum directory depth
///
/// This test validates that directory resolution respects MAX_DIRECTORY_DEPTH
/// limits and handles very deep directory structures appropriately.
#[test]
fn test_maximum_directory_depth() {
    let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
    
    // Create directory structure at exactly MAX_DIRECTORY_DEPTH
    let max_depth_path = guard.create_deep_structure(MAX_DIRECTORY_DEPTH);
    
    // Should still find Git repository from max depth
    guard.cd_to_subdir(max_depth_path.strip_prefix(guard.path()).unwrap())
        .expect("Failed to change to max depth directory");

    let git_root = find_git_repository_root();
    assert!(git_root.is_some(), 
           "Should find Git repository at exactly MAX_DIRECTORY_DEPTH");
    assert_eq!(git_root.unwrap(), guard.path());

    let swissarmyhammer_dir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_dir.is_some(), 
           "Should find .swissarmyhammer directory at exactly MAX_DIRECTORY_DEPTH");

    // Create structure beyond MAX_DIRECTORY_DEPTH (this will exceed the limit)
    let beyond_max_path = guard.create_deep_structure(MAX_DIRECTORY_DEPTH + 5);
    
    guard.cd_to_subdir(beyond_max_path.strip_prefix(guard.path()).unwrap())
        .expect("Failed to change to beyond max depth directory");

    let git_root_beyond = find_git_repository_root();
    assert!(git_root_beyond.is_none(), 
           "Should not find Git repository beyond MAX_DIRECTORY_DEPTH");

    let swissarmyhammer_beyond = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_beyond.is_none(), 
           "Should not find .swissarmyhammer directory beyond MAX_DIRECTORY_DEPTH");
}

/// Test behavior with special characters in paths
///
/// This test validates handling of paths containing special characters,
/// spaces, unicode characters, and other edge cases.
#[test]
fn test_special_characters_in_paths() {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    
    // Create directory with special characters
    let special_chars_dir = temp_dir.path().join("project with spaces & símböls 🚀");
    fs::create_dir_all(&special_chars_dir).expect("Failed to create special chars directory");

    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&special_chars_dir)
        .expect("Failed to change to special chars directory");

    // Initialize Git repository
    let _git_repo = git2::Repository::init(&special_chars_dir)
        .expect("Failed to initialize Git repository with special chars");

    // Directory resolution should work with special characters
    let git_root = find_git_repository_root();
    assert!(git_root.is_some(), "Should find Git repository with special characters in path");
    assert_eq!(git_root.unwrap(), special_chars_dir);

    // Creating .swissarmyhammer should work
    let create_result = get_or_create_swissarmyhammer_directory();
    assert!(create_result.is_ok(), "Should create .swissarmyhammer with special characters in path");

    let swissarmyhammer_dir = create_result.unwrap();
    assert!(swissarmyhammer_dir.exists());
    assert!(swissarmyhammer_dir.to_string_lossy().contains("símböls"));
    assert!(swissarmyhammer_dir.to_string_lossy().contains("🚀"));

    // Create subdirectories with special characters
    let special_subdir = swissarmyhammer_dir.join("memos");
    fs::create_dir_all(&special_subdir).expect("Failed to create special subdirectory");

    // Create file with special characters in content
    let memo_content = "# Mémo with Émojis 🎉\n\nThis memo contains ünïcödé characters: αβγδε";
    let memo_file = special_subdir.join("spécial_mémo.md");
    fs::write(&memo_file, memo_content).expect("Failed to write memo with special characters");

    assert!(memo_file.exists());
    let read_content = fs::read_to_string(&memo_file)
        .expect("Failed to read memo with special characters");
    assert_eq!(read_content, memo_content);

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test behavior with symbolic links
///
/// This test validates handling of symbolic links in directory resolution
/// and ensures security considerations are maintained.
#[test]
#[cfg(unix)]
fn test_symbolic_links() {
    let temp_dir = TempDir::new().expect("Failed to create temporary directory");
    let real_project = temp_dir.path().join("real-project");
    let link_project = temp_dir.path().join("link-project");
    
    fs::create_dir_all(&real_project).expect("Failed to create real project directory");

    let original_cwd = std::env::current_dir().expect("Failed to get current directory");
    std::env::set_current_dir(&real_project).expect("Failed to change to real project");

    // Initialize Git repository in real location
    let _git_repo = git2::Repository::init(&real_project)
        .expect("Failed to initialize Git repository");

    // Create .swissarmyhammer in real location
    let real_swissarmyhammer = real_project.join(".swissarmyhammer");
    fs::create_dir_all(&real_swissarmyhammer).expect("Failed to create real .swissarmyhammer");

    std::env::set_current_dir(temp_dir.path()).expect("Failed to change to temp directory");

    // Create symbolic link to project directory
    std::os::unix::fs::symlink(&real_project, &link_project)
        .expect("Failed to create symbolic link");

    // Test from symbolic link
    std::env::set_current_dir(&link_project).expect("Failed to change to linked project");

    let git_root = find_git_repository_root();
    assert!(git_root.is_some(), "Should find Git repository through symbolic link");

    let swissarmyhammer_dir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_dir.is_some(), 
           "Should find .swissarmyhammer directory through symbolic link");

    // Test creating file through symbolic link
    let memo_content = "# Symlink Memo\n\nCreated through symbolic link.";
    let memo_file = swissarmyhammer_dir.unwrap().join("memos").join("symlink_memo.md");
    fs::create_dir_all(memo_file.parent().unwrap()).expect("Failed to create memos directory");
    fs::write(&memo_file, memo_content).expect("Failed to write memo through symlink");

    // Verify file exists in real location
    let real_memo_file = real_swissarmyhammer.join("memos/symlink_memo.md");
    assert!(real_memo_file.exists(), "Memo should exist in real location");
    let read_content = fs::read_to_string(&real_memo_file)
        .expect("Failed to read memo from real location");
    assert_eq!(read_content, memo_content);

    std::env::set_current_dir(original_cwd).expect("Failed to restore directory");
}

/// Test behavior with concurrent directory operations
///
/// This test validates thread safety and race condition handling in
/// directory resolution and creation operations.
#[test]
fn test_concurrent_directory_operations() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let guard = GitRepositoryTestGuard::new();
    let project_root = Arc::new(guard.path().to_path_buf());
    
    // Use barrier to synchronize threads
    let barrier = Arc::new(Barrier::new(5));
    let mut handles = vec![];

    for thread_id in 0..5 {
        let project_root = Arc::clone(&project_root);
        let barrier = Arc::clone(&barrier);
        
        let handle = thread::spawn(move || {
            // Change to project directory
            std::env::set_current_dir(&*project_root)
                .expect("Failed to change to project directory");

            // Wait for all threads to be ready
            barrier.wait();

            // All threads try to create .swissarmyhammer simultaneously
            let create_result = get_or_create_swissarmyhammer_directory();
            
            // At least one should succeed
            let swissarmyhammer_dir = create_result
                .unwrap_or_else(|e| panic!("Thread {} failed to create/find .swissarmyhammer: {:?}", thread_id, e));

            // Verify directory exists and is correct
            assert!(swissarmyhammer_dir.exists(), "Thread {}: .swissarmyhammer should exist", thread_id);
            assert_eq!(swissarmyhammer_dir, project_root.join(".swissarmyhammer"), 
                      "Thread {}: .swissarmyhammer should be in correct location", thread_id);

            // Try to create a file unique to this thread
            let memo_content = format!("# Thread {} Memo\n\nCreated by thread {}.", thread_id, thread_id);
            let memo_file = swissarmyhammer_dir.join("memos").join(format!("thread_{}_memo.md", thread_id));
            
            fs::create_dir_all(memo_file.parent().unwrap())
                .expect("Failed to create memos directory");
            fs::write(&memo_file, memo_content)
                .unwrap_or_else(|e| panic!("Thread {} failed to write memo: {}", thread_id, e));

            assert!(memo_file.exists(), "Thread {}: memo file should exist", thread_id);

            thread_id
        });
        
        handles.push(handle);
    }

    // Wait for all threads to complete
    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    // All threads should have completed successfully
    assert_eq!(results, vec![0, 1, 2, 3, 4]);

    // Verify all memo files were created
    let swissarmyhammer_dir = guard.path().join(".swissarmyhammer");
    assert!(swissarmyhammer_dir.exists());

    let memos_dir = swissarmyhammer_dir.join("memos");
    assert!(memos_dir.exists());

    for thread_id in 0..5 {
        let memo_file = memos_dir.join(format!("thread_{}_memo.md", thread_id));
        assert!(memo_file.exists(), "Memo from thread {} should exist", thread_id);
        
        let content = fs::read_to_string(&memo_file)
            .expect("Failed to read thread memo");
        assert!(content.contains(&format!("Created by thread {}.", thread_id)));
    }
}

/// Test behavior with very long paths
///
/// This test validates handling of very long file paths that might approach
/// or exceed system limits.
#[test]
fn test_very_long_paths() {
    let guard = GitRepositoryTestGuard::new().with_swissarmyhammer();
    
    // Create very long nested directory names
    let long_name = "a".repeat(50); // 50 character directory name
    let mut current_path = guard.path().to_path_buf();
    
    // Create several levels of long directory names
    for i in 0..10 {
        current_path = current_path.join(format!("{}{}", long_name, i));
        fs::create_dir_all(&current_path).expect("Failed to create long path directory");
    }

    // Test directory resolution from very long path
    guard.cd_to_subdir(current_path.strip_prefix(guard.path()).unwrap())
        .expect("Failed to change to long path directory");

    let git_root = find_git_repository_root();
    assert!(git_root.is_some(), "Should find Git repository from long path");
    assert_eq!(git_root.unwrap(), guard.path());

    let swissarmyhammer_dir = find_swissarmyhammer_directory();
    assert!(swissarmyhammer_dir.is_some(), "Should find .swissarmyhammer from long path");

    // Create file with very long filename
    let long_filename = format!("{}_memo.md", "b".repeat(100));
    let memo_content = "# Long Path Memo\n\nCreated from very long path.";
    let memo_file = swissarmyhammer_dir.unwrap().join("memos").join(&long_filename);
    
    let create_result = fs::write(&memo_file, memo_content);
    
    // Some systems may have path length limits, so we handle both success and failure
    match create_result {
        Ok(_) => {
            assert!(memo_file.exists(), "Long filename memo should exist if creation succeeded");
            let read_content = fs::read_to_string(&memo_file)
                .expect("Failed to read long filename memo");
            assert_eq!(read_content, memo_content);
        }
        Err(_) => {
            // Path too long for this system - this is acceptable behavior
            // Just verify directory resolution still works
            assert!(swissarmyhammer_dir.unwrap().exists());
        }
    }
}

/// Test behavior with empty directories and edge cases
///
/// This test validates handling of empty directories, missing subdirectories,
/// and other structural edge cases.
#[test]
fn test_empty_directory_edge_cases() {
    let guard = GitRepositoryTestGuard::new();
    
    // Test with empty .swissarmyhammer directory (no subdirectories)
    let swissarmyhammer_dir = guard.path().join(".swissarmyhammer");
    fs::create_dir(&swissarmyhammer_dir).expect("Failed to create empty .swissarmyhammer");

    let found_dir = find_swissarmyhammer_directory();
    assert!(found_dir.is_some(), "Should find empty .swissarmyhammer directory");
    assert_eq!(found_dir.unwrap(), swissarmyhammer_dir);

    // Test creating subdirectories on demand
    let memo_file = swissarmyhammer_dir.join("memos/test_memo.md");
    fs::create_dir_all(memo_file.parent().unwrap()).expect("Failed to create memos subdirectory");
    fs::write(&memo_file, "# Test Memo\n\nContent").expect("Failed to write memo");

    assert!(memo_file.exists());
    assert!(swissarmyhammer_dir.join("memos").exists());

    // Test with hidden files in .swissarmyhammer
    let hidden_file = swissarmyhammer_dir.join(".hidden_config");
    fs::write(&hidden_file, "hidden content").expect("Failed to create hidden file");

    // Directory should still be found and functional
    let found_dir_with_hidden = find_swissarmyhammer_directory();
    assert!(found_dir_with_hidden.is_some());
    assert!(hidden_file.exists());

    // Test with broken symbolic links in .swissarmyhammer
    #[cfg(unix)]
    {
        let broken_link = swissarmyhammer_dir.join("broken_link");
        let nonexistent_target = swissarmyhammer_dir.join("nonexistent");
        
        let _ = std::os::unix::fs::symlink(&nonexistent_target, &broken_link);
        
        // Directory should still work despite broken symbolic link
        let found_dir_with_broken_link = find_swissarmyhammer_directory();
        assert!(found_dir_with_broken_link.is_some());
    }
}

/// Test behavior with case sensitivity issues
///
/// This test validates handling of case sensitivity differences across
/// file systems (case-sensitive vs case-insensitive).
#[test]
fn test_case_sensitivity_scenarios() {
    let guard = GitRepositoryTestGuard::new();
    
    // Create .swissarmyhammer with standard case
    let swissarmyhammer_dir = guard.path().join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir.join("memos"))
        .expect("Failed to create .swissarmyhammer");

    // Verify normal case works
    let found_dir = find_swissarmyhammer_directory();
    assert!(found_dir.is_some());
    assert_eq!(found_dir.unwrap(), swissarmyhammer_dir);

    // Test creating files with different cases
    let memo_content = "# Case Test Memo\n\nTesting case sensitivity.";
    
    let lowercase_memo = swissarmyhammer_dir.join("memos/lowercase.md");
    let uppercase_memo = swissarmyhammer_dir.join("memos/UPPERCASE.md");
    let mixedcase_memo = swissarmyhammer_dir.join("memos/MixedCase.md");

    fs::write(&lowercase_memo, memo_content).expect("Failed to write lowercase memo");
    fs::write(&uppercase_memo, memo_content).expect("Failed to write uppercase memo");
    fs::write(&mixedcase_memo, memo_content).expect("Failed to write mixed case memo");

    // All files should exist (behavior may differ on case-insensitive systems)
    assert!(lowercase_memo.exists());
    
    // On case-insensitive systems, these might resolve to the same file
    // On case-sensitive systems, they should be different files
    let memo_count = fs::read_dir(swissarmyhammer_dir.join("memos"))
        .expect("Failed to read memos directory")
        .count();
    
    // Should have at least 1 memo file, up to 3 depending on file system
    assert!(memo_count >= 1 && memo_count <= 3, 
           "Should have 1-3 memo files depending on case sensitivity");
}

/// Test behavior with malformed directory structures
///
/// This test validates graceful handling of malformed or corrupted
/// .swissarmyhammer directory structures.
#[test]
fn test_malformed_directory_structures() {
    let guard = GitRepositoryTestGuard::new();
    let swissarmyhammer_dir = guard.path().join(".swissarmyhammer");
    
    // Create .swissarmyhammer with some normal structure
    fs::create_dir_all(&swissarmyhammer_dir.join("memos"))
        .expect("Failed to create .swissarmyhammer");

    // Add some malformed elements
    // 1. File where directory is expected
    let file_where_dir_expected = swissarmyhammer_dir.join("todo");
    fs::write(&file_where_dir_expected, "This should be a directory")
        .expect("Failed to create file instead of directory");

    // 2. Empty subdirectory with unusual name
    let weird_dir = swissarmyhammer_dir.join("...weird-name...");
    fs::create_dir(&weird_dir).expect("Failed to create weird directory");

    // 3. Directory with no permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let noaccess_dir = swissarmyhammer_dir.join("noaccess");
        fs::create_dir(&noaccess_dir).expect("Failed to create no-access directory");
        
        let mut perms = fs::metadata(&noaccess_dir).unwrap().permissions();
        perms.set_mode(0o000); // no permissions
        fs::set_permissions(&noaccess_dir, perms)
            .expect("Failed to set no permissions");
    }

    // Directory should still be found despite malformed structure
    let found_dir = find_swissarmyhammer_directory();
    assert!(found_dir.is_some(), "Should find .swissarmyhammer despite malformed structure");
    assert_eq!(found_dir.unwrap(), swissarmyhammer_dir);

    // Normal operations should still work in unaffected parts
    let memo_content = "# Malformed Structure Test\n\nTesting resilience to malformed structure.";
    let memo_file = swissarmyhammer_dir.join("memos/malformed_test.md");
    fs::write(&memo_file, memo_content).expect("Failed to write memo in malformed structure");

    assert!(memo_file.exists());
    let read_content = fs::read_to_string(&memo_file)
        .expect("Failed to read memo from malformed structure");
    assert_eq!(read_content, memo_content);

    // Clean up permissions for proper cleanup
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let noaccess_dir = swissarmyhammer_dir.join("noaccess");
        if noaccess_dir.exists() {
            let mut restore_perms = fs::metadata(&noaccess_dir).unwrap().permissions();
            restore_perms.set_mode(0o755);
            let _ = fs::set_permissions(&noaccess_dir, restore_perms);
        }
    }
}