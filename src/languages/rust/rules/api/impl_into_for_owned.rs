use ra_ap_syntax::ast;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "impl Into directly",
        code: "struct Foo;\nimpl Into<String> for Foo { fn into(self) -> String { String::new() } }",
        pass: false,
    },
    Example {
        label: "impl From instead",
        code: "struct Foo;\nimpl From<Foo> for String { fn from(_: Foo) -> String { String::new() } }",
        pass: true,
    },
    Example {
        label: "impl Into in test",
        code: "#[cfg(test)]\nmod tests {\n    struct Foo;\n    impl Into<String> for Foo { fn into(self) -> String { String::new() } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    impl_into_for_owned,
    "Flag `impl Into<T> for X` — implement `From<X> for T` instead (gives Into for free).",
    "Implementing From<X> for T gives you Into<T> for X for free. Implementing Into directly is redundant and non-standard.",
    Medium,
);

fn check_impl_into_for_owned(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Impl>()
        .filter(|item| !ctx.is_in_test(item))
        .filter_map(|item| {
            let ast::Type::PathType(path) = item.trait_()? else {
                return None;
            };
            let name = path.path()?.segment()?.name_ref()?;

            (name.text() == "Into").then(|| {
                ctx.violation(
                    &name,
                    "implement `From` instead of `Into` — From gives you Into for free",
                )
            })
        })
        .collect()
}

crate::rulewright_ast_test!(check_impl_into_for_owned, {
    crate::example_tests!(EXAMPLES, check_impl_into_for_owned);
});
