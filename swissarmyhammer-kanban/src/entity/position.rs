//! Shared position-resolution helpers for kanban-column-placed entities.
//!
//! Entities whose schema declares `position_column` / `position_ordinal`
//! participate in kanban-column placement. Creating such an entity requires
//! resolving:
//!
//! - A target column (explicit override wins, else the lowest-order column
//!   on the board).
//! - A target ordinal (explicit override wins, else an ordinal strictly
//!   after every existing entity of the same type in the target column).
//!
//! Both `AddTask` (the opinionated task-specialised command) and `AddEntity`
//! (the generic schema-driven command surfaced dynamically from a view scope)
//! need identical resolution logic. Centralising it here means future changes
//! to ordinal computation propagate to every caller without duplication drift.

use crate::error::{KanbanError, Result};
use crate::types::Ordinal;
use swissarmyhammer_entity::EntityContext;

/// Entity field name that marks an entity as kanban-column-placed.
///
/// Exposed for callers that introspect entity schemas (e.g. `AddEntity`
/// checks whether the entity type opts into column placement by looking
/// for this field in the entity's declared field list).
pub const POSITION_COLUMN_FIELD: &str = "position_column";

/// Entity field name for the fractional-index ordinal that orders entities
/// within a column.
pub const POSITION_ORDINAL_FIELD: &str = "position_ordinal";

/// Resolve the target column for a new kanban-column-placed entity.
///
/// If `explicit` is `Some`, it is validated against the board's column
/// list and returned. Otherwise, the lowest-order column on the board is
/// chosen — matching the "tasks land in 'todo' by default" behaviour
/// callers expect.
///
/// # Arguments
/// - `ectx` — entity context used to enumerate existing columns.
/// - `explicit` — optional caller-supplied column id override. When
///   supplied, validated against the actual columns on the board;
///   unknown ids are rejected with [`KanbanError::parse`] rather than
///   silently written to the entity.
/// - `entity_type_for_error` — entity type name used only for the error
///   message when the board has no columns (e.g. `"task"`).
///
/// # Errors
/// - Returns [`KanbanError::parse`] when `explicit` is supplied but
///   names a column that does not exist on the board. Validating here
///   — rather than letting an arbitrary user-supplied string flow
///   through to the stored entity — prevents unknown / malformed values
///   from poisoning downstream queries that join on `position_column`.
/// - Returns [`KanbanError::parse`] when `explicit` is `None` and the
///   board has no columns — there is nothing sensible to fall back to.
pub async fn resolve_column(
    ectx: &EntityContext,
    explicit: Option<&str>,
    entity_type_for_error: &str,
) -> Result<String> {
    let columns = ectx.list("column").await?;
    if let Some(col) = explicit {
        if !columns.iter().any(|c| c.id == col) {
            return Err(KanbanError::parse(format!(
                "column '{col}' does not exist on this board"
            )));
        }
        return Ok(col.to_string());
    }
    let first = columns
        .iter()
        .min_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
        .ok_or_else(|| {
            KanbanError::parse(format!(
                "board has no columns — cannot add {entity_type_for_error} with position_column"
            ))
        })?;
    Ok(first.id.to_string())
}

/// Resolve the target ordinal for a new kanban-column-placed entity.
///
/// If `explicit` is `Some`, it is validated as a well-formed
/// `FractionalIndex` encoding and returned. Otherwise, an ordinal
/// strictly after every existing entity of type `entity_type` currently
/// in `column` is generated. When the column is empty, [`Ordinal::first`]
/// is returned.
///
/// # Arguments
/// - `ectx` — entity context used to enumerate existing entities.
/// - `entity_type` — the entity type to scan (e.g. `"task"`).
/// - `column` — the target column id; only entities currently in this
///   column contribute to the "after the last" computation.
/// - `explicit` — optional caller-supplied ordinal string override. When
///   supplied, the string must be a valid `FractionalIndex` encoding
///   ([`Ordinal::is_valid`]); malformed input is rejected rather than
///   silently collapsed to [`Ordinal::first`].
///
/// # Errors
/// Returns [`KanbanError::parse`] when `explicit` is supplied but is
/// not a valid `FractionalIndex` string. This surfaces bad input at the
/// point of entity creation rather than leaving a position that sorts
/// unexpectedly and collides with the next inserted entity.
pub async fn resolve_ordinal(
    ectx: &EntityContext,
    entity_type: &str,
    column: &str,
    explicit: Option<&str>,
) -> Result<String> {
    if let Some(ord) = explicit {
        if !Ordinal::is_valid(ord) {
            return Err(KanbanError::parse(format!(
                "ordinal '{ord}' is not a valid FractionalIndex encoding"
            )));
        }
        return Ok(ord.to_string());
    }
    let entities = ectx.list(entity_type).await?;
    let last = entities
        .iter()
        .filter(|e| e.get_str(POSITION_COLUMN_FIELD).unwrap_or("") == column)
        .filter_map(|e| e.get_str(POSITION_ORDINAL_FIELD).map(Ordinal::from_string))
        .max();
    Ok(match last {
        Some(last) => Ordinal::after(&last).as_str().to_string(),
        None => Ordinal::first().as_str().to_string(),
    })
}
