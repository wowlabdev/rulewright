use ra_ap_syntax::{
    AstNode,
    ast::{self, ArithOp, BinaryOp, CmpOp, LogicOp},
};

use crate::{AstCtx, Example, Violation};

use super::super::support::compact_syntax;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "x == x",
        code: "fn f(x: i32) { if x == x {} }",
        pass: false,
    },
    Example {
        label: "x == y",
        code: "fn f(x: i32, y: i32) { if x == y {} }",
        pass: true,
    },
    Example {
        label: "a - a",
        code: "fn f(a: i32) { let _z = a - a; }",
        pass: false,
    },
    Example {
        label: "a + b",
        code: "fn f(a: i32, b: i32) { let _z = a + b; }",
        pass: true,
    },
    Example {
        label: "b && b",
        code: "fn f(b: bool) { if b && b {} }",
        pass: false,
    },
    Example {
        label: "dup in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t(x: i32) { if x == x {} }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    dup_expressions,
    "Flag identical sub-expressions like `x == x`, `a - a`, `b && b`.",
    "Identical operands on both sides of an operator (x == x, a - a) are almost always copy-paste bugs.",
    High,
);

fn check_dup_expressions(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::BinExpr>()
        .filter(|expr| !ctx.is_in_test(expr))
        .filter_map(|expr| {
            let op = expr.op_kind()?;

            if !is_suspicious_op(op) {
                return None;
            }

            let (left, right) = expr.sub_exprs();
            let (left, right) = (left?, right?);

            (compact_syntax(left.syntax()) == compact_syntax(right.syntax())).then(|| {
                let message = format!("identical sub-expressions on both sides of `{op}`");

                ctx.violation(&expr, message)
            })
        })
        .collect()
}

fn is_suspicious_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::CmpOp(CmpOp::Eq { .. })
            | BinaryOp::LogicOp(LogicOp::And | LogicOp::Or)
            | BinaryOp::ArithOp(
                ArithOp::Sub
                    | ArithOp::Div
                    | ArithOp::Rem
                    | ArithOp::BitXor
                    | ArithOp::BitAnd
                    | ArithOp::BitOr
            )
    )
}

crate::rulewright_ast_test!(check_dup_expressions, {
    crate::example_tests!(EXAMPLES, check_dup_expressions);
});
