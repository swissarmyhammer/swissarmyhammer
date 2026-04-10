use chumsky::prelude::*;

use crate::Expr;

/// A parse error with span information.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Human-readable error message.
    pub message: String,
    /// Byte offset range in the input where the error occurred.
    pub span: std::ops::Range<usize>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

/// Returns true if `c` is a valid character inside an atom body (after the sigil).
///
/// Excludes whitespace, sigils (`#@^$`), parens, and operator characters (`&|!`)
/// so the parser never accidentally consumes operators as part of an atom name.
/// Excluding `$` prevents inputs like `$$bar` from parsing as `Project("$bar")`.
fn is_body_char(c: &char) -> bool {
    !c.is_whitespace() && !"#@^$()&|!".contains(*c)
}

/// Returns true if `c` cannot follow a keyword (word boundary lookahead).
fn is_keyword_boundary(c: &char) -> bool {
    !c.is_alphanumeric() && *c != '_'
}

/// Build a parser that matches a keyword string followed by a word boundary.
///
/// Ensures "not" doesn't match inside "nothing", "and" doesn't match inside
/// "android", etc. Uses a rewind lookahead so the boundary char is not consumed.
fn keyword<'src>(
    word: &'src str,
) -> impl Parser<'src, &'src str, (), extra::Err<Rich<'src, char>>> + Clone {
    just(word)
        .then_ignore(any().filter(is_keyword_boundary).rewind())
        .to(())
}

/// Build a parser for a binary keyword operator with both lower and uppercase variants,
/// plus a symbolic alternative (e.g. "&&", "||").
fn binary_op<'src>(
    symbol: &'src str,
    lower: &'src str,
    upper: &'src str,
) -> impl Parser<'src, &'src str, (), extra::Err<Rich<'src, char>>> + Clone {
    choice((just(symbol).to(()), keyword(lower), keyword(upper))).padded()
}

/// Build the atom and NOT-expression parsers (highest precedence layer).
fn atom_and_not<'src>(
    expr: impl Parser<'src, &'src str, Expr, extra::Err<Rich<'src, char>>> + Clone,
) -> impl Parser<'src, &'src str, Expr, extra::Err<Rich<'src, char>>> + Clone {
    let body = any().filter(is_body_char).repeated().at_least(1).to_slice();

    let tag = just('#')
        .ignore_then(body)
        .map(|s: &str| Expr::Tag(s.to_string()));
    let mention = just('@')
        .ignore_then(body)
        .map(|s: &str| Expr::Assignee(s.to_string()));
    let reference = just('^')
        .ignore_then(body)
        .map(|s: &str| Expr::Ref(s.to_string()));
    let project = just('$')
        .ignore_then(body)
        .map(|s: &str| Expr::Project(s.to_string()));
    let group = expr.delimited_by(just('(').padded(), just(')').padded());
    let atom = choice((tag, mention, reference, project, group)).padded();

    let not_op = choice((just('!').to(()), keyword("not"), keyword("NOT"))).padded();
    not_op
        .repeated()
        .foldr(atom, |_op, rhs| Expr::Not(Box::new(rhs)))
}

/// Build the chumsky parser for the filter DSL.
///
/// Grammar (precedence low→high):
/// ```text
/// expr      = or_expr
/// or_expr   = and_expr (("||" | "or" | "OR") and_expr)*
/// and_expr  = not_expr (("&&" | "and" | "AND")? not_expr)*   // implicit AND
/// not_expr  = ("!" | "not" | "NOT") not_expr | atom
/// atom      = "#" body | "@" body | "^" body | "$" body | "(" expr ")"
/// body      = [^ \t\n\r#@^$()&|!]+
/// ```
fn filter_parser<'src>() -> impl Parser<'src, &'src str, Expr, extra::Err<Rich<'src, char>>> {
    recursive(|expr| {
        let not_expr = atom_and_not(expr);
        let and_op = binary_op("&&", "and", "AND");

        // Implicit AND: operator is optional between adjacent not_exprs.
        let and_expr = not_expr.clone().foldl(
            and_op.or_not().ignore_then(not_expr).repeated(),
            |lhs, rhs| Expr::And(Box::new(lhs), Box::new(rhs)),
        );

        let or_op = binary_op("||", "or", "OR");
        and_expr
            .clone()
            .foldl(or_op.ignore_then(and_expr).repeated(), |lhs, rhs| {
                Expr::Or(Box::new(lhs), Box::new(rhs))
            })
    })
}

