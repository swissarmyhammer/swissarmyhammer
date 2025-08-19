# Migration Validation and Verification System

## Overview
Implement comprehensive validation and verification system to ensure migration completed successfully and provide detailed diagnostics for troubleshooting migration issues.

Refer to /Users/wballard/github/sah-issues/ideas/move_issues.md

## Current State
After steps 000290-000292, we have migration functionality but need robust validation and verification capabilities.

## Target Implementation

### Comprehensive Migration Validation
```rust
#[derive(Debug)]
pub struct MigrationValidation {
    pub file_integrity: FileIntegrityCheck,
    pub directory_structure: DirectoryStructureCheck,
    pub content_verification: ContentVerificationCheck,
    pub metadata_preservation: MetadataPreservationCheck,
}

#[derive(Debug)]
pub struct FileIntegrityCheck {
    pub total_files: usize,
    pub verified_files: usize,
    pub missing_files: Vec<PathBuf>,
    pub extra_files: Vec<PathBuf>,
    pub corrupted_files: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct DirectoryStructureCheck {
    pub directories_preserved: bool,
    pub relative_paths_correct: bool,
    pub permissions_preserved: bool,
    pub structure_differences: Vec<String>,
}

impl FileSystemIssueStorage {
    /// Comprehensive validation of completed migration
    pub fn validate_migration_comprehensive(
        source_backup: &Path,
        destination: &Path,
    ) -> Result<MigrationValidation> {
        let file_integrity = Self::validate_file_integrity(source_backup, destination)?;
        let directory_structure = Self::validate_directory_structure(source_backup, destination)?;
        let content_verification = Self::validate_content_integrity(source_backup, destination)?;
        let metadata_preservation = Self::validate_metadata_preservation(source_backup, destination)?;
        
        Ok(MigrationValidation {
            file_integrity,
            directory_structure,
            content_verification,
            metadata_preservation,
        })
    }
    
    /// Validate file integrity (count, names, sizes)
    fn validate_file_integrity(
        source: &Path,
        destination: &Path,
    ) -> Result<FileIntegrityCheck> {
        let source_files = Self::collect_file_list(source)?;
        let dest_files = Self::collect_file_list(destination)?;
        
        let missing_files: Vec<PathBuf> = source_files
            .difference(&dest_files)
            .cloned()
            .collect();
            
        let extra_files: Vec<PathBuf> = dest_files
            .difference(&source_files)
            .cloned()
            .collect();
        
        let mut corrupted_files = Vec::new();
        
        // Check file sizes for files that exist in both
        for file_path in source_files.intersection(&dest_files) {
            let source_file = source.join(file_path);
            let dest_file = destination.join(file_path);
            
            let source_size = std::fs::metadata(&source_file)?.len();
            let dest_size = std::fs::metadata(&dest_file)?.len();
            
            if source_size != dest_size {
                corrupted_files.push(file_path.clone());
            }
        }
        
        Ok(FileIntegrityCheck {
            total_files: source_files.len(),
            verified_files: source_files.len() - missing_files.len() - corrupted_files.len(),
            missing_files,
            extra_files,
            corrupted_files,
        })
    }
    
    /// Collect recursive file list with relative paths
    fn collect_file_list(root: &Path) -> Result<std::collections::HashSet<PathBuf>> {
        let mut files = std::collections::HashSet::new();
        
        fn visit_dir(
            dir: &Path,
            root: &Path,
            files: &mut std::collections::HashSet<PathBuf>,
        ) -> Result<()> {
            for entry in std::fs::read_dir(dir).map_err(SwissArmyHammerError::Io)? {
                let entry = entry.map_err(SwissArmyHammerError::Io)?;
                let path = entry.path();
                
                if path.is_dir() {
                    visit_dir(&path, root, files)?;
                } else if path.is_file() {
                    if let Ok(relative_path) = path.strip_prefix(root) {
                        files.insert(relative_path.to_path_buf());
                    }
                }
            }
            Ok(())
        }
        
        if root.exists() {
            visit_dir(root, root, &mut files)?;
        }
        
        Ok(files)
    }
}
```

