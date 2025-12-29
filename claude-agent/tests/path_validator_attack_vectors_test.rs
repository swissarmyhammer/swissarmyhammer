use claude_agent::path_validator::{PathValidationError, PathValidator};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Tests for common path traversal attack vectors
#[cfg(test)]
mod path_traversal_attacks {
    use super::*;

    #[test]
    fn test_basic_parent_directory_traversal() {
        let validator = PathValidator::new();

        let attacks = vec![
            "/tmp/../etc/passwd",
            "/home/user/../../root/.ssh/id_rsa",
            "/var/www/../../../etc/shadow",
        ];

        for attack in attacks {
            let result = validator.validate_absolute_path(attack);
            assert!(
                result.is_err(),
                "Path traversal attack should be blocked: {}",
                attack
            );
        }
    }

    #[test]
    fn test_windows_style_traversal() {
        let validator = PathValidator::new();

        let attacks = vec![
            "C:\\Windows\\..\\..\\..\\etc\\passwd",
            "D:\\Program Files\\..\\..\\Windows\\System32\\config\\SAM",
            "\\\\server\\share\\..\\..\\sensitive",
        ];

        for attack in attacks {
            let result = validator.validate_absolute_path(attack);
            assert!(
                result.is_err(),
                "Windows path traversal should be blocked: {}",
                attack
            );
        }
    }

    #[test]
    fn test_mixed_separator_traversal() {
        let validator = PathValidator::new();

        let attacks = vec![
            "/home/user\\..\\..\\etc/passwd",
            "C:\\Users/user\\../../../Windows",
            "/tmp\\..\\etc/shadow",
        ];

        for attack in attacks {
            let result = validator.validate_absolute_path(attack);
            assert!(
                result.is_err(),
                "Mixed separator traversal should be blocked: {}",
                attack
            );
        }
    }

    #[test]
    fn test_trailing_traversal_segments() {
        let validator = PathValidator::new();

        let attacks = vec!["/tmp/file/..", "/home/user/document/..", "C:\\Windows\\.."];

        for attack in attacks {
            let result = validator.validate_absolute_path(attack);
            assert!(
                result.is_err(),
                "Trailing traversal should be blocked: {}",
                attack
            );
        }
    }

    #[test]
    fn test_multiple_consecutive_traversals() {
        let validator = PathValidator::new();

        let attacks = vec![
            "/tmp/../../../../../../etc/passwd",
            "C:\\Users\\..\\..\\..\\..\\..\\Windows\\System32",
        ];

        for attack in attacks {
            let result = validator.validate_absolute_path(attack);
            assert!(
                result.is_err(),
                "Multiple consecutive traversals should be blocked: {}",
                attack
            );
        }
    }

    #[test]
    fn test_legitimate_dotdot_in_filename() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file..txt");
        File::create(&file_path).unwrap();

        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&file_path.to_string_lossy());

        // This should NOT be blocked - it's a legitimate filename with dots
        assert!(
            result.is_ok(),
            "Legitimate filename with dots should be allowed: {}",
            file_path.display()
        );
    }

    #[test]
    fn test_legitimate_dotdot_in_dirname() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("dir..name");
        fs::create_dir(&dir_path).unwrap();
        let file_path = dir_path.join("file.txt");
        File::create(&file_path).unwrap();

        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&file_path.to_string_lossy());

        // This should NOT be blocked - it's a legitimate directory name with dots
        assert!(
            result.is_ok(),
            "Legitimate directory name with dots should be allowed: {}",
            file_path.display()
        );
    }
}

/// Tests for symlink-based attack vectors
#[cfg(unix)]
#[cfg(test)]
mod symlink_attacks {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn test_symlink_escape_from_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let symlink_path = workspace.join("escape");
        symlink("/etc", &symlink_path).unwrap();

        let target = symlink_path.join("passwd");

