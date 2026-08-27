use ra_ap_syntax::{ast, ast::LiteralKind};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "variable index",
        code: "fn f(v: Vec<i32>, i: usize) { let _ = v[i]; }",
        pass: false,
    },
    Example {
        label: "literal index",
        code: "fn f(v: Vec<i32>) { let _ = v[0]; }",
        pass: true,
    },
    Example {
        label: "indexing in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(v: Vec<i32>, i: usize) { let _ = v[i]; }\n}",
        pass: true,
    },
    Example {
        label: "with BOUNDS comment",
        code: "fn f(v: Vec<i32>, i: usize) {\n    // BOUNDS: index validated by caller\n    let _ = v[i];\n}",
        pass: true,
    },
];

crate::ast_rule!(
    unchecked_indexing,
    "Flag `container[expr]` indexing with non-literal indices.",
    "Indexing with a variable panics on out-of-bounds. Use .get() or add a // BOUNDS: comment explaining why the index is safe.",
    Low,
);

fn check_unchecked_indexing(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::IndexExpr>()
        .filter(|index| !ctx.is_in_test(index))
        .filter_map(|index| {
            let expr = index.index()?;

            if matches!(expr, ast::Expr::Literal(ref literal) if matches!(literal.kind(), LiteralKind::IntNumber(_))) {
                return None;
            }

            let line = ctx.line_of(&expr);

            (!crate::infra::helpers::has_preceding_comment(
                ctx.file.lines,
                line,
                &["BOUNDS:"],
            ))
            .then(|| {
                ctx.violation(
                    &expr,
                    "unchecked indexing — prefer `.get()` or add `// BOUNDS:` comment",
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_unchecked_indexing, {
    crate::example_tests!(EXAMPLES, check_unchecked_indexing);
});
