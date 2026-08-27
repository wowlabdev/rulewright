use ra_ap_syntax::ast::{self, BinaryOp, CmpOp};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "literal on left eq",
        code: "fn f(x: i32) { if 0 == x {} }",
        pass: false,
    },
    Example {
        label: "literal on right",
        code: "fn f(x: i32) { if x == 0 {} }",
        pass: true,
    },
    Example {
        label: "literal on left ne",
        code: "fn f(y: i32) { if 1 != y {} }",
        pass: false,
    },
    Example {
        label: "both literals",
        code: r#"fn f() { if "a" == "b" {} }"#,
        pass: true,
    },
    Example {
        label: "non-comparison",
        code: "fn f(x: i32) { let _ = 1 + x; }",
        pass: true,
    },
    Example {
        label: "yoda in test module",
        code: "#[cfg(test)]\nmod tests {\n  fn t(x: i32) { if 0 == x {} }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    yoda_conditions,
    "Flag reversed comparisons like `0 == x` — prefer `x == 0`.",
    "In Rust there is no accidental assignment in conditions, so the C-style 0 == x guard is unnecessary and harder to read.",
);

fn check_yoda_conditions(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::BinExpr>()
        .filter(|expr| {
            !ctx.is_in_test(expr)
                && matches!(expr.op_kind(), Some(BinaryOp::CmpOp(CmpOp::Eq { .. })))
        })
        .filter_map(|expr| {
            let (left, right) = expr.sub_exprs();
            let (left, right) = (left?, right?);

            (matches!(left, ast::Expr::Literal(_)) && !matches!(right, ast::Expr::Literal(_)))
                .then(|| ctx.violation(&left, "Yoda condition — put the literal on the right side"))
        })
        .collect()
}

crate::rulewright_ast_test!(check_yoda_conditions, {
    crate::example_tests!(EXAMPLES, check_yoda_conditions);
});