### Content Verification System
```rust
impl FileSystemIssueStorage {
    /// Verify content integrity using checksums
    fn validate_content_integrity(
        source: &Path,
        destination: &Path,
    ) -> Result<ContentVerificationCheck> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let source_files = Self::collect_file_list(source)?;
        let mut verified_count = 0;
        let mut mismatched_files = Vec::new();
        let mut unreadable_files = Vec::new();
        
        for file_path in &source_files {
            let source_file = source.join(file_path);
            let dest_file = destination.join(file_path);
            
            if !dest_file.exists() {
                continue; // Already caught by file integrity check
            }
            
            match Self::compare_file_content(&source_file, &dest_file) {
                Ok(true) => verified_count += 1,
                Ok(false) => mismatched_files.push(file_path.clone()),
                Err(_) => unreadable_files.push(file_path.clone()),
            }
        }
        
        Ok(ContentVerificationCheck {
            total_files: source_files.len(),
            verified_files: verified_count,
            mismatched_files,
            unreadable_files,
        })
    }
    
    /// Compare file content using efficient hashing
    fn compare_file_content(source: &Path, destination: &Path) -> Result<bool> {
        // For small files, use full content comparison
        let source_size = std::fs::metadata(source)?.len();
        if source_size < 1024 * 1024 { // 1MB
            let source_content = std::fs::read(source)?;
            let dest_content = std::fs::read(destination)?;
            return Ok(source_content == dest_content);
        }
        
        // For large files, use hash comparison
        let source_hash = Self::calculate_file_hash(source)?;
        let dest_hash = Self::calculate_file_hash(destination)?;
        Ok(source_hash == dest_hash)
    }
    
    /// Calculate hash of file content
    fn calculate_file_hash(path: &Path) -> Result<u64> {
        use std::io::Read;
        
        let mut file = std::fs::File::open(path)?;
        let mut hasher = DefaultHasher::new();
        let mut buffer = [0; 8192];
        
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.write(&buffer[..bytes_read]);
        }
        
        Ok(hasher.finish())
    }
}

#[derive(Debug)]
pub struct ContentVerificationCheck {
    pub total_files: usize,
    pub verified_files: usize,
    pub mismatched_files: Vec<PathBuf>,
    pub unreadable_files: Vec<PathBuf>,
}
```

### Migration Diagnostics and Reporting
```rust
impl MigrationValidation {
    /// Generate comprehensive migration report
    pub fn generate_report(&self) -> MigrationReport {
        let overall_success = self.is_successful();
        let mut issues = Vec::new();
        let mut warnings = Vec::new();
        
        // File integrity issues
        if !self.file_integrity.missing_files.is_empty() {
            issues.push(format!(
                "Missing files: {}",
                self.file_integrity.missing_files.len()
            ));
        }
        
        if !self.file_integrity.corrupted_files.is_empty() {
            issues.push(format!(
                "Corrupted files: {}",
                self.file_integrity.corrupted_files.len()
            ));
        }
        
        if !self.file_integrity.extra_files.is_empty() {
            warnings.push(format!(
                "Extra files found: {}",
                self.file_integrity.extra_files.len()
            ));
        }
        
        // Content verification issues
        if !self.content_verification.mismatched_files.is_empty() {
            issues.push(format!(
                "Content mismatch: {}",
                self.content_verification.mismatched_files.len()
            ));
        }
        
        // Directory structure issues
        if !self.directory_structure.directories_preserved {
            issues.push("Directory structure not preserved".to_string());
        }
        
        MigrationReport {
            overall_success,
            total_files: self.file_integrity.total_files,
            verified_files: self.content_verification.verified_files,
            issues,
            warnings,
            detailed_validation: self.clone(),
        }
    }
    
    /// Check if migration validation passed all critical checks
    pub fn is_successful(&self) -> bool {
        self.file_integrity.missing_files.is_empty()
            && self.file_integrity.corrupted_files.is_empty()
            && self.content_verification.mismatched_files.is_empty()
            && self.directory_structure.directories_preserved
    }
}

#[derive(Debug)]
pub struct MigrationReport {
    pub overall_success: bool,
    pub total_files: usize,
    pub verified_files: usize,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub detailed_validation: MigrationValidation,
}
```

