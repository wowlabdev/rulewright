use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "bare forget without comment",
        code: "fn f() { std::mem::forget(String::new()); }",
        pass: false,
    },
    Example {
        label: "forget with LEAK comment",
        code: "fn f() {\n    // LEAK: intentionally leaked for static lifetime\n    std::mem::forget(String::new());\n}",
        pass: true,
    },
    Example {
        label: "forget with SAFETY comment",
        code: "fn f() {\n    // SAFETY: ownership transferred to FFI\n    std::mem::forget(String::new());\n}",
        pass: true,
    },
    Example {
        label: "forget in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { std::mem::forget(String::new()); }\n}",
        pass: true,
    },
    Example {
        label: "unrelated forget function",
        code: "fn f() { cache::forget(key); }",
        pass: true,
    },
];

crate::ast_rule!(
    mem_forget,
    "Require `LEAK` or `SAFETY` comment on `std::mem::forget()` calls.",
    "mem::forget permanently leaks memory. A justification comment proves the leak is intentional, not a bug.",
    High,
);

fn check_mem_forget(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(path_expr) = call.expr()? else {
                return None;
            };
            let path = path_expr.path()?;
            let source = path.syntax().text().to_string();
            let is_forget = path
                .segment()
                .and_then(|segment| segment.name_ref())
                .is_some_and(|name| name.text() == "forget")
                && source.split("::").any(|segment| segment == "mem");

            if !is_forget {
                return None;
            }

            let line = ctx.line_of(&path);

            (!crate::infra::helpers::has_preceding_comment(
                ctx.file.lines,
                line,
                &["LEAK:", "SAFETY:"],
            ))
            .then(|| {
                ctx.violation(
                    &path,
                    "std::mem::forget without // LEAK: or // SAFETY: comment on a preceding line",
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_mem_forget, {
    crate::example_tests!(EXAMPLES, check_mem_forget);
});
