//! Plan command implementation
//! 
//! Executes planning workflow for specific specification files

use crate::plan;

/// Help text for the plan command
pub const DESCRIPTION: &str = include_str!("description.md");



/// Handle the plan command
pub async fn handle_command(plan_filename: String) -> i32 {
    plan::run_plan(plan_filename).await
}