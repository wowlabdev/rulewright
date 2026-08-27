use ra_ap_syntax::ast::{self, HasName};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "get_ method",
        code: "struct S;\nimpl S {\n    fn get_name(&self) {}\n}",
        pass: false,
    },
    Example {
        label: "get_ trait method",
        code: "trait T {\n    fn get_len(&self) -> usize;\n}",
        pass: false,
    },
    Example {
        label: "plain get passes",
        code: "struct S;\nimpl S {\n    fn get(&self) {}\n}",
        pass: true,
    },
    Example {
        label: "get_mut passes",
        code: "struct S;\nimpl S {\n    fn get_mut(&mut self) {}\n}",
        pass: true,
    },
    Example {
        label: "get_unchecked_mut passes",
        code: "struct S;\nimpl S {\n    fn get_unchecked_mut(&mut self) {}\n}",
        pass: true,
    },
    Example {
        label: "get_or_insert_with passes",
        code: "struct S;\nimpl S {\n    fn get_or_insert_with(&mut self) {}\n}",
        pass: true,
    },
    Example {
        label: "free fn is not a getter",
        code: "fn get_config() {}",
        pass: true,
    },
    Example {
        label: "associated fn without receiver",
        code: "struct S;\nimpl S {\n    fn get_default() -> S { S }\n}",
        pass: true,
    },
    Example {
        label: "getter in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct S;\n    impl S {\n        fn get_name(&self) {}\n    }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    getter_prefix,
    "Flag methods named `get_something` — Rust getters are named after the field (C-GETTER).",
    "The `get_` prefix is noise: the std convention is `fn name(&self)`, with `get`/`get_mut` reserved for keyed or checked access.",
    Low,
);

const ALLOWED: &[&str] = &[
    "get",
    "get_mut",
    "get_unchecked",
    "get_unchecked_mut",
    "get_or_insert_with",
    "get_or_init",
];

fn check_getter_prefix(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Fn>()
        .filter(|function| {
            !ctx.is_in_test(function)
                && function
                    .param_list()
                    .is_some_and(|params| params.self_param().is_some())
        })
        .filter_map(|function| {
            let name = function.name()?;
            let name_text = name.text();
            let field = name_text.strip_prefix("get_")?;

            (!field.is_empty() && !ALLOWED.contains(&name_text.as_str())).then(|| {
                ctx.violation(
                    &name,
                    format!(
                        "getter `{name_text}` — name it `{field}` after what it returns (C-GETTER)"
                    ),
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_getter_prefix, {
    crate::example_tests!(EXAMPLES, check_getter_prefix);
});
