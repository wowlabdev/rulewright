#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{ast, ast::HasArgList};

use super::support::is_inside_loop_body;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "clone in for loop",
        code: "fn f(v: Vec<String>) { for x in &v { let y = x.clone(); } }",
        pass: false,
    },
    Example {
        label: "clone in while loop",
        code: "fn f(s: &String) { while true { let y = s.clone(); } }",
        pass: false,
    },
    Example {
        label: "clone in loop loop",
        code: "fn f(s: &String) { loop { let y = s.clone(); break; } }",
        pass: false,
    },
    Example {
        label: "clone outside loop",
        code: "fn f(s: &String) { let y = s.clone(); }",
        pass: true,
    },
    Example {
        label: "clone in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(v: Vec<String>) { for x in &v { let y = x.clone(); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    clone_in_loop,
    "Flag `.clone()` calls inside loop bodies (potential O(n) allocations).",
    "Cloning inside a loop allocates on every iteration. Borrow or restructure to avoid O(n) heap allocations. \
     Note: this rule has no type information, so it flags all .clone() calls including cheap ones (Arc, Rc, Copy types). \
     Suppress with `// #rw(rust_clone_in_loop) Arc clone is O(1)` when the clone is intentionally cheap.",
    Medium,
);

fn check_clone_in_loop(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let clone_calls = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call) && is_inside_loop_body(call))
        .filter(|call| {
            call.name_ref().is_some_and(|name| name.text() == "clone")
                && call
                    .arg_list()
                    .is_none_or(|arguments| arguments.args().next().is_none())
        });

    clone_calls
        .map(|call| {
            ctx.violation(
                &call,
                ".clone() inside a loop — consider borrowing or restructuring to avoid repeated clones",
            )
        })
        .collect()
}

crate::rulewright_ast_test!(check_clone_in_loop, {
    crate::example_tests!(EXAMPLES, check_clone_in_loop);

    #[gtest]
    fn clone_in_for_iterator_is_evaluated_once() -> Result<()> {
        verify_true!(run("fn f(tokens: Tokens) { for token in tokens.clone() {} }").is_empty())?;

        Ok(())
    }
});
