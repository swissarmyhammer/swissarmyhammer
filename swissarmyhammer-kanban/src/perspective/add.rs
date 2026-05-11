//! AddPerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::{Perspective, PerspectiveFieldEntry, SortEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Add a new perspective to the board.
///
/// Creates a named view configuration with optional fields, filter, group, and
/// sort settings. A ULID is generated automatically for the perspective's stable
/// identity. The perspective is persisted as a YAML file and logged to the
/// perspective changelog.
#[operation(
    verb = "add",
    noun = "perspective",
    description = "Add a new perspective to the board"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddPerspective {
    /// Human-readable name (e.g. "Active Sprint")
    pub name: String,
    /// View type (e.g. "board", "grid")
    pub view: String,
    /// Ordered list of field entries (defines column order)
    #[serde(default)]
    pub fields: Vec<PerspectiveFieldEntry>,
    /// Opaque filter function string
    pub filter: Option<String>,
    /// Opaque group function string
    pub group: Option<String>,
    /// Sort entries, applied in order
    #[serde(default)]
    pub sort: Vec<SortEntry>,
}

impl AddPerspective {
    /// Create a new AddPerspective with the required name and view.
    pub fn new(name: impl Into<String>, view: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            view: view.into(),
            fields: Vec::new(),
            filter: None,
            group: None,
            sort: Vec::new(),
        }
    }

    /// Set the fields list.
    pub fn with_fields(mut self, fields: Vec<PerspectiveFieldEntry>) -> Self {
        self.fields = fields;
        self
    }

    /// Set the filter expression.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Set the group expression.
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Set the sort entries.
    pub fn with_sort(mut self, sort: Vec<SortEntry>) -> Self {
        self.sort = sort;
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddPerspective {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: std::result::Result<Value, KanbanError> = async {
            let pctx = ctx.perspective_context().await?;
            let mut pctx = pctx.write().await;

            let id = ulid::Ulid::new().to_string();
            let mut perspective = Perspective::new(id, self.name.clone(), self.view.clone())
                .with_fields(self.fields.clone())
                .with_sort(self.sort.clone());
            perspective.filter = self.filter.clone();
            perspective.group = self.group.clone();

            pctx.write(&perspective).await?;

            Ok(perspective_to_json(&perspective))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}

/// Convert a Perspective to the API JSON format.
pub(crate) fn perspective_to_json(p: &Perspective) -> Value {
    serde_json::to_value(p).expect("Perspective serializes to JSON")
}
