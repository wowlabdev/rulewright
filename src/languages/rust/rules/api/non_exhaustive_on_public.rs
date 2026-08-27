use ra_ap_syntax::ast::{self, HasAttrs, HasName, HasVisibility, VisibilityKind};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "public enum without non_exhaustive",
        code: "pub enum Color { Red, Green, Blue }",
        pass: false,
    },
    Example {
        label: "public enum with non_exhaustive",
        code: "#[non_exhaustive]\npub enum Color { Red, Green, Blue }",
        pass: true,
    },
    Example {
        label: "private enum without non_exhaustive",
        code: "enum Color { Red, Green, Blue }",
        pass: true,
    },
    Example {
        label: "pub(crate) enum without non_exhaustive",
        code: "pub(crate) enum Color { Red, Green, Blue }",
        pass: true,
    },
    Example {
        label: "public enum in test",
        code: "#[cfg(test)]\nmod tests {\n    pub enum Color { Red, Green, Blue }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    non_exhaustive_on_public,
    "Flag public enums without `#[non_exhaustive]` — prevents breaking changes when adding variants.",
    "Adding a variant to a public enum is a breaking change. #[non_exhaustive] lets you add variants without a major version bump.",
    Medium,
);

fn check_non_exhaustive_on_public(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let public_enums = ctx
        .nodes::<ast::Enum>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| {
            item.visibility()
                .is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
        });

    public_enums
        .filter(|item| {
            !item
                .attrs()
                .any(|attr| attr.simple_name().as_deref() == Some("non_exhaustive"))
        })
        .filter_map(|item| {
            let name = item.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "public enum `{name}` should have `#[non_exhaustive]` to allow adding variants without breaking downstream"
                ),
            ))
        })
        .collect()
}

crate::rulewright_ast_test!(check_non_exhaustive_on_public, {
    crate::example_tests!(EXAMPLES, check_non_exhaustive_on_public);
});
