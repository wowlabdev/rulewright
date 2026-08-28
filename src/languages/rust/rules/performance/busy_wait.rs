use ra_ap_syntax::{AstNode, ast};

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "try_recv spin loop without sleep",
        code: "fn f(rx: std::sync::mpsc::Receiver<u8>) { loop { if let Ok(v) = rx.try_recv() { drop(v); } } }",
        pass: false,
    },
    Example {
        label: "while spinning on atomic load condition",
        code: "fn f(flag: &std::sync::atomic::AtomicBool) { while flag.load(std::sync::atomic::Ordering::Acquire) {} }",
        pass: false,
    },
    Example {
        label: "compare_exchange spin lock",
        code: "fn f(lock: &std::sync::atomic::AtomicBool) { loop { match lock.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed) { Ok(_) => break, Err(_) => {} } } }",
        pass: false,
    },
    Example {
        label: "while spinning on try_lock condition",
        code: "fn f(m: &std::sync::Mutex<u8>) { while m.try_lock().is_err() {} }",
        pass: false,
    },
    Example {
        label: "try_recv loop with sleep",
        code: "fn f(rx: std::sync::mpsc::Receiver<u8>) { loop { if let Ok(v) = rx.try_recv() { drop(v); } std::thread::sleep(std::time::Duration::from_millis(1)); } }",
        pass: true,
    },
    Example {
        label: "async poll loop with yield_now",
        code: "async fn f(q: &Queue) { loop { if q.try_recv().is_none() { tokio::task::yield_now().await; } } }",
        pass: true,
    },
    Example {
        label: "try_lock loop with park",
        code: "fn f(m: &std::sync::Mutex<u8>) { loop { if let Ok(g) = m.try_lock() { drop(g); break; } std::thread::park(); } }",
        pass: true,
    },
    Example {
        label: "blocking recv is not a spin",
        code: "fn f(rx: std::sync::mpsc::Receiver<u8>) { loop { let Ok(v) = rx.recv() else { break }; drop(v); } }",
        pass: true,
    },
    Example {
        label: "plain computation loop",
        code: "fn f(mut n: u32) { while n > 0 { n -= 1; } }",
        pass: true,
    },
    Example {
        label: "spin loop in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t(rx: std::sync::mpsc::Receiver<u8>) { loop { if let Ok(v) = rx.try_recv() { drop(v); } } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    busy_wait,
    "Flag spin loops polling `try_recv`/`try_lock`/atomics without sleeping, yielding, or blocking.",
    "Hot spinning burns CPU cycles when no work is present. Sleep, yield_now, park, or block on recv() between polls.",
    Medium,
);

fn check_busy_wait(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let loops = ctx.nodes::<ast::LoopExpr>().filter_map(|node| {
        if ctx.is_in_test(&node) {
            return None;
        }

        let body = support::loop_body(&node).map(|body| SpinScan::of_node(body.syntax()))?;

        (body.polls && !body.waits).then(|| ctx.violation(&node, MSG))
    });
    let whiles = ctx.nodes::<ast::WhileExpr>().filter_map(|node| {
        if ctx.is_in_test(&node) {
            return None;
        }

        let body = support::loop_body(&node).map(|body| SpinScan::of_node(body.syntax()))?;
        let condition = support::loop_source(&node)
            .map_or_else(SpinScan::default, |expr| SpinScan::of_node(expr.syntax()));
        let polling = body.polls || condition.polls || condition.loads;

        (polling && !(body.waits || condition.waits)).then(|| ctx.violation(&node, MSG))
    });

    loops.chain(whiles).collect()
}

const POLL_METHODS: &[&str] = &[
    "compare_exchange",
    "compare_exchange_weak",
    "try_lock",
    "try_read",
    "try_recv",
    "try_write",
];
const WAIT_FNS: &[&str] = &["park", "sleep", "yield_now"];

const MSG: &str = "busy-wait loop polls without sleeping or yielding — sleep, yield_now, park, or block on recv() when idle";

#[derive(Default)]
struct SpinScan {
    loads: bool,
    polls: bool,
    waits: bool,
}

impl SpinScan {
    fn of_node(node: &ra_ap_syntax::SyntaxNode) -> Self {
        let mut scan = Self::default();

        for syntax in node.descendants() {
            if ast::AwaitExpr::can_cast(syntax.kind()) {
                scan.waits = true;
            }

            if let Some(call) = ast::CallExpr::cast(syntax.clone())
                && call
                    .expr()
                    .and_then(|expr| support::path_expr_last_name(&expr))
                    .is_some_and(|name| WAIT_FNS.contains(&name.as_str()))
            {
                scan.waits = true;
            }

            if let Some(call) = ast::MethodCallExpr::cast(syntax) {
                let Some(method) = support::method_name(&call) else {
                    continue;
                };

                if POLL_METHODS.contains(&method.as_str()) {
                    scan.polls = true;
                }

                if method == "load" {
                    scan.loads = true;
                }

                if method == "recv" && support::has_no_args(&call) {
                    scan.waits = true;
                }
            }
        }

        scan
    }
}

crate::rulewright_ast_test!(check_busy_wait, {
    crate::example_tests!(EXAMPLES, check_busy_wait);
});
