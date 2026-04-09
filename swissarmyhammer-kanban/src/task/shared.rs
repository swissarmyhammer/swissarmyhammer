//! Shared helpers for task operations.

use crate::error::KanbanError;
use crate::tag::tag_name_exists_entity;
use crate::{auto_color, tag_parser};
use serde_json::json;
use swissarmyhammer_entity::{Entity, EntityContext};

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