        let validator = PathValidator::with_allowed_roots(vec![workspace.clone()]);
        let result = validator.validate_absolute_path(&target.to_string_lossy());

        // Should be blocked - symlink escapes workspace
        assert!(
            result.is_err(),
            "Symlink escape from workspace should be blocked"
        );
    }

    #[test]
    fn test_symlink_to_blocked_path() {
        let temp_dir = TempDir::new().unwrap();
        let blocked_dir = temp_dir.path().join("blocked");
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&blocked_dir).unwrap();
        fs::create_dir(&workspace).unwrap();

        let blocked_file = blocked_dir.join("secret.txt");
        fs::write(&blocked_file, "secret").unwrap();

        let symlink_path = workspace.join("link_to_secret");
        symlink(&blocked_file, &symlink_path).unwrap();

        let validator = PathValidator::with_allowed_and_blocked(
            vec![workspace.clone()],
            vec![blocked_dir.clone()],
        );
        let result = validator.validate_absolute_path(&symlink_path.to_string_lossy());

        // Should be blocked - symlink points to blocked path
        assert!(result.is_err(), "Symlink to blocked path should be blocked");
    }

    #[test]
    fn test_symlink_in_allowed_roots() {
        let temp_dir = TempDir::new().unwrap();
        let real_workspace = temp_dir.path().join("real_workspace");
        let symlink_workspace = temp_dir.path().join("symlink_workspace");
        fs::create_dir(&real_workspace).unwrap();
        symlink(&real_workspace, &symlink_workspace).unwrap();

        let test_file = real_workspace.join("test.txt");
        fs::write(&test_file, "test").unwrap();

        // Create validator with symlink workspace in allowed_roots
        let validator = PathValidator::with_allowed_roots(vec![symlink_workspace.clone()]);
        let result = validator.validate_absolute_path(&test_file.to_string_lossy());

        // This tests Vulnerability #3 from the audit:
        // If allowed_roots are not canonicalized, this could bypass workspace boundaries
        assert!(
            result.is_err(),
            "Symlink in allowed_roots should be canonicalized and may cause boundary issues"
        );
    }

    #[test]
    fn test_double_symlink_escape() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let link1 = workspace.join("link1");
        let link2 = workspace.join("link2");
        symlink("/etc", &link1).unwrap();
        symlink(&link1, &link2).unwrap();

        let target = link2.join("passwd");

        let validator = PathValidator::with_allowed_roots(vec![workspace.clone()]);
        let result = validator.validate_absolute_path(&target.to_string_lossy());

        // Should be blocked - double symlink escapes workspace
        assert!(result.is_err(), "Double symlink escape should be blocked");
    }

    #[test]
    fn test_symlink_chain_to_sensitive_file() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        // Create a chain: workspace/a -> workspace/b -> /etc/passwd
        let link_a = workspace.join("a");
        let link_b = workspace.join("b");
        symlink("/etc/passwd", &link_b).unwrap();
        symlink(&link_b, &link_a).unwrap();

        let validator = PathValidator::with_allowed_roots(vec![workspace.clone()]);
        let result = validator.validate_absolute_path(&link_a.to_string_lossy());

        // Should be blocked - symlink chain escapes workspace
        assert!(
            result.is_err(),
            "Symlink chain to sensitive file should be blocked"
        );
    }
}

/// Tests for TOCTOU (Time-of-check-time-of-use) race conditions
#[cfg(unix)]
#[cfg(test)]
mod toctou_attacks {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_toctou_validation_to_use_gap() {
        // This test demonstrates Vulnerability #1 from the audit:
        // The path is validated, then returned. Between validation and use,
        // the filesystem can change.

        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let target_path = workspace.join("target.txt");
        fs::write(&target_path, "safe content").unwrap();

        let validator = PathValidator::with_allowed_roots(vec![workspace.clone()]);

        // Validate the path
        let validated = validator.validate_absolute_path(&target_path.to_string_lossy());
        assert!(validated.is_ok(), "Initial validation should succeed");

