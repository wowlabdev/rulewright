use ra_ap_syntax::{
    AstNode,
    ast::{self},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "compound assign in debug_assert",
        code: "fn f() { let mut x = 0; debug_assert!({ x += 1; x > 0 }); }",
        pass: false,
    },
    Example {
        label: "simple comparison",
        code: "fn f() { debug_assert!(x > 0); }",
        pass: true,
    },
    Example {
        label: "debug_assert_eq passes",
        code: "fn f() { debug_assert_eq!(a, b); }",
        pass: true,
    },
    Example {
        label: "subtract assign in debug_assert",
        code: "fn f() { let mut x = 5; debug_assert!({ x -= 1; x > 0 }); }",
        pass: false,
    },
    Example {
        label: "compound assign in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let mut x = 0; debug_assert!({ x += 1; x > 0 }); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    assert_side_effects,
    "Ban compound assignments (`+=`, `-=`) inside `debug_assert!` macros.",
    "Compound assignments inside debug_assert! are side effects that vanish in release builds, causing silent behavior changes.",
    High,
);

const DEBUG_ASSERT_MACROS: &[&str] = &["debug_assert", "debug_assert_eq", "debug_assert_ne"];
const MAX_COMPOUND_OPERATOR_LEN: usize = 3;

fn check_assert_side_effects(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let name = macro_name(&call)?;

            if !DEBUG_ASSERT_MACROS.contains(&name.as_str()) {
                return None;
            }

            let op = has_compound_assign(&call)?;

            Some(ctx.violation(
                &call,
                format!(
                    "{name}!() contains compound assignment `{op}` — side effect lost in release builds"
                ),
            ))
        })
        .collect()
}

fn macro_name(call: &ast::MacroCall) -> Option<String> {
    call.path()?
        .segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

// #rw(fn: rust_alloc_in_loop) the punctuation window is built incrementally from syntax tokens
fn has_compound_assign(call: &ast::MacroCall) -> Option<&'static str> {
    const OPERATORS: &[&str] = &["+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="];
    let mut punctuation = String::new();

    for token in call
        .token_tree()?
        .syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
    {
        if !token.kind().is_punct()
            || matches!(token.text(), "(" | ")" | "[" | "]" | "{" | "}" | "," | ";")
        {
            punctuation.clear();
            continue;
        }

        punctuation.push_str(token.text());

        if let Some(operator) = OPERATORS
            .iter()
            .copied()
            .find(|operator| punctuation.ends_with(operator))
        {
            return Some(operator);
        }

        if punctuation.len() > MAX_COMPOUND_OPERATOR_LEN {
            punctuation.remove(0);
        }
    }

    None
}

crate::rulewright_ast_test!(check_assert_side_effects, {
    crate::example_tests!(EXAMPLES, check_assert_side_effects);
});
