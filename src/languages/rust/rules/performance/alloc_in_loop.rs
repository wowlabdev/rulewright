#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{AstNode, ast};

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "format! in for loop",
        code: r#"fn f() { for i in 0..10 { let _ = format!("item {}", i); } }"#,
        pass: false,
    },
    Example {
        label: "to_string in while loop",
        code: "fn f() { let mut i = 0; while i < 10 { let _ = i.to_string(); i += 1; } }",
        pass: false,
    },
    Example {
        label: "push_str in loop",
        code: "fn f() { let mut s = String::new(); for _ in 0..10 { s.push_str(\"x\"); } }",
        pass: false,
    },
    Example {
        label: "format! outside loop",
        code: r#"fn f() { let _ = format!("hello"); }"#,
        pass: true,
    },
    Example {
        label: "to_string outside loop",
        code: "fn f() { let _ = 42.to_string(); }",
        pass: true,
    },
    Example {
        label: "push_str outside loop",
        code: "fn f() { let mut s = String::new(); s.push_str(\"x\"); }",
        pass: true,
    },
    Example {
        label: "format! in loop in test",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { for i in 0..10 { let _ = format!(\"x{}\", i); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    alloc_in_loop,
    "Flag `format!()`, `format_args!()`, `.to_string()`, and `.push_str()` inside loops.",
    "These allocate or format a new String each iteration. Pre-allocate, use write! to a buffer, or collect and join.",
    Medium,
);

fn check_alloc_in_loop(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let methods = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| {
            !ctx.is_in_test(call) && support::is_inside_loop_body(call) && !inside_diagnostic(call)
        })
        .filter_map(|call| {
            let method = support::method_name(&call)?;
            let message = if method == "to_string" && support::has_no_args(&call) {
                ".to_string() inside a loop — allocates each iteration"
            } else if method == "push_str" {
                ".push_str() inside a loop — consider pre-allocating or collecting and joining"
            } else {
                return None;
            };

            Some(ctx.violation(&call, message))
        });
    let macros = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| {
            !ctx.is_in_test(call) && support::is_inside_loop_body(call) && !inside_diagnostic(call)
        })
        .filter_map(|call| {
            let path = call.path()?;

            if path.qualifier().is_some() {
                return None;
            }

            let name = path.segment()?.name_ref()?.text().to_string();

            matches!(name.as_str(), "format" | "format_args")
                .then(|| ctx.violation(&call, "format!() inside a loop — allocates each iteration"))
        });

    methods.chain(macros).collect()
}

fn inside_diagnostic<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax().ancestors().skip(1).any(|ancestor| {
        if let Some(call) = ast::MethodCallExpr::cast(ancestor.clone()) {
            return call
                .name_ref()
                .is_some_and(|name| name.text() == "violation");
        }

        let function = ast::CallExpr::cast(ancestor)
            .and_then(|call| call.expr())
            .and_then(|expr| support::path_expr_name(&expr));

        function.is_some_and(|name| name == "violation")
    })
}

crate::rulewright_ast_test!(check_alloc_in_loop, {
    crate::example_tests!(EXAMPLES, check_alloc_in_loop);

    #[gtest]
    fn diagnostic_messages_may_be_built_in_loops() -> Result<()> {
        let source = r#"fn check(ctx: &Ctx) {
            for name in names {
                out.push(ctx.violation(format!("bad {name}")));
                out.push(violation(path, format!("bad {name}")));
            }
        }"#;
        verify_true!(run(source).is_empty())?;

        Ok(())
    }
});