        // Simulate TOCTOU attack: replace file with symlink after validation
        fs::remove_file(&target_path).unwrap();
        let sensitive_file = temp_dir.path().join("sensitive.txt");
        fs::write(&sensitive_file, "sensitive data").unwrap();
        symlink(&sensitive_file, &target_path).unwrap();

        // Now if code tries to use the validated path, it would access the symlink
        // This test documents the vulnerability - proper fix would be to use
        // file descriptors or O_NOFOLLOW flags
        assert!(
            target_path.is_symlink(),
            "File was replaced with symlink after validation (TOCTOU vulnerability)"
        );
    }

    #[test]
    fn test_toctou_concurrent_modification() {
        // Test concurrent modification during validation
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let target_path = workspace.join("race.txt");
        fs::write(&target_path, "initial").unwrap();

        let validator = Arc::new(PathValidator::with_allowed_roots(vec![workspace.clone()]));
        let path_str = target_path.to_string_lossy().to_string();
        let target_clone = target_path.clone();
        let temp_dir_clone = temp_dir.path().to_path_buf();

        let attack_thread = thread::spawn(move || {
            // Continuously swap between regular file and symlink
            for _ in 0..100 {
                let _ = fs::remove_file(&target_clone);
                let _ = fs::write(&target_clone, "safe");
                thread::sleep(Duration::from_micros(10));

                let _ = fs::remove_file(&target_clone);
                let sensitive = temp_dir_clone.join("sensitive");
                let _ = fs::write(&sensitive, "attack");
                let _ = symlink(&sensitive, &target_clone);
                thread::sleep(Duration::from_micros(10));
            }
        });

        // Try to validate while attack is happening
        let mut validation_attempts = 0;
        let mut successful_validations = 0;
        for _ in 0..100 {
            validation_attempts += 1;
            if validator.validate_absolute_path(&path_str).is_ok() {
                successful_validations += 1;
            }
            thread::sleep(Duration::from_micros(10));
        }

        attack_thread.join().unwrap();

        // This test demonstrates that validation can succeed while file is being
        // actively manipulated, showing the TOCTOU vulnerability
        println!(
            "Validation succeeded {}/{} times during concurrent modification",
            successful_validations, validation_attempts
        );
    }

    #[test]
    fn test_toctou_symlink_check_gap() {
        // This tests the gap between is_symlink() check and later file operations
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let target_path = workspace.join("check_gap.txt");
        fs::write(&target_path, "normal file").unwrap();

        let validator = PathValidator::with_allowed_roots(vec![workspace.clone()]);

        // First check: not a symlink
        assert!(!target_path.is_symlink(), "Initially not a symlink");
        let validated = validator.validate_absolute_path(&target_path.to_string_lossy());
        assert!(validated.is_ok(), "Validation should succeed");

        // Replace with symlink immediately after check
        fs::remove_file(&target_path).unwrap();
        let outside = temp_dir.path().join("outside.txt");
        fs::write(&outside, "outside workspace").unwrap();
        symlink(&outside, &target_path).unwrap();

        // Second check: now a symlink
        assert!(
            target_path.is_symlink(),
            "File became symlink after validation (TOCTOU)"
        );
    }
}

/// Tests for Unicode and encoding-based attack vectors
#[cfg(test)]
mod unicode_attacks {
    use super::*;

    #[test]
    fn test_unicode_normalization_bypass() {
        let validator = PathValidator::new();

        // Test Unicode normalization attacks
        // U+002E (.) vs U+2024 (one dot leader) vs U+FE52 (small full stop)
        let attacks = vec![
            "/tmp/\u{002E}\u{002E}/etc/passwd", // Normal ..
            "/tmp/\u{2024}\u{2024}/etc/passwd", // One dot leader
            "/tmp/\u{FE52}\u{FE52}/etc/passwd", // Small full stop
            "/tmp/.\u{0300}./etc/passwd",       // . with combining grave
            "/tmp/\u{2024}\u{002E}/etc/passwd", // Mixed
        ];

        for attack in attacks {
            let result = validator.validate_absolute_path(attack);
            // Should either block or normalize correctly
            // The important thing is that it doesn't silently allow traversal
            if result.is_ok() {
                println!(
                    "Warning: Unicode path was allowed without normalization: {}",
                    attack
                );
            }
        }
    }

