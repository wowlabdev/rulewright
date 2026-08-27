use ra_ap_syntax::{
    AstNode,
    ast::{self, HasLoopBody},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "info in for loop",
        code: "fn f(v: Vec<u32>) { for m in v { tracing::info!(\"processing\"); } }",
        pass: false,
    },
    Example {
        label: "warn in while loop",
        code: "fn f() { while running() { warn!(\"still waiting\"); } }",
        pass: false,
    },
    Example {
        label: "event in loop",
        code: "fn f() { loop { event!(Level::TRACE, \"tick\"); } }",
        pass: false,
    },
    Example {
        label: "debug in nested block in loop",
        code: "fn f(v: Vec<u32>) { for m in v { if m > 0 { log::debug!(\"item\"); } } }",
        pass: false,
    },
    Example {
        label: "log outside loop",
        code: "fn f() { tracing::info!(\"batch done\"); }",
        pass: true,
    },
    Example {
        label: "batch event before loop",
        code: "fn f(v: Vec<u32>) { info!(\"processing batch\"); for m in v { handle(m); } }",
        pass: true,
    },
    Example {
        label: "non-log macro in loop",
        code: "fn f(v: Vec<u32>) { for m in v { black_box!(m); } }",
        pass: true,
    },
    Example {
        label: "log in loop in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(v: Vec<u32>) { for m in v { tracing::info!(\"x\"); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    log_in_loop,
    "Flag logging macro invocations inside loop bodies in library code.",
    "Per-iteration telemetry turns hot loops into allocation and I/O hotspots; emit one batch-level event before or after the loop instead.",
    Low,
);

const LOG_MACROS: &[&str] = &["debug", "error", "event", "info", "log", "trace", "warn"];

fn macro_name(call: &ast::MacroCall) -> Option<String> {
    call.path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

fn check_log_in_loop(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let log_calls = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter(|call| macro_name(call).is_some_and(|name| LOG_MACROS.contains(&name.as_str())));

    log_calls
        .filter(is_inside_loop)
        .map(|call| {
            ctx.violation(
                &call,
                "logging macro inside a loop — emit one batch-level event instead of per-iteration telemetry",
            )
        })
        .collect()
}

fn is_inside_loop(call: &ast::MacroCall) -> bool {
    call.syntax().ancestors().skip(1).any(|ancestor| {
        if let Some(for_expr) = ast::ForExpr::cast(ancestor.clone()) {
            return for_expr.loop_body().is_some_and(|body| {
                body.syntax()
                    .text_range()
                    .contains_range(call.syntax().text_range())
            });
        }

        ast::WhileExpr::can_cast(ancestor.kind()) || ast::LoopExpr::can_cast(ancestor.kind())
    })
}

crate::rulewright_ast_test!(check_log_in_loop, {
    crate::example_tests!(EXAMPLES, check_log_in_loop);
});
