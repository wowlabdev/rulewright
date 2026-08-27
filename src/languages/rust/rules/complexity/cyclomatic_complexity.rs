#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, BinaryOp, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "simple function",
        code: "fn f() { let x = 1; }",
        pass: true,
    },
    Example {
        label: "moderate branching is fine",
        code: "fn f(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} }",
        pass: true,
    },
    Example {
        label: "too many branches",
        code: "fn f(x: bool) { if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} if x {} }",
        pass: false,
    },
    Example {
        label: "mixed constructs over threshold",
        code: "fn f(x: u8) -> u8 {\n    match x { 0 => 1, 1 => 2, 2 => 3, 3 => 4, _ => 5 }\n    for _ in it { }\n    while a && b { }\n    loop { break v && w; }\n    let _ = c || d || e;\n    let _ = |z: bool| if z { 1 } else { 2 };\n    let _ = h()?;\n    let _ = p && q;\n    return x;\n}",
        pass: false,
    },
    Example {
        label: "mixed constructs at threshold",
        code: "fn f(x: u8) -> u8 {\n    match x { 0 => 1, 1 => 2, 2 => 3, 3 => 4, _ => 5 }\n    for _ in it { }\n    while a && b { }\n    loop { break v && w; }\n    let _ = c || d || e;\n    let _ = |z: bool| if z { 1 } else { 2 };\n    let _ = h()?;\n    return x;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    cyclomatic_complexity,
    "Flag functions with cyclomatic complexity > threshold.",
    "High cyclomatic complexity means many execution paths, making the function hard to test and prone to bugs.",
    Medium,
    params {
        threshold: i64 = 15
    },
);

fn check_cyclomatic_complexity(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx
        .file
        .config
        .get_usize("rust_cyclomatic_complexity", &PARAMS[0]);

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            function.body()?;
            let complexity = function
                .syntax()
                .descendants()
                .filter_map(ast::Expr::cast)
                .filter(|expression| enclosing_function(expression).as_ref() == Some(&function))
                .map(expression_complexity)
                .sum::<usize>();

            (complexity > threshold).then(|| {
                let name = function.name()?;

                Some(ctx.violation(
                    &name,
                    format!(
                        "function `{name}` has cyclomatic complexity {complexity} (max {threshold})"
                    ),
                ))
            })?
        })
        .collect()
}

fn enclosing_function<N>(node: &N) -> Option<ast::Fn>
where
    N: AstNode,
{
    node.syntax().ancestors().skip(1).find_map(ast::Fn::cast)
}

fn expression_complexity(expression: ast::Expr) -> usize {
    match expression {
        ast::Expr::IfExpr(_)
        | ast::Expr::ForExpr(_)
        | ast::Expr::WhileExpr(_)
        | ast::Expr::LoopExpr(_)
        | ast::Expr::TryExpr(_)
        | ast::Expr::ReturnExpr(_)
        | ast::Expr::LetExpr(_) => 1,
        ast::Expr::MatchExpr(expression) => expression
            .match_arm_list()
            .map_or(0, |arms| arms.arms().count().saturating_sub(1)),
        ast::Expr::BinExpr(expression) => {
            usize::from(matches!(expression.op_kind(), Some(BinaryOp::LogicOp(_))))
        }
        ast::Expr::BreakExpr(expression) => usize::from(expression.expr().is_some()),
        _ => 0,
    }
}

crate::rulewright_ast_test!(check_cyclomatic_complexity, {
    crate::example_tests!(EXAMPLES, check_cyclomatic_complexity);

    #[gtest]
    fn complex_fn_fails() -> Result<()> {
        let src = "fn f(x: bool) {
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
        }";
        let v = run(src);
        verify_eq!(v.len(), 1)?;
        verify_true!(v[0].message.contains("cyclomatic complexity 16"))?;

        Ok(())
    }

    #[gtest]
    fn at_threshold_passes() -> Result<()> {
        let src = "fn f(x: bool) {
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
            if x { }
        }";
        verify_true!(run(src).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn test_code_passes() -> Result<()> {
        let src = "#[cfg(test)]
        mod tests {
            fn f(x: bool) {
                if x { } if x { } if x { } if x { }
                if x { } if x { } if x { } if x { }
                if x { } if x { } if x { } if x { }
                if x { } if x { } if x { } if x { }
            }
        }";
        verify_true!(run(src).is_empty())?;

        Ok(())
    }
});
