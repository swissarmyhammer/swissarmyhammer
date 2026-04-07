//! Shared helpers for task operations.

use crate::error::KanbanError;

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
