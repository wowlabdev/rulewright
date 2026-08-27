use ra_ap_syntax::ast::{self, HasArgList, LiteralKind};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "empty expect message",
        code: "fn f() { Some(1).expect(\"\"); }",
        pass: false,
    },
    Example {
        label: "generic expect message",
        code: "fn f() { Some(1).expect(\"failed\"); }",
        pass: false,
    },
    Example {
        label: "good expect message",
        code: "fn f() { Some(1).expect(\"config file must exist at /etc/app.conf\"); }",
        pass: true,
    },
    Example {
        label: "expect in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { Some(1).expect(\"\"); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    expect_message,
    "Require `.expect()` to have a meaningful message, not generic ones.",
    "Generic expect messages like 'failed' give no context in panics. Describe what was expected and why.",
);

const GENERIC_MESSAGES: &[&str] = &[
    "",
    "failed",
    "error",
    "unwrap",
    "should not happen",
    "unreachable",
    "impossible",
    "bug",
];

fn check_expect_message(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let method = call.name_ref()?;

            if method.text() != "expect" {
                return None;
            }

            let ast::Expr::Literal(literal) = call.arg_list()?.args().next()? else {
                return None;
            };
            let LiteralKind::String(text) = literal.kind() else {
                return None;
            };
            let message = text.value().ok()?.into_owned();
            let lower = message.trim().to_lowercase();

            GENERIC_MESSAGES
                .iter()
                .any(|generic| lower == *generic)
                .then(|| {
                    ctx.violation(
                        &method,
                        format!(
                            ".expect() with generic message {message:?} (provide a specific reason)"
                        ),
                    )
                })
        })
        .collect()
}

crate::rulewright_ast_test!(check_expect_message, {
    crate::example_tests!(EXAMPLES, check_expect_message);
});
