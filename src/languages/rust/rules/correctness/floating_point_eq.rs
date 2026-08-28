use ra_ap_syntax::{
    AstNode,
    ast::{self, BinaryOp, CmpOp, HasName, LiteralKind},
};
use std::collections::HashSet;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "direct f64 equality",
        code: "fn f(a: f64, b: f64) -> bool { a == b }",
        pass: false,
    },
    Example {
        label: "direct f32 equality",
        code: "fn f(a: f32, b: f32) -> bool { a == b }",
        pass: false,
    },
    Example {
        label: "f64 not-equal",
        code: "fn f(a: f64, b: f64) -> bool { a != b }",
        pass: false,
    },
    Example {
        label: "integer equality is fine",
        code: "fn f(a: i32, b: i32) -> bool { a == b }",
        pass: true,
    },
    Example {
        label: "float comparison with epsilon",
        code: "fn f(a: f64, b: f64) -> bool { (a - b).abs() < f64::EPSILON }",
        pass: true,
    },
    Example {
        label: "float eq in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t(a: f64, b: f64) -> bool { a == b }\n}",
        pass: true,
    },
    Example {
        label: "compare float to literal zero",
        code: "fn f(a: f64) -> bool { a == 0.0 }",
        pass: true,
    },
    Example {
        label: "local float binding equality",
        code: "fn f() -> bool { let x: f64 = 1.0; x == 0.0 }",
        pass: true,
    },
    Example {
        label: "local float literal equality",
        code: "fn f() -> bool { let x = 1.0; x == 0.0 }",
        pass: true,
    },
    Example {
        label: "left-hand-side float literal",
        code: "fn f(a: f64) -> bool { 0.0 == a }",
        pass: true,
    },
    Example {
        label: "cast-to-float comparison",
        code: "fn f(b: i32) -> bool { b as f64 == 0 }",
        pass: false,
    },
    Example {
        label: "compare float to nonzero literal",
        code: "fn f(a: f64) -> bool { a == 1.5 }",
        pass: false,
    },
    Example {
        label: "compare float to negative zero",
        code: "fn f(a: f64) -> bool { a != -0.0 }",
        pass: true,
    },
    Example {
        label: "float equality in impl method",
        code: "struct S; impl S { fn m(&self, a: f64, b: f64) -> bool { a == b } }",
        pass: false,
    },
    Example {
        label: "integer not-equal is fine",
        code: "fn f(a: u32, b: u32) -> bool { a != b }",
        pass: true,
    },
];

crate::ast_rule!(
    floating_point_eq,
    "Flag direct nonzero `==`/`!=` comparison on `f32`/`f64` values for review.",
    "Computed floating-point values usually need absolute or relative tolerance. Exact zero is a useful structural and sentinel check; keep other exact equality only when the domain invariant genuinely requires it.",
    High,
);

fn check_floating_point_eq(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::BinExpr>()
        .filter(|expr| {
            !ctx.is_in_test(expr)
                && matches!(expr.op_kind(), Some(BinaryOp::CmpOp(CmpOp::Eq { .. })))
        })
        .filter_map(|expr| {
            let function = expr.syntax().ancestors().find_map(ast::Fn::cast)?;
            let float_names = float_names_before(&function, &expr);
            let (left, right) = expr.sub_exprs();
            let (left, right) = (left?, right?);

            if expr_is_zero_float_literal(&left) || expr_is_zero_float_literal(&right) {
                return None;
            }

            (expr_uses_float_name(&left, &float_names)
                || expr_uses_float_name(&right, &float_names))
            .then(|| {
                ctx.violation(
                    &expr,
                    "direct float equality comparison — use an absolute or relative tolerance, or document why this domain requires exact equality",
                )
            })
        })
        .collect()
}

fn expr_is_zero_float_literal(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Literal(literal) => literal_is_zero_float(literal),

        ast::Expr::PrefixExpr(prefix) => prefix.expr().is_some_and(|expr| {
            let ast::Expr::Literal(literal) = expr else {
                return false;
            };

            literal_is_zero_float(&literal)
        }),

        _ => false,
    }
}

fn literal_is_zero_float(literal: &ast::Literal) -> bool {
    let LiteralKind::FloatNumber(number) = literal.kind() else {
        return false;
    };
    let Ok(value) = number.value_string().parse::<f64>() else {
        return false;
    };

    value.to_bits() << 1 == 0
}

fn expr_is_float(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Literal(literal) => matches!(literal.kind(), LiteralKind::FloatNumber(_)),
        ast::Expr::CastExpr(cast) => cast.ty().is_some_and(|ty| type_is_float(&ty)),
        _ => false,
    }
}

fn expr_uses_float_name(expr: &ast::Expr, float_names: &HashSet<String>) -> bool {
    match expr {
        ast::Expr::PathExpr(path) => {
            let name = path
                .path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref());

            name.is_some_and(|name| float_names.contains(name.text().as_str()))
        }

        _ => expr_is_float(expr),
    }
}

fn float_names_before(function: &ast::Fn, expr: &ast::BinExpr) -> HashSet<String> {
    let mut names = HashSet::default();

    if let Some(parameters) = function.param_list() {
        for parameter in parameters.params() {
            if parameter.ty().is_some_and(|ty| type_is_float(&ty))
                && let Some(ast::Pat::IdentPat(pattern)) = parameter.pat()
                && let Some(name) = pattern.name()
            {
                names.insert(name.text().to_string());
            }
        }
    }

    let expression_start = expr.syntax().text_range().start();

    for local in function
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
        .filter(|local| {
            local.syntax().text_range().start() < expression_start
                && local.syntax().ancestors().find_map(ast::Fn::cast).as_ref() == Some(function)
        })
    {
        let Some(ast::Pat::IdentPat(pattern)) = local.pat() else {
            continue;
        };
        let is_float = local.ty().is_some_and(|ty| type_is_float(&ty))
            || local.initializer().is_some_and(|expr| expr_is_float(&expr));

        if is_float && let Some(name) = pattern.name() {
            names.insert(name.text().to_string());
        }
    }

    names
}

fn type_is_float(ty: &ast::Type) -> bool {
    let ast::Type::PathType(path) = ty else {
        return false;
    };

    let name = path
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());

    name.is_some_and(|name| matches!(name.text().as_str(), "f32" | "f64"))
}

crate::rulewright_ast_test!(check_floating_point_eq, {
    crate::example_tests!(EXAMPLES, check_floating_point_eq);
});
