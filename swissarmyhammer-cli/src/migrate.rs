use crate::cli::MigrateCommands;
use std::io::{self, Write};
use swissarmyhammer::issues::filesystem::{
    FileSystemIssueStorage, MigrationResult,
};

pub async fn handle_migrate_command(
    command: MigrateCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        MigrateCommands::Status => {
            handle_migrate_status().await?;
        }
        MigrateCommands::Run {
            force,
            backup,
            dry_run,
        } => {
            handle_migrate_run(force, backup, dry_run).await?;
        }
        MigrateCommands::Check => {
            handle_migrate_check().await?;
        }
        MigrateCommands::Cleanup => {
            handle_migrate_cleanup().await?;
        }
    }
    Ok(())
}

async fn handle_migrate_status() -> Result<(), Box<dyn std::error::Error>> {
    let info = FileSystemIssueStorage::migration_info()?;

    println!("📊 Migration Status");
    println!();

    if info.should_migrate {
        println!("✅ Migration needed");
        println!(
            "   Source: ./issues/ ({} files, {:.1} KB)",
            info.file_count,
            info.total_size as f64 / 1024.0
        );
        println!("   Target: .swissarmyhammer/issues/");
        println!();
        println!("Run 'swissarmyhammer migrate run' to perform migration");
    } else if info.source_exists && info.destination_exists {
        println!("⚠️  Both directories exist");
        println!("   Legacy: ./issues/");
        println!("   Current: .swissarmyhammer/issues/");
        println!();
        println!("Manual intervention may be required");
    } else if info.destination_exists {
        println!("✅ Already using .swissarmyhammer/issues/");
    } else {
        println!("ℹ️  No issues directory found");
        println!("   Will create .swissarmyhammer/issues/ when needed");
    }

    Ok(())
}

async fn handle_migrate_run(
    force: bool,
    backup: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let info = FileSystemIssueStorage::migration_info()?;

    if !info.should_migrate {
        println!("No migration needed");
        return Ok(());
    }

    println!("🚀 Migration Plan");
    println!("   Source: ./issues/");
    println!("   Target: .swissarmyhammer/issues/");
    println!("   Files: {}", info.file_count);
    println!("   Size: {:.1} KB", info.total_size as f64 / 1024.0);
    if backup {
        println!("   Backup: Yes");
    }
    println!();

    if dry_run {
        println!("🧪 Dry Run - No files will be moved");
        return Ok(());
    }

    if !force {
        println!("Proceed with migration? (y/N)");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().to_lowercase().starts_with('y') {
            println!("Migration cancelled");
            return Ok(());
        }
    }

    println!("🔄 Starting migration...");

    let result = if backup {
        perform_migration_with_backup().await?
    } else {
        FileSystemIssueStorage::perform_migration()?
    };

    match result {
        MigrationResult::Success(stats) => {
            println!("✅ Migration completed successfully!");
            println!("   Files moved: {}", stats.files_moved);
            println!(
                "   Data transferred: {:.1} KB",
                stats.bytes_moved as f64 / 1024.0
            );
            println!("   Duration: {:.2}s", stats.duration.as_secs_f64());
        }
        MigrationResult::NotNeeded(_) => {
            println!("ℹ️  No migration was needed");
        }
    }

    Ok(())
}

