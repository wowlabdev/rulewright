use ra_ap_syntax::{
    AstNode,
    ast::{self, LiteralKind, UnaryOp},
};

use super::support::parse_expr;
use crate::{AstCtx, Example, Violation};

use super::super::support::macro_arguments;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "const vs literal",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(MAX_SIZE, 10); }\n}",
        pass: false,
    },
    Example {
        label: "literal vs const",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(3, Config::DEFAULT_RETRIES); }\n}",
        pass: false,
    },
    Example {
        label: "const vs array literal",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(CHECKPOINTS, [0, 90, 180, 270]); }\n}",
        pass: false,
    },
    Example {
        label: "literal vs literal",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(2, 2); }\n}",
        pass: false,
    },
    Example {
        label: "assert_ne const vs literal",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_ne!(LIMIT, 0); }\n}",
        pass: false,
    },
    Example {
        label: "test attr without cfg test module",
        code: "#[test]\nfn t() { assert_eq!(VERSION, \"1.0\"); }",
        pass: false,
    },
    Example {
        label: "behavior vs literal",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(add(1, 2), 3); }\n}",
        pass: true,
    },
    Example {
        label: "variable vs const",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { let x = grow(); assert_eq!(x, MAX_SIZE); }\n}",
        pass: true,
    },
    Example {
        label: "method call vs literal",
        code: "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() { assert_eq!(result.len(), 4); }\n}",
        pass: true,
    },
    Example {
        label: "production assert is out of scope",
        code: "fn f() { assert_eq!(MAX_SIZE, 10); }",
        pass: true,
    },
];

crate::ast_rule!(
    tautological_assert,
    "Flag test asserts comparing a constant against a literal (or literal vs literal).",
    "Asserts that restate a definition pass by construction and add noise instead of verifying behavior; test a property the value must satisfy instead.",
    Low,
);

#[derive(Clone, Copy, Eq, PartialEq)]
enum ArgKind {
    ConstPath,
    Literal,
    Other,
}

fn is_screaming_snake(ident: &str) -> bool {
    ident.len() > 1
        && ident.chars().any(|c| c.is_ascii_uppercase())
        && ident
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn is_plain_literal(literal: &ast::Literal) -> bool {
    matches!(
        literal.kind(),
        LiteralKind::IntNumber(_)
            | LiteralKind::FloatNumber(_)
            | LiteralKind::String(_)
            | LiteralKind::Bool(_)
    )
}

fn unwrap_expr(mut expr: ast::Expr) -> ast::Expr {
    loop {
        match expr {
            ast::Expr::ParenExpr(inner) => {
                let Some(nested) = inner.expr() else {
                    return ast::Expr::ParenExpr(inner);
                };

                expr = nested;
            }

            ast::Expr::RefExpr(inner) => {
                let Some(nested) = inner.expr() else {
                    return ast::Expr::RefExpr(inner);
                };

                expr = nested;
            }

            ast::Expr::PrefixExpr(inner) if inner.op_kind() == Some(UnaryOp::Neg) => {
                let Some(nested) = inner.expr() else {
                    return ast::Expr::PrefixExpr(inner);
                };

                expr = nested;
            }

            _ => return expr,
        }
    }
}

fn is_literal_expr(expr: ast::Expr) -> bool {
    matches!(unwrap_expr(expr), ast::Expr::Literal(literal) if is_plain_literal(&literal))
}

fn classify(expr: ast::Expr) -> ArgKind {
    match unwrap_expr(expr) {
        ast::Expr::Literal(literal) if is_plain_literal(&literal) => ArgKind::Literal,

        ast::Expr::ArrayExpr(array)
            if array.semicolon_token().is_none()
                && array
                    .syntax()
                    .children()
                    .filter_map(ast::Expr::cast)
                    .all(is_literal_expr) =>
        {
            ArgKind::Literal
        }

        ast::Expr::TupleExpr(tuple) if tuple.fields().all(is_literal_expr) => ArgKind::Literal,

        ast::Expr::PathExpr(path) => {
            let name = path
                .path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref());

            name.map_or(ArgKind::Other, |name| {
                if is_screaming_snake(name.text().as_str()) {
                    ArgKind::ConstPath
                } else {
                    ArgKind::Other
                }
            })
        }

        _ => ArgKind::Other,
    }
}

fn is_tautological(a: ArgKind, b: ArgKind) -> bool {
    matches!(
        (a, b),
        (ArgKind::ConstPath | ArgKind::Literal, ArgKind::Literal)
            | (ArgKind::Literal, ArgKind::ConstPath)
    )
}

fn check_tautological_assert(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| ctx.is_in_test(call))
        .filter_map(|call| {
            let name = call
                .path()?
                .segment()?
                .name_ref()?
                .text()
                .to_string();

            if !matches!(name.as_str(), "assert_eq" | "assert_ne") {
                return None;
            }

            let args = macro_arguments(&call)?;
            let left = parse_expr(args.first()?)?;
            let right = parse_expr(args.get(1)?)?;

            is_tautological(classify(left), classify(right)).then(|| {
                ctx.violation(
                    &call,
                    "tautological assert restates a definition — test a property the value must satisfy instead",
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_tautological_assert, {
    crate::example_tests!(EXAMPLES, check_tautological_assert);
});
