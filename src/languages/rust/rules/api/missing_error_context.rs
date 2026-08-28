use ra_ap_syntax::{AstNode, ast, ast::HasArgList, ast::HasAttrs};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "wildcard discard",
        code: r#"fn f() { let _ = Ok::<i32, i32>(1).map_err(|_| "bad"); }"#,
        pass: false,
    },
    Example {
        label: "named param",
        code: r#"fn f() { let _ = Ok::<i32, i32>(1).map_err(|e| format!("{e}")); }"#,
        pass: true,
    },
    Example {
        label: "underscore-prefixed param",
        code: r#"fn f() { let _ = Ok::<i32, i32>(1).map_err(|_e| "bad"); }"#,
        pass: false,
    },
    Example {
        label: "function ref",
        code: "fn f() { let _ = Ok::<i32, String>(1).map_err(String::from); }",
        pass: true,
    },
    Example {
        label: "documented boundary error normalization",
        code: "#[expect(clippy::map_err_ignore, reason = \"wire contract exposes stable error categories\")] fn f() { let _ = Ok::<i32, i32>(1).map_err(|_| \"bad\"); }",
        pass: true,
    },
    Example {
        label: "discard in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let _ = Ok::<i32, i32>(1).map_err(|_| \"bad\"); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    missing_error_context,
    "Flag `.map_err(|_| ...)` that discards the original error.",
    "Discarding the original error in map_err(|_| ...) destroys the root cause, making failures hard to diagnose.",
    Medium,
);

fn check_missing_error_context(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call) && !has_documented_normalization(call))
        .filter_map(|call| {
            let method = call.name_ref()?;

            if method.text() != "map_err" {
                return None;
            }

            let closure = call.arg_list()?.args().next().and_then(|arg| match arg {
                ast::Expr::ClosureExpr(closure) => Some(closure),
                _ => None,
            })?;
            let param = closure.param_list()?.syntax().text().to_string();
            let binding = param.trim().trim_matches('|').trim();

            (binding == "_" || binding.starts_with('_')).then(|| {
                ctx.violation(
                    &method,
                    ".map_err(|_| ...) discards the original error — use the error variable",
                )
            })
        })
        .collect()
}

fn has_documented_normalization(call: &ast::MethodCallExpr) -> bool {
    call.syntax()
        .ancestors()
        .find_map(ast::Fn::cast)
        .is_some_and(|function| {
            function.attrs().any(|attribute| {
                let source = attribute.syntax().text().to_string();

                source.contains("expect(")
                    && source.contains("clippy::map_err_ignore")
                    && source.contains("reason")
            })
        })
}

crate::rulewright_ast_test!(check_missing_error_context, {
    crate::example_tests!(EXAMPLES, check_missing_error_context);
});
