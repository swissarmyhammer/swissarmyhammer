//! Seeded sample crate for the local-model review verification harness.
//!
//! Every module contains *planted, intentional* findings — duplicated
//! function bodies and repeated bare numeric literals — so that a working
//! review run over this crate must report findings. Do not "fix" them.

pub mod invoices;
pub mod orders;