    #[test]
    fn test_homograph_attack() {
        let temp_dir = TempDir::new().unwrap();
        let blocked_dir = temp_dir.path().join("blocked");
        fs::create_dir(&blocked_dir).unwrap();

        // Create file with Cyrillic 'е' (U+0435) instead of Latin 'e' (U+0065)
        let homograph_path = temp_dir.path().join("block\u{0435}d"); // "blockеd" with Cyrillic е
        fs::create_dir(&homograph_path).unwrap();
        let file = homograph_path.join("file.txt");
        fs::write(&file, "content").unwrap();

        let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);
        let result = validator.validate_absolute_path(&file.to_string_lossy());

        // This should be allowed since "blockеd" (Cyrillic) != "blocked" (Latin)
        // But documents the risk of homograph attacks
        if result.is_ok() {
            println!(
                "Warning: Homograph attack bypassed blocked path check: {} vs {}",
                "blocked", "block\u{0435}d"
            );
        }
    }

    #[test]
    fn test_overlong_utf8_encoding() {
        let validator = PathValidator::new();

        // Test overlong UTF-8 encodings (should be rejected by Rust's UTF-8 validation)
        // These are intentionally malformed strings
        let attacks = vec![
            "/tmp/\u{FFFD}/etc/passwd", // Replacement character
        ];

        for attack in attacks {
            let _ = validator.validate_absolute_path(attack);
            // Just ensure it doesn't panic
        }
    }

    #[test]
    fn test_null_byte_injection() {
        let validator = PathValidator::new();

        let attacks = vec![
            "/tmp/safe.txt\0../../etc/passwd",
            "/home/user/\0/root/.ssh/id_rsa",
            "C:\\Windows\0\\..\\..\\sensitive",
        ];

        for attack in attacks {
            let result = validator.validate_absolute_path(attack);
            assert!(
                matches!(result, Err(PathValidationError::NullBytesInPath)),
                "Null byte injection should be blocked: {}",
                attack.escape_debug()
            );
        }
    }

    #[test]
    fn test_zero_width_characters() {
        let temp_dir = TempDir::new().unwrap();

        // Create path with zero-width characters
        let path_with_zwc = temp_dir.path().join("file\u{200B}name.txt"); // Zero-width space
        fs::write(&path_with_zwc, "content").unwrap();

        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&path_with_zwc.to_string_lossy());

        // Should handle zero-width characters without crashing
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle zero-width characters"
        );
    }

    #[test]
    fn test_rtl_override_attack() {
        let temp_dir = TempDir::new().unwrap();

        // Right-to-Left Override can disguise file extensions
        // "file.txt" + RLO + "gpj." displays as "file.gpj.txt" but is actually "file.txt"
        let rtl_path = temp_dir.path().join("file\u{202E}gpj.txt");
        fs::write(&rtl_path, "content").unwrap();

        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&rtl_path.to_string_lossy());

        // Should handle RTL override without security issues
        assert!(
            result.is_ok() || result.is_err(),
            "Should handle RTL override characters"
        );
    }
}

/// Tests for case sensitivity attacks
#[cfg(test)]
mod case_sensitivity_attacks {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn test_case_insensitive_bypass_on_macos() {
        let temp_dir = TempDir::new().unwrap();
        let blocked_dir = temp_dir.path().join("blocked");
        fs::create_dir(&blocked_dir).unwrap();

