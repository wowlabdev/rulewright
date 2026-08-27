use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "full path catch_unwind",
        code: "fn f() { let _ = std::panic::catch_unwind(|| {}); }",
        pass: false,
    },
    Example {
        label: "short path catch_unwind",
        code: "fn f() { let _ = panic::catch_unwind(|| {}); }",
        pass: false,
    },
    Example {
        label: "method catch_unwind",
        code: "fn f<F: Future>(fut: F) { let _ = fut.catch_unwind(); }",
        pass: false,
    },
    Example {
        label: "catch_unwind with boundary comment",
        code: "fn f() {\n    // PANIC-BOUNDARY: isolates one request; the worker restarts after any unwind\n    let _ = std::panic::catch_unwind(|| {});\n}",
        pass: true,
    },
    Example {
        label: "catch_unwind in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let _ = std::panic::catch_unwind(|| {}); }\n}",
        pass: true,
    },
    Example {
        label: "no catch_unwind",
        code: "fn f() -> Result<(), String> { Ok(()) }",
        pass: true,
    },
];

crate::ast_rule!(
    catch_unwind,
    "Require `// PANIC-BOUNDARY:` comment on `catch_unwind` calls.",
    "Catching a panic and continuing risks observing broken state. The comment must state the controlled-restart story.",
    High,
);

fn check_catch_unwind(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let calls = ctx
        .nodes::<ast::CallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let ast::Expr::PathExpr(path) = call.expr()? else {
                return None;
            };
            let path = path.path()?;

            path.segment()
                .and_then(|segment| segment.name_ref())
                .filter(|name| name.text() == "catch_unwind")
        });
    let methods = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| call.name_ref().filter(|name| name.text() == "catch_unwind"));

    calls
        .chain(methods)
        .filter_map(|name| missing_boundary(ctx, &name))
        .collect()
}

fn missing_boundary(ctx: &AstCtx<'_>, name: &ast::NameRef) -> Option<Violation> {
    let line = ctx.line_of(name);

    if !crate::infra::helpers::has_preceding_comment(ctx.file.lines, line, &["PANIC-BOUNDARY:"]) {
        return Some(ctx.violation(
            name,
            "catch_unwind without // PANIC-BOUNDARY: comment stating the controlled-restart story",
        ));
    }

    None
}

crate::rulewright_ast_test!(check_catch_unwind, {
    crate::example_tests!(EXAMPLES, check_catch_unwind);
});
