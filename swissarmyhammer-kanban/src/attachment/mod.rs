//! Attachment commands

mod add;
mod delete;
mod get;
mod list;
mod update;

pub(crate) use add::extract_stored_filenames;
pub use add::AddAttachment;
pub use delete::DeleteAttachment;
pub use get::GetAttachment;
pub use list::ListAttachments;
pub use update::UpdateAttachment;
