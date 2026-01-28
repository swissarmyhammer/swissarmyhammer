//! Shared diagnostic infrastructure for SwissArmyHammer tools
//!
//! This crate provides common types and utilities for implementing
//! `doctor` commands across SwissArmyHammer tools like `sah` and `avp`.
//!
//! # Example
//!
//! ```
//! use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner, print_checks_table};
//!
//! struct MyDoctor {
//!     checks: Vec<Check>,
//! }
//!
//! impl DoctorRunner for MyDoctor {
//!     fn checks(&self) -> &[Check] {
//!         &self.checks
//!     }
//!
//!     fn checks_mut(&mut self) -> &mut Vec<Check> {
//!         &mut self.checks
//!     }
//! }
//!
//! let mut doctor = MyDoctor { checks: Vec::new() };
//! doctor.add_check(Check {
//!     name: "Test Check".to_string(),
//!     status: CheckStatus::Ok,
//!     message: "Everything is working".to_string(),
//!     fix: None,
//! });
//!
//! assert_eq!(doctor.get_exit_code(), 0);
//! ```

mod display;
mod runner;
mod table;
mod types;

pub use display::{categorize_check, format_check_status, CheckResult, VerboseCheckResult};
pub use runner::DoctorRunner;
pub use table::print_checks_table;
pub use types::{Check, CheckStatus, ExitCode};
