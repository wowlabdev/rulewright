use ra_ap_syntax::{ast, ast::HasArgList};

use crate::{AstCtx, Example, Fix, Violation, infra::parse};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "ok_or with call",
        code: "fn f() { None::<i32>.ok_or(String::new()); }",
        pass: false,
    },
    Example {
        label: "ok_or with path",
        code: "fn f() { None::<i32>.ok_or(MyError::Static); }",
        pass: true,
    },
    Example {
        label: "ok_or with struct literal",
        code: "fn f() { None::<i32>.ok_or(MyError { id: 1 }); }",
        pass: true,
    },
    Example {
        label: "unwrap_or with call",
        code: "fn f() { None::<String>.unwrap_or(String::new()); }",
        pass: false,
    },
    Example {
        label: "unwrap_or with literal",
        code: "fn f() { None::<i32>.unwrap_or(0); }",
        pass: true,
    },
    Example {
        label: "unwrap_or with len",
        code: "fn f(v: &[u8]) { v.iter().position(|&b| b == 0).unwrap_or(v.len()); }",
        pass: true,
    },
    Example {
        label: "ok_or in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { None::<i32>.ok_or(String::new()); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    ok_or_eager,
    "Flag `.ok_or()`/`.unwrap_or()` with eagerly evaluated arguments.",
    "ok_or() and unwrap_or() eagerly evaluate their argument even on the happy path. Use the _else variant for expensive expressions.",
    Low,
    fix_ok_or_eager,
);

fn check_ok_or_eager(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let method = call.name_ref()?.text().to_string();

            if method != "ok_or" && method != "unwrap_or" {
                return None;
            }

            let arguments = call.arg_list()?;
            let mut args = arguments.args();
            let argument = args.next()?;

            if args.next().is_some() || !is_eager_expr(&argument) {
                return None;
            }

            Some(ctx.violation(
                &call,
                format!(
                    ".{method}() with eagerly evaluated argument — use .{method}_else(|| ...) instead"
                ),
            ))
        })
        .collect()
}

fn is_eager_expr(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::CallExpr(_) | ast::Expr::MacroExpr(_) => true,
        ast::Expr::MethodCallExpr(call) => {
            let Some(name) = call.name_ref().map(|name| name.text().to_string()) else {
                return false;
            };

            !matches!(
                name.as_str(),
                "len" | "is_empty" | "clone" | "to_owned" | "to_string" | "into"
            )
        }
        _ => false,
    }
}

fn fix_ok_or_eager(ctx: &AstCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.file.line(v.line)?;

    ["ok_or", "unwrap_or"].iter().find_map(|method| {
        let pattern = format!(".{method}(");
        let (before, inner, after) = parse::balanced_extract(line, &pattern)?;

        Some(Fix::replace_line(
            v.line,
            format!("{before}.{method}_else(|| {inner}){after}"),
        ))
    })
}

crate::rulewright_ast_test!(check_ok_or_eager, {
    crate::example_tests!(EXAMPLES, check_ok_or_eager);
    crate::fix_tests!(ast, check_ok_or_eager, fix_ok_or_eager);
});
