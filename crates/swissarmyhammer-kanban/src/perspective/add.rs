//! AddPerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::ensure_default;
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
    /// Idempotent "ensure" mode.
    ///
    /// When `true`, the op first looks for an existing perspective matching
    /// the target view scope (`view_id` when set — after legacy pinning —
    /// else view kind) and returns it WITHOUT writing. When nothing matches,
    /// the perspective is created under the deterministic scope-derived id
    /// ([`crate::perspective::ensure_default::default_perspective_id`])
    /// instead of a fresh ULID, so concurrent windows and sibling processes
    /// with stale caches converge on ONE file instead of accumulating
    /// duplicates (the "perspectives gone missing" live bug, task
    /// 01KTY6T1GPY94VYWANE9X41SKJ).
    #[serde(default)]
    pub ensure: bool,
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
            ensure: false,
        }
    }

    /// Switch on idempotent ensure mode (see the `ensure` field docs).
    pub fn with_ensure(mut self) -> Self {
        self.ensure = true;
        self
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

            if self.ensure {
                // The ensure-created default's id embeds the scope (the
                // view_id when pinned) in the on-disk filename
                // (`default-<scope>.yaml`), so a caller-supplied view_id
                // must be validated before it can reach the filesystem.
                // A view id missing from the views registry falls back to
                // the view-kind scope — otherwise an ensure against a dead
                // view re-mints a default the next board open prunes
                // (create/prune churn). When no registry is wired (bare
                // contexts), the filename-safety check is the backstop
                // against path separators and overlong components.
                if let Some(vid) = perspective.view_id.clone() {
                    let valid = match ctx.views() {
                        Some(views_lock) => views_lock.read().await.get_by_id(&vid).is_some(),
                        None => ensure_default::is_safe_scope_component(&vid),
                    };
                    if !valid {
                        tracing::warn!(
                            view_id = %vid,
                            "ensure: view_id is unknown or unsafe — falling back to view-kind scope"
                        );
                        perspective.view_id = None;
                    }
                }

                // Idempotent ensure: an existing perspective for this view
                // scope short-circuits the create (no write, no
                // store-changed notification). The check runs under the
                // perspective write lock taken above, so it is atomic with
                // the create within this process; ACROSS processes the
                // deterministic id below makes a racing create converge on
                // the same file instead of duplicating.
                if let Some(existing) = pctx.all().iter().find(|p| {
                    ensure_default::matches_scope(
                        p,
                        perspective.view_id.as_deref(),
                        &perspective.view,
                    )
                }) {
                    return Ok(perspective_to_json(existing));
                }
                perspective.id = ensure_default::default_perspective_id(
                    ensure_default::perspective_scope(&perspective),
                );
            }

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
