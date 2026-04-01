//! UpdatePerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::add::perspective_to_json;
use crate::perspective::{PerspectiveFieldEntry, SortEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

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
    /// New view type
    pub view: Option<String>,
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

    /// Set the new view type.
    pub fn with_view(mut self, view: impl Into<String>) -> Self {
        self.view = Some(view.into());
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
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

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

            let previous = existing.clone();

            // If renaming, reject if the new name is taken by a different perspective
            if let Some(new_name) = &self.name {
                if *new_name != existing.name {
                    if let Some(other) = pctx.get_by_name(new_name) {
                        if other.id != self.id {
                            return Err(KanbanError::duplicate_name("perspective", new_name));
                        }
                    }
                }
            }

            // Merge only provided fields
            let updated = crate::perspective::Perspective {
                id: existing.id,
                name: self.name.clone().unwrap_or(existing.name),
                view: self.view.clone().unwrap_or(existing.view),
                fields: self.fields.clone().unwrap_or(existing.fields),
                filter: match &self.filter {
                    Some(f) => f.clone(),
                    None => existing.filter,
                },
                group: match &self.group {
                    Some(g) => g.clone(),
                    None => existing.group,
                },
                sort: self.sort.clone().unwrap_or(existing.sort),
            };

            pctx.write(&updated).await?;

            // Log to changelog
            if let Err(e) = ctx
                .perspective_changelog()
                .log_update(&self.id, &previous, &updated)
                .await
            {
                tracing::warn!(%e, "failed to log perspective update");
            }

            Ok(perspective_to_json(&updated))
        }
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(value) => ExecutionResult::Logged {
                value: value.clone(),
                log_entry: LogEntry::new(self.op_string(), input, value, None, duration_ms),
            },
            Err(error) => {
                let error_msg = error.to_string();
                ExecutionResult::Failed {
                    error,
                    log_entry: Some(LogEntry::new(
                        self.op_string(),
                        input,
                        serde_json::json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
    }
}
