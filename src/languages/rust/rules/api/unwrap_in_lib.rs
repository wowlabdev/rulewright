use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unwrap in library",
        code: "fn f() { Some(1).unwrap(); }",
        pass: false,
    },
    Example {
        label: "expect passes",
        code: "fn f() { Some(1).expect(\"reason\"); }",
        pass: true,
    },
    Example {
        label: "unwrap in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { Some(1).unwrap(); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    unwrap_in_lib,
    "Ban `.unwrap()` in library code.",
    "unwrap() in library code panics the caller with no context. Return Result or use expect() with a message.",
    Medium,
);

fn check_unwrap_in_lib(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let method = call.name_ref()?;

            (method.text() == "unwrap").then(|| {
                ctx.violation(
                    &method,
                    ".unwrap() in library code (use .expect(\"reason\") or propagate with ?)",
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_unwrap_in_lib, {
    crate::example_tests!(EXAMPLES, check_unwrap_in_lib);
});