async fn handle_migrate_check() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Checking migration prerequisites...");

    let info = FileSystemIssueStorage::migration_info()?;
    let paths = FileSystemIssueStorage::migration_paths()?;

    // Check source directory
    if info.source_exists {
        println!("✅ Source directory exists: {}", paths.source.display());
        println!("   Files: {}", info.file_count);
        println!("   Size: {:.1} KB", info.total_size as f64 / 1024.0);
    } else {
        println!(
            "❌ Source directory does not exist: {}",
            paths.source.display()
        );
    }

    // Check destination
    if info.destination_exists {
        println!(
            "⚠️  Destination already exists: {}",
            paths.destination.display()
        );
    } else {
        println!(
            "✅ Destination available: {}",
            paths.destination.display()
        );
    }

    // Check parent directory permissions
    if let Some(parent) = paths.destination.parent() {
        if parent.exists() {
            match std::fs::metadata(parent) {
                Ok(metadata) => {
                    if metadata.permissions().readonly() {
                        println!(
                            "❌ Parent directory is read-only: {}",
                            parent.display()
                        );
                    } else {
                        println!(
                            "✅ Parent directory is writable: {}",
                            parent.display()
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "⚠️  Cannot check parent directory permissions: {}",
                        e
                    );
                }
            }
        } else {
            println!(
                "ℹ️  Parent directory will be created: {}",
                parent.display()
            );
        }
    }

    // Overall assessment
    if info.should_migrate {
        println!();
        println!("🎯 Migration is recommended and should succeed");
    } else {
        println!();
        println!("ℹ️  No migration needed at this time");
    }

    Ok(())
}

async fn handle_migrate_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧹 Cleaning up migration artifacts...");

    let current_dir = std::env::current_dir()?;
    let swissarmyhammer_dir = current_dir.join(".swissarmyhammer");

    // Look for backup directories
    let mut backup_count = 0;
    let mut backup_size = 0u64;

    if swissarmyhammer_dir.exists() {
        for entry in std::fs::read_dir(&swissarmyhammer_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("issues_backup_") {
                        let size = calculate_directory_size(&path)?;
                        backup_size += size;
                        backup_count += 1;

                        println!("Found backup: {}", name);
                    }
                }
            }
        }
    }

    if backup_count == 0 {
        println!("No migration artifacts found");
        return Ok(());
    }

    println!();
    println!(
        "Found {} backup(s) using {:.1} KB",
        backup_count,
        backup_size as f64 / 1024.0
    );
    println!("Remove backups? (y/N)");

    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if !input.trim().to_lowercase().starts_with('y') {
        println!("Cleanup cancelled");
        return Ok(());
    }

    // Remove backups
    let mut removed_count = 0;
    for entry in std::fs::read_dir(&swissarmyhammer_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("issues_backup_") {
                    std::fs::remove_dir_all(&path)?;
                    removed_count += 1;
                    println!("Removed: {}", name);
                }
            }
        }
    }

    println!("✅ Removed {} backup(s)", removed_count);
    Ok(())
}

// Helper function for backup creation during migration
async fn perform_migration_with_backup() -> Result<MigrationResult, Box<dyn std::error::Error>> {
    let paths = FileSystemIssueStorage::migration_paths()?;
    
    // Create backup first
    println!("📦 Creating backup...");
    let current_dir = std::env::current_dir()?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let backup_dir = current_dir.join(".swissarmyhammer").join(format!("issues_backup_{}", timestamp));
    
    if paths.source.exists() {
        copy_directory(&paths.source, &backup_dir)?;
        println!("✅ Backup created: {}", backup_dir.display());
    }
    
    // Now perform the migration
    match FileSystemIssueStorage::perform_migration() {
        Ok(result) => Ok(result),
        Err(e) => {
            // If migration fails, the backup is still available for manual recovery
            println!("⚠️  Migration failed, but backup is available at: {}", backup_dir.display());
            Err(e.into())
        }
    }
}

// Helper function to copy a directory recursively
fn copy_directory(src: &std::path::Path, dst: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    if !src.exists() {
        return Ok(());
    }
    
    std::fs::create_dir_all(dst)?;
    
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if src_path.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        } else if src_path.is_dir() {
            copy_directory(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}

// Helper function to calculate directory size
fn calculate_directory_size(path: &std::path::Path) -> Result<u64, Box<dyn std::error::Error>> {
    let mut total_size = 0;

    if path.is_file() {
        return Ok(std::fs::metadata(path)?.len());
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_file() {
            total_size += std::fs::metadata(&entry_path)?.len();
        } else if entry_path.is_dir() {
            total_size += calculate_directory_size(&entry_path)?;
        }
    }

    Ok(total_size)
}