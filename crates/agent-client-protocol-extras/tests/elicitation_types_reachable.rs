//! Compile-level proof that the ACP elicitation types are reachable.
//!
//! The elicitation types (`CreateElicitationRequest`, `CreateElicitationResponse`,
//! `ElicitationAction`) live in the `agent-client-protocol-schema` crate behind its
//! `unstable_elicitation` feature and are re-exported through the main
//! `agent-client-protocol` crate's `schema` module. The workspace enables that
//! feature unconditionally by depending on the schema crate directly with
//! `features = ["unstable_elicitation"]`; Cargo feature unification then turns the
//! types on everywhere the main crate's `schema` re-export is visible.
//!
//! This test exists to fail the build the moment that wiring regresses: if the
//! feature is dropped or the re-export path changes, these references stop
//! resolving and the workspace will not compile.

use agent_client_protocol::schema::{
    CreateElicitationRequest, CreateElicitationResponse, ElicitationAction,
};

/// Constructs the elicitation request/response types and matches on every
/// `ElicitationAction` variant, proving the types are reachable and usable
/// through the `agent_client_protocol::schema` re-export.
#[test]
fn elicitation_types_are_reachable_through_schema_reexport() {
    // Build a response around each terminal action. The unit variants
    // (`Decline`, `Cancel`) keep the construction minimal while still
    // exercising the public `new` constructor and the enum surface.
    let declined = CreateElicitationResponse::new(ElicitationAction::Decline);
    let cancelled = CreateElicitationResponse::new(ElicitationAction::Cancel);

    // Match on the action enum so a variant rename in a future schema bump is
    // caught here rather than at a downstream call site. `ElicitationAction` is
    // `#[non_exhaustive]`, so a wildcard arm is required even though we only
    // constructed the terminal variants.
    for response in [declined, cancelled] {
        match response.action {
            ElicitationAction::Accept(_) => unreachable!("constructed Decline/Cancel only"),
            ElicitationAction::Decline | ElicitationAction::Cancel => {}
            _ => unreachable!("only Accept/Decline/Cancel exist in this schema version"),
        }
    }

    // Name the request type in a value position so it is part of the compiled
    // surface, not just an unused import. `size_of` forces the type to be fully
    // resolved without needing to build the (non-trivial) `ElicitationMode`.
    assert!(std::mem::size_of::<CreateElicitationRequest>() > 0);
}