/// Parse a filter expression string into an AST.
///
/// Returns `Ok(Expr)` on success, or `Err(Vec<ParseError>)` with span info
/// on failure. Empty or whitespace-only input returns an error.
pub fn parse(input: &str) -> Result<Expr, Vec<ParseError>> {
    if input.trim().is_empty() {
        return Err(vec![ParseError {
            message: "empty filter expression".to_string(),
            span: 0..input.len(),
        }]);
    }

    let result = filter_parser().parse(input);

    if result.has_errors() {
        let errors: Vec<ParseError> = result
            .into_errors()
            .into_iter()
            .map(|e| {
                let span = e.span();
                ParseError {
                    message: format!("{e}"),
                    span: span.start..span.end,
                }
            })
            .collect();
        Err(errors)
    } else {
        // The parser succeeded; unwrap the output.
        result.into_output().ok_or_else(|| {
            vec![ParseError {
                message: "unexpected parse failure".to_string(),
                span: 0..input.len(),
            }]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Atom parsing ────────────────────────────────────────────────

    #[test]
    fn tag_atom() {
        assert_eq!(parse("#bug").unwrap(), Expr::Tag("bug".into()));
    }

    #[test]
    fn assignee_atom() {
        assert_eq!(parse("@alice").unwrap(), Expr::Assignee("alice".into()));
    }

    #[test]
    fn ref_atom() {
        assert_eq!(parse("^01ABC").unwrap(), Expr::Ref("01ABC".into()));
    }

    #[test]
    fn tag_with_hyphens() {
        assert_eq!(parse("#bug-fix").unwrap(), Expr::Tag("bug-fix".into()));
    }

    #[test]
    fn tag_with_dots() {
        assert_eq!(parse("#v2.0").unwrap(), Expr::Tag("v2.0".into()));
    }

    #[test]
    fn tag_with_underscores() {
        assert_eq!(parse("#my_tag").unwrap(), Expr::Tag("my_tag".into()));
    }

    // ── Project atom (`$project`) ──────────────────────────────────

    #[test]
    fn project_atom() {
        assert_eq!(
            parse("$auth-migration").unwrap(),
            Expr::Project("auth-migration".into())
        );
    }

    #[test]
    fn project_with_dots() {
        assert_eq!(parse("$v2.0").unwrap(), Expr::Project("v2.0".into()));
    }

    #[test]
    fn project_with_underscores() {
        assert_eq!(
            parse("$my_project").unwrap(),
            Expr::Project("my_project".into())
        );
    }

    #[test]
    fn project_dollar_alone_is_error() {
        // `$` alone has a zero-length body, which violates `at_least(1)`.
        assert!(parse("$").is_err());
    }

    #[test]
    fn project_double_dollar_is_error() {
        // `$$bar` has an empty body after the first `$` sigil (because the
        // second `$` is now excluded from body chars), so it must fail.
        assert!(parse("$$bar").is_err());
    }

    #[test]
    fn project_combines_with_and() {
        assert_eq!(
            parse("$auth && #bug").unwrap(),
            Expr::And(
                Box::new(Expr::Project("auth".into())),
                Box::new(Expr::Tag("bug".into())),
            )
        );
    }

    #[test]
    fn project_implicit_and() {
        // `$auth #bug @alice` builds a left-associative implicit-AND chain.
        assert_eq!(
            parse("$auth #bug @alice").unwrap(),
            Expr::And(
                Box::new(Expr::And(
                    Box::new(Expr::Project("auth".into())),
                    Box::new(Expr::Tag("bug".into())),
                )),
                Box::new(Expr::Assignee("alice".into())),
            )
        );
    }

    #[test]
    fn not_project() {
        assert_eq!(
            parse("!$auth").unwrap(),
            Expr::Not(Box::new(Expr::Project("auth".into())))
        );
    }

    // ── NOT operator ────────────────────────────────────────────────

    #[test]
    fn not_bang() {
        assert_eq!(
            parse("!#done").unwrap(),
            Expr::Not(Box::new(Expr::Tag("done".into())))
        );
    }

    #[test]
    fn not_keyword() {
        assert_eq!(
            parse("not #done").unwrap(),
            Expr::Not(Box::new(Expr::Tag("done".into())))
        );
    }

    #[test]
    fn not_keyword_uppercase() {
        assert_eq!(
            parse("NOT #done").unwrap(),
            Expr::Not(Box::new(Expr::Tag("done".into())))
        );
    }

    #[test]
    fn double_not() {
        assert_eq!(
            parse("!!#done").unwrap(),
            Expr::Not(Box::new(Expr::Not(Box::new(Expr::Tag("done".into())))))
        );
    }

    // ── AND operator ────────────────────────────────────────────────

    #[test]
    fn and_explicit_ampersand() {
        assert_eq!(
            parse("#a && #b").unwrap(),
            Expr::And(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    #[test]
    fn and_keyword() {
        assert_eq!(
            parse("#a and #b").unwrap(),
            Expr::And(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    #[test]
    fn and_keyword_uppercase() {
        assert_eq!(
            parse("#a AND #b").unwrap(),
            Expr::And(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    #[test]
    fn and_implicit() {
        assert_eq!(
            parse("#a #b").unwrap(),
            Expr::And(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    #[test]
    fn and_implicit_three_atoms() {
        assert_eq!(
            parse("#a #b #c").unwrap(),
            Expr::And(
                Box::new(Expr::And(
                    Box::new(Expr::Tag("a".into())),
                    Box::new(Expr::Tag("b".into())),
                )),
                Box::new(Expr::Tag("c".into())),
            )
        );
    }

    // ── OR operator ─────────────────────────────────────────────────

    #[test]
    fn or_explicit_pipe() {
        assert_eq!(
            parse("#a || #b").unwrap(),
            Expr::Or(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    #[test]
    fn or_keyword() {
        assert_eq!(
            parse("#a or #b").unwrap(),
            Expr::Or(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    #[test]
    fn or_keyword_uppercase() {
        assert_eq!(
            parse("#a OR #b").unwrap(),
            Expr::Or(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    // ── Precedence ──────────────────────────────────────────────────

    #[test]
    fn precedence_and_over_or() {
        // #a || #b && #c  →  #a || (#b && #c)
        assert_eq!(
            parse("#a || #b && #c").unwrap(),
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
    fn precedence_not_over_and() {
        // !#a && #b  →  (!#a) && #b
        assert_eq!(
            parse("!#a && #b").unwrap(),
            Expr::And(
                Box::new(Expr::Not(Box::new(Expr::Tag("a".into())))),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }

    // ── Grouping ────────────────────────────────────────────────────

    #[test]
    fn grouping_overrides_precedence() {
        assert_eq!(
            parse("(#a || #b) && #c").unwrap(),
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
    fn nested_grouping() {
        assert_eq!(parse("((#a))").unwrap(), Expr::Tag("a".into()));
    }

    // ── Keyword operator full expression ────────────────────────────

    #[test]
    fn keyword_operators_full() {
        // not #done and @will or #bug  →  ((!#done) && @will) || #bug
        assert_eq!(
            parse("not #done and @will or #bug").unwrap(),
            Expr::Or(
                Box::new(Expr::And(
                    Box::new(Expr::Not(Box::new(Expr::Tag("done".into())))),
                    Box::new(Expr::Assignee("will".into())),
                )),
                Box::new(Expr::Tag("bug".into())),
            )
        );
    }

    // ── Error cases ─────────────────────────────────────────────────

    #[test]
    fn error_empty() {
        assert!(parse("").is_err());
    }

    #[test]
    fn error_whitespace_only() {
        assert!(parse("   ").is_err());
    }

    #[test]
    fn error_invalid_chars() {
        // `$$garbage` fails because the first `$` is consumed as the project
        // sigil, then the body parser needs at least one non-sigil char, but
        // the next char is another `$` (excluded from body chars). So the
        // project atom fails to parse.
        assert!(parse("$$garbage").is_err());
    }

    #[test]
    fn error_incomplete_and() {
        assert!(parse("#bug &&").is_err());
    }

    #[test]
    fn error_incomplete_or() {
        assert!(parse("#bug ||").is_err());
    }

    #[test]
    fn error_unmatched_paren() {
        assert!(parse("(#bug").is_err());
    }

    #[test]
    fn error_has_span_info() {
        // `$$` fails for the same reason as `$$garbage`: the first `$`
        // becomes the project sigil and the second `$` cannot start the
        // body (it is an excluded sigil char). The resulting error must
        // carry span info.
        let errors = parse("$$").unwrap_err();
        assert!(!errors.is_empty());
        // Span should be within the input range
        assert!(errors[0].span.start <= 2);
    }

    // ── Whitespace handling ─────────────────────────────────────────

    #[test]
    fn leading_trailing_whitespace() {
        assert_eq!(parse("  #bug  ").unwrap(), Expr::Tag("bug".into()));
    }

    #[test]
    fn mixed_whitespace() {
        assert_eq!(
            parse("#a  &&  #b").unwrap(),
            Expr::And(
                Box::new(Expr::Tag("a".into())),
                Box::new(Expr::Tag("b".into())),
            )
        );
    }
}
