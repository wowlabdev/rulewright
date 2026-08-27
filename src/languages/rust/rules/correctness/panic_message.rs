use ra_ap_syntax::{AstNode, AstToken, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unreachable without message",
        code: "fn f() { unreachable!(); }",
        pass: false,
    },
    Example {
        label: "debug_assert without message",
        code: "fn f(x: u8) { debug_assert!(x > 0); }",
        pass: false,
    },
    Example {
        label: "debug_assert_eq without message",
        code: "fn f(a: u8, b: u8) { debug_assert_eq!(a, b); }",
        pass: false,
    },
    Example {
        label: "debug_assert_ne without message",
        code: "fn f(a: u8, b: u8) { debug_assert_ne!(a, b); }",
        pass: false,
    },
    Example {
        label: "unreachable with empty message",
        code: "fn f() { unreachable!(\"\"); }",
        pass: false,
    },
    Example {
        label: "unreachable with descriptive message",
        code: "fn f(m: u8) { unreachable!(\"month {m} out of range after validation\"); }",
        pass: true,
    },
    Example {
        label: "unreachable with short message",
        code: "fn f() { unreachable!(\"parser state was initialized before input\"); }",
        pass: true,
    },
    Example {
        label: "debug_assert with message",
        code: "fn f(x: u8) { debug_assert!(x > 0, \"x must be positive, got {x}\"); }",
        pass: true,
    },
    Example {
        label: "debug_assert_eq with message",
        code: "fn f(a: u8, b: u8) { debug_assert_eq!(a, b, \"lengths must match\"); }",
        pass: true,
    },
    Example {
        label: "plain assert not covered",
        code: "fn f(x: u8) { assert!(x > 0); }",
        pass: true,
    },
    Example {
        label: "unreachable in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { unreachable!(); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    panic_message,
    "Require a message on `unreachable!` and `debug_assert!*`.",
    "Panic messages must state what went wrong; a missing or empty message gives the developer nothing to act on.",
    Medium,
);

const COMPARISON_ASSERT_ARGUMENTS: usize = 2;

fn check_panic_message(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let name = macro_name(&call)?;
            let is_unreachable = name == "unreachable";
            let is_debug_assert = name == "debug_assert";
            let is_debug_assert_cmp = matches!(name.as_str(), "debug_assert_eq" | "debug_assert_ne");
            let message = if is_unreachable && macro_tokens(&call).next().is_none() {
                Some("unreachable!() without a message — state why this branch is impossible".to_owned())
            } else if is_debug_assert || is_debug_assert_cmp {
                let needed = if is_debug_assert {
                    1
                } else {
                    COMPARISON_ASSERT_ARGUMENTS
                };

                (message_comma_count(&call) < needed).then(|| {
                    "debug_assert without a message — describe the violated invariant".to_owned()
                })
            } else if is_unreachable {
                first_string_literal(&call).and_then(|message| {
                    is_empty_message(&message).then(|| {
                        format!(
                            "unreachable! message {message:?} is empty — state why this branch is impossible"
                        )
                    })
                })
            } else {
                None
            }?;

            Some(ctx.violation(&call, message))
        })
        .collect()
}

fn message_comma_count(call: &ast::MacroCall) -> usize {
    let tokens: Vec<_> = macro_tokens(call).collect();
    let count = tokens.iter().filter(|token| token.text() == ",").count();

    if tokens.last().is_some_and(|token| token.text() == ",") {
        count - 1
    } else {
        count
    }
}

fn first_string_literal(call: &ast::MacroCall) -> Option<String> {
    ast::String::cast(macro_tokens(call).next()?)?
        .value()
        .ok()
        .map(std::borrow::Cow::into_owned)
}

fn is_empty_message(msg: &str) -> bool {
    msg.trim().is_empty()
}

fn macro_name(call: &ast::MacroCall) -> Option<String> {
    call.path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

fn macro_tokens(call: &ast::MacroCall) -> impl Iterator<Item = ra_ap_syntax::SyntaxToken> + '_ {
    let tokens = call
        .token_tree()
        .into_iter()
        .flat_map(|tree| tree.syntax().children_with_tokens())
        .filter_map(ra_ap_syntax::NodeOrToken::into_token);

    tokens
        .filter(|token| !token.kind().is_trivia())
        .filter(|token| !matches!(token.text(), "(" | ")" | "[" | "]" | "{" | "}"))
}

crate::rulewright_ast_test!(check_panic_message, {
    crate::example_tests!(EXAMPLES, check_panic_message);
});
