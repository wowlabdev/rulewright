use ra_ap_syntax::{AstNode, ast};

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "bounded async for loop",
        code: "async fn f(xs: &[u32]) { for x in xs { process(*x); } }",
        pass: true,
    },
    Example {
        label: "loop in async block without await",
        code: "fn g() { let _fut = async { loop { step(); } }; }",
        pass: false,
    },
    Example {
        label: "bounded for loop with several statements",
        code: "async fn f(xs: &[u32]) { for x in xs { let a = *x; let b = a + 1; let _c = b - a; } }",
        pass: true,
    },
    Example {
        label: "loop yields via yield_now().await",
        code: "async fn f(xs: &[u32]) { for x in xs { process(*x); tokio::task::yield_now().await; } }",
        pass: true,
    },
    Example {
        label: "sync fn loop is fine",
        code: "fn f(xs: &[u32]) { for x in xs { process(*x); } }",
        pass: true,
    },
    Example {
        label: "async while loop without yield",
        code: "async fn f(mut ready: bool) { while ready { ready = poll(); } }",
        pass: false,
    },
    Example {
        label: "tiny accumulation loop without calls",
        code: "async fn f(xs: &[u32]) -> u32 { let mut total = 0; for x in xs { total += x; } total }",
        pass: true,
    },
    Example {
        label: "loop inside sync closure in async fn",
        code: "async fn f(xs: &[u32]) { let sum = || { for x in xs { process(*x); } }; sum(); }",
        pass: true,
    },
    Example {
        label: "await nested deeper in loop body",
        code: "async fn f(n: u32) { for _ in 0..n { if n > 1 { step().await; } } }",
        pass: true,
    },
    Example {
        label: "async loop in test module",
        code: "#[cfg(test)]\nmod tests {\n    async fn t(xs: &[u32]) { for x in xs { process(*x); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    async_loop_no_yield,
    "Flag potentially unbounded `loop`/`while` work in async contexts whose bodies never `.await`.",
    "Potentially unbounded CPU work in an async task must cooperate with the runtime or move to a blocking/worker boundary. Finite `for` traversals are not enough evidence by themselves: chunk or yield those only when their input size and measured runtime justify it.",
);

fn check_async_loop_no_yield(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for node in ctx.nodes::<ast::WhileExpr>() {
        check_loop(ctx, &node, &mut violations);
    }

    for node in ctx.nodes::<ast::LoopExpr>() {
        check_loop(ctx, &node, &mut violations);
    }

    violations
}

const MIN_LOOP_BODY_STMTS: usize = 3;

const MSG: &str = "loop in async context has no .await — CPU-bound iterations starve the runtime; add yield_now().await between chunks";

fn check_loop<N>(ctx: &AstCtx<'_>, node: &N, violations: &mut Vec<Violation>)
where
    N: AstNode,
{
    if ctx.is_in_test(node) || !support::is_in_async_context(node) {
        return;
    }

    let Some(body) = support::loop_body(node) else {
        return;
    };
    let has_await = body
        .syntax()
        .descendants()
        .any(|syntax| ast::AwaitExpr::can_cast(syntax.kind()));

    if has_await {
        return;
    }

    let has_call = body.syntax().descendants().any(|syntax| {
        ast::CallExpr::can_cast(syntax.kind()) || ast::MethodCallExpr::can_cast(syntax.kind())
    });
    let statements = body
        .stmt_list()
        .map_or(0, |statements| statements.statements().count());

    if has_call || statements >= MIN_LOOP_BODY_STMTS {
        violations.push(ctx.violation(node, MSG));
    }
}

crate::rulewright_ast_test!(check_async_loop_no_yield, {
    crate::example_tests!(EXAMPLES, check_async_loop_no_yield);
});
