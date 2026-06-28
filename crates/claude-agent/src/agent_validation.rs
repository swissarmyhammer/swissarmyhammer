//! Protocol-version negotiation for the ACP `initialize` handler.
//!
//! Per the ACP specification, `initialize` *negotiates* the protocol version —
//! it never hard-fails on a version mismatch. If the client requests a version
//! this agent does not support, the agent answers with its own latest
//! supported version and the client decides whether to proceed. There is no
//! request-body validation beyond this: `initialize` is a light, non-fatal
//! handshake, and `llama-agent` follows the identical convention.

use agent_client_protocol::schema::ProtocolVersion;

impl crate::agent::ClaudeAgent {
    /// Protocol versions this agent supports.
    pub(crate) const SUPPORTED_PROTOCOL_VERSIONS: &'static [ProtocolVersion] =
        &[ProtocolVersion::V0, ProtocolVersion::V1];

    /// Negotiate the protocol version for a session.
    ///
    /// Returns the client's requested version if it is supported; otherwise
    /// returns the agent's latest supported version. This never fails — a
    /// version mismatch is resolved by negotiation, not by an error.
    ///
    /// Defined as an associated function (no `&self`): negotiation depends only
    /// on the client's requested version and the static
    /// [`Self::SUPPORTED_PROTOCOL_VERSIONS`] list, never on instance state.
    /// `llama-agent` carries the identical `pub(crate)` associated-function
    /// signature so the "one convention" claim holds at the signature level.
    ///
    /// # Arguments
    /// * `client_requested_version` - The protocol version requested by the client.
    ///
    /// # Returns
    /// The negotiated protocol version to use for the session.
    pub(crate) fn negotiate_protocol_version(
        client_requested_version: &ProtocolVersion,
    ) -> ProtocolVersion {
        if Self::SUPPORTED_PROTOCOL_VERSIONS.contains(client_requested_version) {
            *client_requested_version
        } else {
            Self::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .max()
                .copied()
                .unwrap_or(ProtocolVersion::V1)
        }
    }
}