### Integration with Migration Process
```rust
impl FileSystemIssueStorage {
    /// Enhanced migration with validation
    pub fn perform_migration_with_validation() -> Result<MigrationResult> {
        let info = Self::migration_info()?;
        
        if !info.should_migrate {
            return Ok(MigrationResult::NotNeeded(info));
        }
        
        let paths = Self::migration_paths()?;
        
        // Create backup for validation
        let backup_path = Self::create_backup(&paths.source)?;
        
        // Perform migration
        match Self::execute_migration(&paths) {
            Ok(stats) => {
                // Validate migration
                let validation = Self::validate_migration_comprehensive(&backup_path, &paths.destination)?;
                let report = validation.generate_report();
                
                if report.overall_success {
                    tracing::info!("Migration completed and validated successfully");
                    Ok(MigrationResult::Success(stats))
                } else {
                    tracing::error!("Migration validation failed: {:?}", report.issues);
                    // Rollback
                    Self::rollback_migration(&paths, &backup_path)?;
                    Err(SwissArmyHammerError::validation_failed(&format!(
                        "Migration validation failed: {}",
                        report.issues.join(", ")
                    )))
                }
            }
            Err(e) => {
                tracing::error!("Migration failed, rolling back: {}", e);
                Self::rollback_migration(&paths, &backup_path)?;
                Err(e)
            }
        }
    }
}
```

### CLI Integration for Validation
```rust
// Add to CLI migrate commands
pub async fn handle_migrate_verify() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Verifying migration integrity...");
    
    let current_dir = std::env::current_dir()?;
    let destination = current_dir.join(".swissarmyhammer/issues");
    
    if !destination.exists() {
        println!("❌ No migrated issues directory found");
        return Ok(());
    }
    
    // Look for backup to compare against
    let swissarmyhammer_dir = current_dir.join(".swissarmyhammer");
    let mut backup_path = None;
    
    if swissarmyhammer_dir.exists() {
        for entry in std::fs::read_dir(&swissarmyhammer_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("issues_backup_") {
                        backup_path = Some(path);
                        break;
                    }
                }
            }
        }
    }
    
    let backup = backup_path.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No backup found for verification"
        )
    })?;
    
    let validation = swissarmyhammer::issues::FileSystemIssueStorage::validate_migration_comprehensive(
        &backup,
        &destination,
    )?;
    
    let report = validation.generate_report();
    
    // Display results
    println!("📊 Verification Results");
    println!("   Files verified: {}/{}", report.verified_files, report.total_files);
    
    if report.overall_success {
        println!("✅ Migration verification passed");
    } else {
        println!("❌ Migration verification failed");
        for issue in &report.issues {
            println!("   🔴 {}", issue);
        }
    }
    
    if !report.warnings.is_empty() {
        for warning in &report.warnings {
            println!("   ⚠️  {}", warning);
        }
    }
    
    Ok(())
}
```

## Testing Requirements

### Validation Logic Tests
```rust
#[cfg(test)]
mod validation_tests {
    #[test]
    fn test_file_integrity_validation() {
        // Test file counting and integrity checks
    }
    
    #[test]
    fn test_content_verification() {
        // Test content comparison and hashing
    }
    
    #[test]
    fn test_directory_structure_validation() {
        // Test directory structure preservation
    }
    
    #[test]
    fn test_validation_with_corrupted_files() {
        // Test detection of corrupted files
    }
    
    #[test]
    fn test_validation_with_missing_files() {
        // Test detection of missing files
    }
}
```

### Integration Tests
- Test validation with various directory structures
- Test validation with large numbers of files
- Test validation performance with large files
- Test validation error scenarios

### CLI Verification Tests
- Test CLI verification command
- Test verification reporting
- Test verification with missing backups
- Test verification error handling

## Files to Modify
- `swissarmyhammer/src/issues/filesystem.rs`
- Add validation modules and types
- Enhance CLI migrate commands
- Comprehensive test suite
- Update documentation

## Acceptance Criteria
- [ ] Comprehensive validation of file integrity after migration
- [ ] Content verification using hashing for large files
- [ ] Directory structure preservation validation
- [ ] Detailed reporting of validation results
- [ ] CLI integration for verification commands
- [ ] Performance acceptable for typical issue directories
- [ ] Detection of all common migration issues
- [ ] Clear diagnostics for troubleshooting problems

## Dependencies
- Depends on steps 000290-000292 (migration implementation)
- Can be done in parallel with step 000294 (error handling)

## Estimated Effort
~500-600 lines including validation logic, reporting, and comprehensive testing.

## Notes
- Focus on detecting real-world migration issues
- Balance thoroughness with performance
- Provide actionable diagnostics for fixing issues
- Consider cross-platform filesystem differences
- Design for both automatic and manual validation