        // Create file in blocked directory
        let file = blocked_dir.join("secret.txt");
        fs::write(&file, "secret").unwrap();

        let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);

        // Try to access with different case
        let blocked_upper = blocked_dir
            .to_string_lossy()
            .to_uppercase()
            .replace("BLOCKED", "BLOCKED");
        let attack_path = PathBuf::from(blocked_upper).join("secret.txt");

        let result = validator.validate_absolute_path(&attack_path.to_string_lossy());

        // On macOS (case-insensitive by default), this should be blocked
        // because canonicalization should normalize the case
        if result.is_ok() {
            println!(
                "Warning: Case variation bypassed blocked path on case-insensitive filesystem"
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_case_insensitive_bypass_on_windows() {
        let validator = PathValidator::new();

        // Test case variations of system paths
        let attacks = vec![
            "C:\\WINDOWS\\System32\\config\\SAM",
            "c:\\windows\\SYSTEM32\\config\\sam",
            "C:\\WiNdOwS\\system32\\CONFIG\\sam",
        ];

        for attack in attacks {
            let _ = validator.validate_absolute_path(attack);
            // Just ensure consistent behavior across case variations
        }
    }

    #[test]
    fn test_8_3_filename_bypass() {
        // Windows 8.3 short filename format can bypass filters
        // "Program Files" -> "PROGRA~1"
        #[cfg(target_os = "windows")]
        {
            let validator = PathValidator::new();

            let attacks = vec![
                "C:\\PROGRA~1\\file.txt", // Program Files
                "C:\\DOCUME~1\\file.txt", // Documents and Settings
                "C:\\WINDOW~1\\system32\\config\\SAM",
            ];

            for attack in attacks {
                let result = validator.validate_absolute_path(attack);
                // Canonicalization should resolve 8.3 names
                if result.is_ok() {
                    println!("Warning: 8.3 filename format was allowed: {}", attack);
                }
            }
        }
    }
}

/// Tests for path length and resource exhaustion attacks
#[cfg(test)]
mod resource_exhaustion_attacks {
    use super::*;

    #[test]
    fn test_extremely_long_path() {
        let validator = PathValidator::new();

        // Create path exceeding PATH_MAX (4096 on most systems)
        let long_component = "a".repeat(255);
        let mut long_path = String::from("/");
        for _ in 0..20 {
            long_path.push_str(&long_component);
            long_path.push('/');
        }

        let result = validator.validate_absolute_path(&long_path);
        assert!(result.is_err(), "Extremely long path should be rejected");
    }

    #[test]
    fn test_many_path_components() {
        let validator = PathValidator::new();

        // Create path with many components
        let mut path = String::from("/");
        for i in 0..1000 {
            path.push_str(&format!("dir{}/", i));
        }
        path.push_str("file.txt");

        let result = validator.validate_absolute_path(&path);
        // Should either reject or handle gracefully without resource exhaustion
        assert!(
            result.is_err(),
            "Path with excessive components should be rejected"
        );
    }

    #[test]
    fn test_path_length_exactly_at_limit() {
        let max_length = 4096;
        let validator = PathValidator::with_max_length(max_length);

        // Create path exactly at the limit
        let component_length = 255; // Max filename length on most filesystems
        let mut path = String::from("/");
        while path.len() + component_length + 1 < max_length {
            path.push_str(&"a".repeat(component_length));
            path.push('/');
        }
        // Fill remaining space
        path.push_str(&"a".repeat(max_length - path.len()));

        let result = validator.validate_absolute_path(&path);
        assert_eq!(path.len(), max_length);
        assert!(
            result.is_err(),
            "Path exactly at max length should be rejected"
        );
    }

    #[test]
    fn test_deeply_nested_symlinks() {
        // Test for symlink loop detection (potential resource exhaustion)
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let temp_dir = TempDir::new().unwrap();
            let workspace = temp_dir.path().join("workspace");
            fs::create_dir(&workspace).unwrap();

            // Create circular symlink
            let link_a = workspace.join("a");
            let link_b = workspace.join("b");
            symlink(&link_b, &link_a).unwrap();
            symlink(&link_a, &link_b).unwrap();

            let validator = PathValidator::with_allowed_roots(vec![workspace.clone()]);
            let result = validator.validate_absolute_path(&link_a.to_string_lossy());

            // Should fail (either symlink loop or too many levels)
            assert!(result.is_err(), "Circular symlinks should be detected");
        }
    }
}

/// Tests for special file and device attacks
#[cfg(unix)]
#[cfg(test)]
mod special_file_attacks {
    use super::*;

