#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "assert without message",
        code: "fn f() { assert!(true); }",
        pass: false,
    },
    Example {
        label: "assert with message",
        code: "fn f() { assert!(true, \"reason\"); }",
        pass: true,
    },
    Example {
        label: "assert_eq without message",
        code: "fn f() { assert_eq!(1, 2); }",
        pass: false,
    },
    Example {
        label: "assert_eq with message",
        code: "fn f() { assert_eq!(1, 2, \"values differ\"); }",
        pass: true,
    },
    Example {
        label: "assert_ne without message",
        code: "fn f() { assert_ne!(1, 1); }",
        pass: false,
    },
    Example {
        label: "assert_ne with message",
        code: "fn f() { assert_ne!(1, 1, \"should differ\"); }",
        pass: true,
    },
    Example {
        label: "assert in test module",
        code: "#[cfg(test)]\nmod tests {\n  fn t() { assert!(true); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    missing_assert_message,
    "Require a message argument on `assert!`, `assert_eq!`, `assert_ne!`.",
    "Assertions without messages produce opaque failures. State the expected invariant and include the values needed to diagnose it; a message that merely paraphrases the condition adds no useful context.",
);

const COMPARISON_ASSERT_ARGUMENTS: usize = 2;

fn check_missing_assert_message(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let name = macro_name(&call)?;
            let needed = match name.as_str() {
                "assert" => 1,
                "assert_eq" | "assert_ne" => COMPARISON_ASSERT_ARGUMENTS,
                _ => return None,
            };

            (top_level_comma_count(&call) < needed).then(|| {
                ctx.violation(
                    &call,
                    format!("{name}!() without a message — add a description of what failed"),
                )
            })
        })
        .collect()
}

fn macro_name(call: &ast::MacroCall) -> Option<String> {
    call.path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

fn top_level_comma_count(call: &ast::MacroCall) -> usize {
    call.token_tree()
        .into_iter()
        .flat_map(|tree| tree.syntax().children_with_tokens())
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| token.text() == ",")
        .count()
}

crate::rulewright_ast_test!(check_missing_assert_message, {
    crate::example_tests!(EXAMPLES, check_missing_assert_message);

    #[gtest]
    fn nested_commas_not_counted() -> Result<()> {
        let v = run("fn f() { assert!(vec![1, 2].contains(&1)); }");
        verify_eq!(v.len(), 1)?;

        Ok(())
    }
});
