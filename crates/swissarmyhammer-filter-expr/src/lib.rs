//! Filter expression DSL for SwissArmyHammer kanban.
//!
//! Provides a parser and evaluator for a small boolean filter language:
//! - `#tag` — match entities with a given tag (including virtual tags)
//! - `@user` — match entities assigned to a user
//! - `^ref` — match entities referencing a card ID
//! - `$project` — match entities belonging to a project
//! - `&&` / `and` — boolean AND
//! - `||` / `or` — boolean OR
//! - `!` / `not` — boolean NOT
//! - `()` — grouping
//! - Adjacent atoms without an operator are implicit AND

mod eval;
mod parser;

pub use eval::FilterContext;
pub use parser::ParseError;

/// A parsed filter expression AST node.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Matches entities having a tag (e.g. `#bug`).
    Tag(String),
    /// Matches entities assigned to a user (e.g. `@alice`).
    Assignee(String),
    /// Matches entities referencing a card (e.g. `^TASK-ID`).
    Ref(String),
    /// Matches entities belonging to a project (e.g. `$auth-migration`).
    Project(String),
    /// Both sub-expressions must match.
    And(Box<Expr>, Box<Expr>),
    /// Either sub-expression must match.
    Or(Box<Expr>, Box<Expr>),
    /// The sub-expression must NOT match.
    Not(Box<Expr>),
}

impl Expr {
    /// Evaluate this expression against a filter context.
    ///
    /// Returns `true` if the entity described by `ctx` matches the filter.
    pub fn matches(&self, ctx: &dyn FilterContext) -> bool {
        eval::evaluate(self, ctx)
    }
}

