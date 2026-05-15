//! Shared helpers for task operations.

use crate::error::KanbanError;
use crate::tag::tag_name_exists_entity;
use crate::{auto_color, tag_parser};
use chrono::{DateTime, NaiveDate};
use serde_json::json;
use swissarmyhammer_entity::{Entity, EntityContext};

/// Parse and normalize an ISO 8601 date string.
///
/// Accepts either:
///   - a calendar date (`YYYY-MM-DD`), or
///   - an RFC 3339 / ISO 8601 datetime (e.g. `2026-04-30T12:00:00Z`).
///
/// Returns the string normalized to the canonical `YYYY-MM-DD` calendar-date
/// form (datetimes are truncated to their date portion) so downstream storage
/// and UI see a single consistent representation.
///
/// Surrounding whitespace is tolerated (trimmed before parsing) so the Rust
/// builder path and the JSON dispatch path behave identically for inputs like
/// `"  "` or `" 2026-04-30 "`.
///
/// `field_name` is used in the error message to identify which parameter was
/// invalid. An empty or whitespace-only string is rejected — callers that want
/// "clear this field" semantics must handle that before calling this function.
pub(crate) fn parse_iso8601_date(value: &str, field_name: &str) -> Result<String, KanbanError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(KanbanError::parse(format!(
            "invalid {field_name} date: empty string (omit the field to leave it unset)"
        )));
    }

    // Try a plain calendar date first (most common case).
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok(date.format("%Y-%m-%d").to_string());
    }

    // Fall back to RFC 3339 / ISO 8601 datetime, truncated to the date portion.
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.date_naive().format("%Y-%m-%d").to_string());
    }

    Err(KanbanError::parse(format!(
        "invalid {field_name} date: {value:?} — expected ISO 8601 date (YYYY-MM-DD) \
         or RFC 3339 datetime"
    )))
}

/// Parse an optional filter DSL string into a compiled expression.
///
/// Returns `None` for empty/whitespace-only input, `Ok(Some(expr))` for valid
/// DSL, or `Err` with a parse error for invalid expressions.
pub(crate) fn parse_filter_expr(
    filter: Option<&str>,
) -> Result<Option<swissarmyhammer_filter_expr::Expr>, KanbanError> {
    match filter.filter(|f| !f.trim().is_empty()) {
        Some(f) => {
            let expr = swissarmyhammer_filter_expr::parse(f).map_err(|errors| {
                let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
                KanbanError::parse(format!("invalid filter: {}", msgs.join("; ")))
            })?;
            Ok(Some(expr))
        }
        None => Ok(None),
    }
}

/// Auto-create Tag entities for any `#tag` patterns in an entity's body field.
///
/// Tags that already exist are skipped. New tags get an auto-generated color.
pub(crate) async fn auto_create_body_tags(
    ectx: &EntityContext,
    entity: &Entity,
) -> Result<(), KanbanError> {
    let body = entity.get_str("body").unwrap_or("");
    let tags = tag_parser::parse_tags(body);
    for tag_name in &tags {
        if !tag_name_exists_entity(ectx, tag_name).await {
            let color = auto_color::auto_color(tag_name).to_string();
            let tag_id = ulid::Ulid::new().to_string();
            let mut tag_entity = Entity::new("tag", tag_id.as_str());
            tag_entity.set("tag_name", json!(tag_name));
            tag_entity.set("color", json!(color));
            ectx.write(&tag_entity).await?;
        }
    }
    Ok(())
}
