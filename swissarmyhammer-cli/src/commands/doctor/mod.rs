//! Doctor command implementation
//! 
//! Diagnoses configuration and setup issues for swissarmyhammer

use crate::doctor::Doctor;
use crate::exit_codes::EXIT_ERROR;

/// Help text for the doctor command
pub const DESCRIPTION: &str = include_str!("description.md");



/// Handle the doctor command
pub async fn handle_command(migration: bool) -> i32 {
    let mut doctor = Doctor::new();

    match doctor.run_diagnostics_with_options(migration) {
        Ok(exit_code) => exit_code,
        Err(e) => {
            eprintln!("Doctor command failed: {}", e);
            EXIT_ERROR
        }
    }
}