//! UpdatePerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::add::perspective_to_json;
use crate::perspective::{PerspectiveFieldEntry, SortEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Update a perspective's properties.
///
/// This is a partial update -- only provided fields are changed. Unspecified
/// fields are preserved from the existing perspective.
#[operation(
    verb = "update",
    noun = "perspective",
    description = "Update a perspective's properties"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePerspective {
    /// The perspective ID (ULID) to update
    pub id: String,
    /// New name
    pub name: Option<String>,
    /// New view kind
    pub view: Option<String>,
    /// New view instance id (`Some(Some(id))` pins it, `Some(None)` clears it
    /// back to legacy shared-by-kind). Mirrors the `filter`/`group` tri-state
    /// shape: outer `None` means "do not touch".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<Option<String>>,
    /// New fields list (replaces entire list)
    pub fields: Option<Vec<PerspectiveFieldEntry>>,
    /// New filter expression (Some(None) clears it)
    pub filter: Option<Option<String>>,
    /// New group expression (Some(None) clears it)
    pub group: Option<Option<String>>,
    /// New sort entries (replaces entire list)
    pub sort: Option<Vec<SortEntry>>,
}

impl UpdatePerspective {
    /// Create a new UpdatePerspective targeting the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            view: None,
            view_id: None,
            fields: None,
            filter: None,
            group: None,
            sort: None,
        }
    }

    /// Set the new name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the new view kind.
    pub fn with_view(mut self, view: impl Into<String>) -> Self {
        self.view = Some(view.into());
        self
    }

    /// Set the new view instance id.
    ///
    /// Pass `Some(id)` to pin the perspective to that view instance, or
    /// `None` to clear it back to legacy shared-by-kind.
    pub fn with_view_id(mut self, view_id: Option<String>) -> Self {
        self.view_id = Some(view_id);
        self
    }

    /// Set the new fields list.
    pub fn with_fields(mut self, fields: Vec<PerspectiveFieldEntry>) -> Self {
        self.fields = Some(fields);
        self
    }

    /// Set the new filter expression.
    pub fn with_filter(mut self, filter: Option<String>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Set the new group expression.
    pub fn with_group(mut self, group: Option<String>) -> Self {
        self.group = Some(group);
        self
    }

    /// Set the new sort entries.
    pub fn with_sort(mut self, sort: Vec<SortEntry>) -> Self {
        self.sort = Some(sort);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdatePerspective {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: std::result::Result<Value, KanbanError> = async {
            let pctx = ctx.perspective_context().await?;
            let mut pctx = pctx.write().await;

            // Read existing perspective
            let existing = pctx
                .get_by_id(&self.id)
                .ok_or_else(|| KanbanError::NotFound {
                    resource: "perspective".to_string(),
                    id: self.id.clone(),
                })?
                .clone();

            // Merge only provided fields
            let mut updated = crate::perspective::Perspective::new(
                existing.id,
                self.name.clone().unwrap_or(existing.name),
                self.view.clone().unwrap_or(existing.view),
            )
            .with_fields(self.fields.clone().unwrap_or(existing.fields))
            .with_sort(self.sort.clone().unwrap_or(existing.sort));
            updated.view_id = match &self.view_id {
                Some(v) => v.clone(),
                None => existing.view_id,
            };
            updated.filter = match &self.filter {
                Some(f) => f.clone(),
                None => existing.filter,
            };
            updated.group = match &self.group {
                Some(g) => g.clone(),
                None => existing.group,
            };

            // Opt-in legacy migration: when the perspective still lacks a
            // `view_id` and the workspace has exactly one matching view, pin
            // it before persisting. See `perspective::migrate` for the
            // save-vs-load placement rationale.
            if let Some(views_lock) = ctx.views() {
                let views = views_lock.read().await;
                crate::perspective::migrate::maybe_pin_view_id_on_save(&mut updated, &views);
            }

            pctx.write(&updated).await?;

            Ok(perspective_to_json(&updated))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}