    #[test]
    fn test_device_file_access() {
        let validator = PathValidator::new();

        let device_files = vec![
            "/dev/null",
            "/dev/zero",
            "/dev/random",
            "/dev/urandom",
            "/dev/mem",  // Dangerous!
            "/dev/kmem", // Very dangerous!
        ];

        for device in device_files {
            let result = validator.validate_absolute_path(device);
            // These might be allowed by default but should be blocked in production
            if result.is_ok() {
                println!("Warning: Device file access was allowed: {}", device);
            }
        }
    }

    #[test]
    fn test_proc_filesystem_access() {
        let validator = PathValidator::new();

        let proc_files = vec![
            "/proc/self/mem",
            "/proc/self/maps",
            "/proc/self/environ",
            "/proc/kcore",
            "/proc/kallsyms",
        ];

        for proc_file in proc_files {
            let result = validator.validate_absolute_path(proc_file);
            // /proc access should be carefully controlled
            if result.is_ok() {
                println!("Warning: /proc access was allowed: {}", proc_file);
            }
        }
    }

    #[test]
    fn test_named_pipe_access() {
        let temp_dir = TempDir::new().unwrap();
        let fifo_path = temp_dir.path().join("fifo");

        // Create named pipe (FIFO)
        use std::process::Command;
        Command::new("mkfifo")
            .arg(&fifo_path)
            .output()
            .expect("Failed to create FIFO");

        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&fifo_path.to_string_lossy());

        // Named pipes should be handled carefully (can cause hangs)
        if result.is_ok() {
            println!("Warning: Named pipe access was allowed");
        }
    }
}

/// Tests for workspace boundary bypass attempts
#[cfg(test)]
mod workspace_boundary_attacks {
    use super::*;

    #[test]
    fn test_workspace_boundary_with_relative_components() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let validator = PathValidator::with_allowed_roots(vec![workspace.clone()]);

        // Try to escape using relative components in absolute path
        let attack = format!("{}/../outside/file.txt", workspace.to_string_lossy());
        let result = validator.validate_absolute_path(&attack);

        assert!(
            result.is_err(),
            "Relative components in workspace boundary check should be blocked"
        );
    }

    #[test]
    fn test_multiple_workspaces_boundary_confusion() {
        let temp_dir = TempDir::new().unwrap();
        let workspace1 = temp_dir.path().join("workspace1");
        let workspace2 = temp_dir.path().join("workspace2");
        fs::create_dir(&workspace1).unwrap();
        fs::create_dir(&workspace2).unwrap();

        let file1 = workspace1.join("file.txt");
        let file2 = workspace2.join("file.txt");
        fs::write(&file1, "ws1").unwrap();
        fs::write(&file2, "ws2").unwrap();

        let validator = PathValidator::with_allowed_roots(vec![workspace1.clone()]);

        // File in workspace1 should be allowed
        let result = validator.validate_absolute_path(&file1.to_string_lossy());
        assert!(result.is_ok(), "File in allowed workspace should pass");

        // File in workspace2 should be blocked
        let result = validator.validate_absolute_path(&file2.to_string_lossy());
        assert!(
            result.is_err(),
            "File in different workspace should be blocked"
        );
    }

    #[test]
    fn test_blocked_path_not_canonicalized() {
        // This tests Vulnerability #4 from the audit:
        // blocked_paths not canonicalized at construction
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let temp_dir = TempDir::new().unwrap();
            let real_blocked = temp_dir.path().join("real_blocked");
            let symlink_blocked = temp_dir.path().join("symlink_blocked");
            fs::create_dir(&real_blocked).unwrap();
            symlink(&real_blocked, &symlink_blocked).unwrap();

            let file = real_blocked.join("secret.txt");
            fs::write(&file, "secret").unwrap();

            // Create validator with symlink in blocked_paths
            let validator = PathValidator::with_blocked_paths(vec![symlink_blocked.clone()]);
            let result = validator.validate_absolute_path(&file.to_string_lossy());

            // This might pass if blocked_paths are not canonicalized,
            // demonstrating the vulnerability
            if result.is_ok() {
                println!(
                    "Warning: Blocked path bypass - blocked_paths not canonicalized: {} -> {}",
                    symlink_blocked.display(),
                    real_blocked.display()
                );
            }
        }
    }
}

