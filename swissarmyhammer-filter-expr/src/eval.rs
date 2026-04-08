use crate::Expr;

/// Trait for providing entity data to the filter evaluator.
///
/// Implementors map filter DSL atoms to entity fields:
/// - `has_tag("bug")` → entity's tags (including virtual tags) contain "bug"
/// - `has_assignee("alice")` → entity is assigned to "alice"
/// - `has_ref("01ABC")` → entity references card "01ABC" (via depends_on or id)
pub trait FilterContext {
    /// Returns true if the entity has the given tag (case-insensitive).
    fn has_tag(&self, tag: &str) -> bool;

    /// Returns true if the entity is assigned to the given user (case-insensitive).
    fn has_assignee(&self, user: &str) -> bool;

    /// Returns true if the entity references the given card ID.
    fn has_ref(&self, id: &str) -> bool;
}

/// Evaluate a filter expression against a context.
pub(crate) fn evaluate(expr: &Expr, ctx: &dyn FilterContext) -> bool {
    match expr {
        Expr::Tag(tag) => ctx.has_tag(tag),
        Expr::Assignee(user) => ctx.has_assignee(user),
        Expr::Ref(id) => ctx.has_ref(id),
        Expr::And(lhs, rhs) => evaluate(lhs, ctx) && evaluate(rhs, ctx),
        Expr::Or(lhs, rhs) => evaluate(lhs, ctx) || evaluate(rhs, ctx),
        Expr::Not(inner) => !evaluate(inner, ctx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCtx {
        tags: Vec<&'static str>,
        assignees: Vec<&'static str>,
        refs: Vec<&'static str>,
    }

    impl FilterContext for MockCtx {
        fn has_tag(&self, tag: &str) -> bool {
            self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
        }
        fn has_assignee(&self, user: &str) -> bool {
            self.assignees.iter().any(|a| a.eq_ignore_ascii_case(user))
        }
        fn has_ref(&self, id: &str) -> bool {
            self.refs.iter().any(|r| *r == id)
        }
    }

    fn mock(tags: &[&'static str], assignees: &[&'static str], refs: &[&'static str]) -> MockCtx {
        MockCtx {
            tags: tags.to_vec(),
            assignees: assignees.to_vec(),
            refs: refs.to_vec(),
        }
    }

    #[test]
    fn tag_positive() {
        assert!(evaluate(
            &Expr::Tag("bug".into()),
            &mock(&["bug"], &[], &[])
        ));
    }

    #[test]
    fn tag_negative() {
        assert!(!evaluate(
            &Expr::Tag("bug".into()),
            &mock(&["feature"], &[], &[])
        ));
    }

    #[test]
    fn tag_case_insensitive() {
        assert!(evaluate(
            &Expr::Tag("READY".into()),
            &mock(&["ready"], &[], &[])
        ));
    }

    #[test]
    fn assignee_positive() {
        assert!(evaluate(
            &Expr::Assignee("will".into()),
            &mock(&[], &["will"], &[])
        ));
    }

    #[test]
    fn ref_positive() {
        assert!(evaluate(
            &Expr::Ref("01ABC".into()),
            &mock(&[], &[], &["01ABC"])
        ));
    }

    #[test]
    fn and_both_true() {
        let expr = Expr::And(
            Box::new(Expr::Tag("bug".into())),
            Box::new(Expr::Assignee("will".into())),
        );
        assert!(evaluate(&expr, &mock(&["bug"], &["will"], &[])));
    }

    #[test]
    fn and_one_false() {
        let expr = Expr::And(
            Box::new(Expr::Tag("bug".into())),
            Box::new(Expr::Assignee("will".into())),
        );
        assert!(!evaluate(&expr, &mock(&["bug"], &["alice"], &[])));
    }

    #[test]
    fn or_one_true() {
        let expr = Expr::Or(
            Box::new(Expr::Tag("bug".into())),
            Box::new(Expr::Tag("feature".into())),
        );
        assert!(evaluate(&expr, &mock(&["feature"], &[], &[])));
    }

    #[test]
    fn or_both_false() {
        let expr = Expr::Or(
            Box::new(Expr::Tag("bug".into())),
            Box::new(Expr::Tag("feature".into())),
        );
        assert!(!evaluate(&expr, &mock(&["docs"], &[], &[])));
    }

    #[test]
    fn not_negates() {
        let expr = Expr::Not(Box::new(Expr::Tag("done".into())));
        assert!(evaluate(&expr, &mock(&["bug"], &[], &[])));
        assert!(!evaluate(&expr, &mock(&["done"], &[], &[])));
    }

    #[test]
    fn short_circuit_and() {
        // AND short-circuits: if left is false, right is never evaluated
        let expr = Expr::And(
            Box::new(Expr::Tag("nonexistent".into())),
            Box::new(Expr::Assignee("will".into())),
        );
        assert!(!evaluate(&expr, &mock(&[], &["will"], &[])));
    }

    #[test]
    fn short_circuit_or() {
        // OR short-circuits: if left is true, right is never evaluated
        let expr = Expr::Or(
            Box::new(Expr::Tag("bug".into())),
            Box::new(Expr::Tag("nonexistent".into())),
        );
        assert!(evaluate(&expr, &mock(&["bug"], &[], &[])));
    }

    #[test]
    fn nested_expression() {
        // (#bug || #feature) && !#done
        let expr = Expr::And(
            Box::new(Expr::Or(
                Box::new(Expr::Tag("bug".into())),
                Box::new(Expr::Tag("feature".into())),
            )),
            Box::new(Expr::Not(Box::new(Expr::Tag("done".into())))),
        );
        assert!(evaluate(&expr, &mock(&["bug"], &[], &[])));
        assert!(evaluate(&expr, &mock(&["feature"], &[], &[])));
        assert!(!evaluate(&expr, &mock(&["bug", "done"], &[], &[])));
        assert!(!evaluate(&expr, &mock(&["docs"], &[], &[])));
    }
}
