//! AddPerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::{Perspective, PerspectiveFieldEntry, SortEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

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
    /// View kind (e.g. "board", "grid"). Retained alongside `view_id` for
    /// backwards compat with legacy shared-by-kind perspectives.
    pub view: String,
    /// Id of the view instance this perspective is scoped to.
    ///
    /// `None` means legacy shared-by-kind: the perspective applies to every
    /// view whose kind matches `view`. `Some(id)` pins the perspective to
    /// exactly that view instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
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
            view_id: None,
            fields: Vec::new(),
            filter: None,
            group: None,
            sort: Vec::new(),
        }
    }

    /// Pin the new perspective to a specific view instance.
    pub fn with_view_id(mut self, view_id: impl Into<String>) -> Self {
        self.view_id = Some(view_id.into());
        self
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
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: std::result::Result<Value, KanbanError> = async {
            let pctx = ctx.perspective_context().await?;
            let mut pctx = pctx.write().await;

            let id = ulid::Ulid::new().to_string();
            let mut perspective = Perspective::new(id, self.name.clone(), self.view.clone())
                .with_fields(self.fields.clone())
                .with_sort(self.sort.clone());
            perspective.view_id = self.view_id.clone();
            perspective.filter = self.filter.clone();
            perspective.group = self.group.clone();

            // Opt-in legacy migration: when the perspective lands without a
            // `view_id` and the workspace has exactly one matching view, pin
            // it before persisting. See `perspective::migrate` for the
            // save-vs-load placement rationale.
            if let Some(views_lock) = ctx.views() {
                let views = views_lock.read().await;
                crate::perspective::migrate::maybe_pin_view_id_on_save(&mut perspective, &views);
            }

            pctx.write(&perspective).await?;

            Ok(perspective_to_json(&perspective))
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

/// Convert a Perspective to the API JSON format.
pub(crate) fn perspective_to_json(p: &Perspective) -> Value {
    serde_json::to_value(p).expect("Perspective serializes to JSON")
}
