//! Single entry point for all swissarmyhammer-views server integration tests.
//!
//! Each test file under `tests/integration/` becomes a submodule here. Cargo
//! treats this file as one integration target so the test binary compiles in
//! one pass.

mod integration;
