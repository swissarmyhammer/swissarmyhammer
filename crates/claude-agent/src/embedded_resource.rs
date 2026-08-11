//! One decision about what payload an ACP embedded resource carries.
//!
//! An [`EmbeddedResource`] holds either text contents or blob contents, and a
//! client is free to send a third kind this build does not know. Two modules
//! must answer that question and must answer it the same way:
//! [`crate::content_security_validator`] checks the payload, and
//! [`crate::content_block_processor`] turns it into text a model can read. If
//! the two disagreed, a kind one of them refuses would still be processed by
//! the other.
//!
//! [`dispatch`] is that one answer. A caller supplies what it does with text
//! contents and what it does with blob contents. The refusal is
//! [`UnsupportedResourceKind`], which each caller's own error type accepts
//! through [`From`], so the decision lives here once and the wording lives with
//! the error that carries it.

use agent_client_protocol::schema::{
    BlobResourceContents, EmbeddedResource, EmbeddedResourceResource, TextResourceContents,
};

/// An embedded resource carries a payload kind this build cannot serve.
///
/// It names the refusal without naming a message, because the message belongs
/// to whichever error type the caller returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedResourceKind;

/// Send an embedded resource to the handler for the payload it carries.
///
/// # Errors
///
/// Returns whatever the chosen handler returns, or the caller's rendering of
/// [`UnsupportedResourceKind`] for a payload kind this build does not know.
pub(crate) fn dispatch<T, E>(
    resource: &EmbeddedResource,
    on_text: impl FnOnce(&TextResourceContents) -> Result<T, E>,
    on_blob: impl FnOnce(&BlobResourceContents) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<UnsupportedResourceKind>,
{
    match &resource.resource {
        EmbeddedResourceResource::TextResourceContents(text_contents) => on_text(text_contents),
        EmbeddedResourceResource::BlobResourceContents(blob_contents) => on_blob(blob_contents),
        _ => Err(UnsupportedResourceKind.into()),
    }
}
