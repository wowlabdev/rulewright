use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "process::exit in library",
        code: "fn f() { std::process::exit(1); }",
        pass: false,
    },
    Example {
        label: "no exit call",
        code: "fn f() { let exit = 0; }",
        pass: true,
    },
    Example {
        label: "exit in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { std::process::exit(1); }\n}",
        pass: true,
    },
    Example {
        label: "custom exit function",
        code: "fn f() { exit(0); }",
        pass: true,
    },
];

crate::ast_rule!(
    deep_exit,
    "Ban `std::process::exit()` in library code.",
    "process::exit() skips destructors and cleanup. Return Result from main instead so resources are released properly.",
    High,
);

fn check_deep_exit(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(path_expr) = call.expr()? else {
                return None;
            };
            let path = path_expr.path()?;
            let source = path.syntax().text().to_string();

            (source.split("::").any(|segment| segment == "process")
                && path
                    .segment()
                    .and_then(|segment| segment.name_ref())
                    .is_some_and(|name| name.text() == "exit"))
            .then(|| {
                ctx.violation(
                    &path,
                    "process::exit() in library code (return Result instead)",
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_deep_exit, {
    crate::example_tests!(EXAMPLES, check_deep_exit);
});