/// Parse a filter expression string into an AST.
///
/// Returns `Ok(Expr)` on success, or `Err(Vec<ParseError>)` with span
/// information on failure.
pub fn parse(input: &str) -> Result<Expr, Vec<ParseError>> {
    parser::parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a simple FilterContext from tags, assignees, refs, and projects.
    struct TestCtx {
        tags: Vec<String>,
        assignees: Vec<String>,
        refs: Vec<String>,
        projects: Vec<String>,
    }

    impl FilterContext for TestCtx {
        fn has_tag(&self, tag: &str) -> bool {
            self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
        }
        fn has_assignee(&self, user: &str) -> bool {
            self.assignees.iter().any(|a| a.eq_ignore_ascii_case(user))
        }
        fn has_ref(&self, id: &str) -> bool {
            self.refs.iter().any(|r| r == id)
        }
        fn has_project(&self, project: &str) -> bool {
            self.projects
                .iter()
                .any(|p| p.eq_ignore_ascii_case(project))
        }
    }

    fn ctx(tags: &[&str], assignees: &[&str], refs: &[&str], projects: &[&str]) -> TestCtx {
        TestCtx {
            tags: tags.iter().map(|s| s.to_string()).collect(),
            assignees: assignees.iter().map(|s| s.to_string()).collect(),
            refs: refs.iter().map(|s| s.to_string()).collect(),
            projects: projects.iter().map(|s| s.to_string()).collect(),
        }
    }

    // ── Parser acceptance criteria ──────────────────────────────────

    #[test]
    fn parse_tag_and_assignee_explicit() {
        let expr = parse("#bug && @will").unwrap();
        assert_eq!(
            expr,
            Expr::And(
                Box::new(Expr::Tag("bug".into())),
                Box::new(Expr::Assignee("will".into())),
            )
        );
    }

    #[test]
    fn parse_implicit_and() {
        let expr = parse("#bug @will").unwrap();
        assert_eq!(
            expr,
            Expr::And(
                Box::new(Expr::Tag("bug".into())),
                Box::new(Expr::Assignee("will".into())),
            )
        );
    }

    #[test]
    fn parse_or() {
        let expr = parse("#bug || #feature").unwrap();
        assert_eq!(
            expr,
            Expr::Or(
                Box::new(Expr::Tag("bug".into())),
                Box::new(Expr::Tag("feature".into())),
            )
        );
    }

    #[test]
    fn parse_not() {
        let expr = parse("!#done").unwrap();
        assert_eq!(expr, Expr::Not(Box::new(Expr::Tag("done".into()))));
    }

    #[test]
    fn parse_precedence_and_over_or() {
        // AND binds tighter than OR: #a || #b && #c == #a || (#b && #c)
        let expr = parse("#a || #b && #c").unwrap();
        assert_eq!(
            expr,
            Expr::Or(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::And(
                    Box::new(Expr::Tag("b".into())),
                    Box::new(Expr::Tag("c".into())),
                )),
            )
        );
    }

    #[test]
    fn parse_grouping_overrides_precedence() {
        let expr = parse("(#a || #b) && #c").unwrap();
        assert_eq!(
            expr,
            Expr::And(
                Box::new(Expr::Or(
                    Box::new(Expr::Tag("a".into())),
                    Box::new(Expr::Tag("b".into())),
                )),
                Box::new(Expr::Tag("c".into())),
            )
        );
    }

    #[test]
    fn parse_keyword_operators() {
        let expr = parse("not #done and @will or #bug").unwrap();
        assert_eq!(
            expr,
            Expr::Or(
                Box::new(Expr::And(
                    Box::new(Expr::Not(Box::new(Expr::Tag("done".into())))),
                    Box::new(Expr::Assignee("will".into())),
                )),
                Box::new(Expr::Tag("bug".into())),
            )
        );
    }

    #[test]
    fn parse_uppercase_keyword_operators() {
        let expr = parse("NOT #done AND @will OR #bug").unwrap();
        assert_eq!(
            expr,
            Expr::Or(
                Box::new(Expr::And(
                    Box::new(Expr::Not(Box::new(Expr::Tag("done".into())))),
                    Box::new(Expr::Assignee("will".into())),
                )),
                Box::new(Expr::Tag("bug".into())),
            )
        );
    }

    #[test]
    fn parse_ref_atom() {
        let expr = parse("^01ABC").unwrap();
        assert_eq!(expr, Expr::Ref("01ABC".into()));
    }

    #[test]
    fn parse_tag_with_hyphens_and_dots() {
        let expr = parse("#v2.0 && #bug-fix").unwrap();
        assert_eq!(
            expr,
            Expr::And(
                Box::new(Expr::Tag("v2.0".into())),
                Box::new(Expr::Tag("bug-fix".into())),
            )
        );
    }

    #[test]
    fn parse_error_on_invalid_input() {
        // `$$garbage` fails because the first `$` is consumed as the project
        // sigil, then the body parser needs at least one non-sigil char, but
        // the next char is another `$` (excluded from body chars). So the
        // project atom fails to parse.
        let result = parse("$$garbage");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_on_empty_input() {
        let result = parse("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_on_incomplete() {
        let result = parse("#bug &&");
        assert!(result.is_err());
    }

    // ── Evaluator acceptance criteria ───────────────────────────────

    #[test]
    fn eval_tag_match() {
        let expr = parse("#bug").unwrap();
        assert!(expr.matches(&ctx(&["bug", "feature"], &[], &[], &[])));
    }

    #[test]
    fn eval_tag_no_match() {
        let expr = parse("#bug").unwrap();
        assert!(!expr.matches(&ctx(&["feature"], &[], &[], &[])));
    }

    #[test]
    fn eval_tag_case_insensitive() {
        let expr = parse("#READY").unwrap();
        assert!(expr.matches(&ctx(&["ready"], &[], &[], &[])));
    }

    #[test]
    fn eval_assignee_match() {
        let expr = parse("@will").unwrap();
        assert!(expr.matches(&ctx(&[], &["will"], &[], &[])));
    }

    #[test]
    fn eval_ref_match() {
        let expr = parse("^01ABC").unwrap();
        assert!(expr.matches(&ctx(&[], &[], &["01ABC"], &[])));
    }

    #[test]
    fn eval_and() {
        let expr = parse("#bug && @will").unwrap();
        assert!(expr.matches(&ctx(&["bug"], &["will"], &[], &[])));
        assert!(!expr.matches(&ctx(&["bug"], &["alice"], &[], &[])));
        assert!(!expr.matches(&ctx(&["feature"], &["will"], &[], &[])));
    }

    #[test]
    fn eval_or() {
        let expr = parse("#bug || #feature").unwrap();
        assert!(expr.matches(&ctx(&["bug"], &[], &[], &[])));
        assert!(expr.matches(&ctx(&["feature"], &[], &[], &[])));
        assert!(!expr.matches(&ctx(&["docs"], &[], &[], &[])));
    }

    #[test]
    fn eval_not() {
        let expr = parse("!#done").unwrap();
        assert!(expr.matches(&ctx(&["bug"], &[], &[], &[])));
        assert!(!expr.matches(&ctx(&["done"], &[], &[], &[])));
    }

    #[test]
    fn eval_complex_expression() {
        let expr = parse("(#bug || #feature) && @will && !#done").unwrap();
        assert!(expr.matches(&ctx(&["bug"], &["will"], &[], &[])));
        assert!(expr.matches(&ctx(&["feature"], &["will"], &[], &[])));
        assert!(!expr.matches(&ctx(&["bug"], &["alice"], &[], &[])));
        assert!(!expr.matches(&ctx(&["bug", "done"], &["will"], &[], &[])));
    }

    // ── Project atom acceptance ─────────────────────────────────────

    #[test]
    fn parse_project_atom() {
        let expr = parse("$auth-migration").unwrap();
        assert_eq!(expr, Expr::Project("auth-migration".into()));
    }

    #[test]
    fn eval_project_match() {
        let expr = parse("$auth").unwrap();
        assert!(expr.matches(&ctx(&[], &[], &[], &["auth"])));
        assert!(!expr.matches(&ctx(&[], &[], &[], &["frontend"])));
    }
}