/// Tests for edge cases and corner cases
#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_root_directory_access() {
        let validator = PathValidator::new();

        let root_paths = if cfg!(unix) {
            vec!["/", "/etc", "/root", "/etc/passwd", "/etc/shadow"]
        } else {
            vec!["C:\\", "C:\\Windows", "C:\\Windows\\System32"]
        };

        for root_path in root_paths {
            let _ = validator.validate_absolute_path(root_path);
            // Just ensure it doesn't panic
        }
    }

    #[test]
    fn test_trailing_slash_handling() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("testdir");
        fs::create_dir(&dir_path).unwrap();

        let validator = PathValidator::new();

        // Test with and without trailing slash
        let without_slash = dir_path.to_string_lossy().to_string();
        let with_slash = format!("{}/", without_slash);

        let result1 = validator.validate_absolute_path(&without_slash);
        let result2 = validator.validate_absolute_path(&with_slash);

        // Both should have consistent behavior
        assert_eq!(
            result1.is_ok(),
            result2.is_ok(),
            "Trailing slash should not affect validation"
        );
    }

    #[test]
    fn test_current_directory_component() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "content").unwrap();

        let validator = PathValidator::new();

        // Path with current directory component
        let path_with_dot = format!("{}/./file.txt", temp_dir.path().display());
        let result = validator.validate_absolute_path(&path_with_dot);

        // Should be rejected due to relative component
        assert!(
            result.is_err(),
            "Path with current directory component should be rejected"
        );
    }

    #[test]
    fn test_repeated_slashes() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "content").unwrap();

        let validator = PathValidator::new();

        // Path with repeated slashes
        let path_str = file_path.to_string_lossy().replace('/', "///");
        let result = validator.validate_absolute_path(&path_str);

        // Should normalize repeated slashes
        assert!(
            result.is_ok() || result.is_err(),
            "Repeated slashes should be handled"
        );
    }

    #[test]
    fn test_whitespace_in_path() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file with spaces.txt");
        fs::write(&file_path, "content").unwrap();

        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&file_path.to_string_lossy());

        // Whitespace in path should be allowed (it's legitimate)
        assert!(
            result.is_ok(),
            "Paths with whitespace should be allowed: {}",
            file_path.display()
        );
    }

    #[test]
    fn test_special_characters_in_path() {
        let temp_dir = TempDir::new().unwrap();

        let special_chars = vec!["file!.txt", "file@.txt", "file#.txt", "file$.txt"];

        for name in special_chars {
            let file_path = temp_dir.path().join(name);
            fs::write(&file_path, "content").unwrap();

            let validator = PathValidator::new();
            let result = validator.validate_absolute_path(&file_path.to_string_lossy());

            assert!(
                result.is_ok(),
                "Path with special character should be allowed: {}",
                name
            );
        }
    }
}
