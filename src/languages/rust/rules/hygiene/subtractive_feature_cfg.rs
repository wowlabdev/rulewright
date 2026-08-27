use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasVisibility},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "not-feature on pub fn",
        code: "#[cfg(not(feature = \"std\"))]\npub fn fallback() {}",
        pass: false,
    },
    Example {
        label: "not-feature on pub struct",
        code: "#[cfg(not(feature = \"alloc\"))]\npub struct Fallback;",
        pass: false,
    },
    Example {
        label: "not-feature on pub impl fn",
        code: "struct S;\nimpl S {\n    #[cfg(not(feature = \"std\"))]\n    pub fn fallback(&self) {}\n}",
        pass: false,
    },
    Example {
        label: "positive feature gate",
        code: "#[cfg(feature = \"std\")]\npub fn f() {}",
        pass: true,
    },
    Example {
        label: "not-feature on private fn",
        code: "#[cfg(not(feature = \"std\"))]\nfn fallback() {}",
        pass: true,
    },
    Example {
        label: "not-feature on pub(crate) fn",
        code: "#[cfg(not(feature = \"std\"))]\npub(crate) fn fallback() {}",
        pass: true,
    },
    Example {
        label: "cfg not test is not a feature",
        code: "#[cfg(not(target_arch = \"wasm32\"))]\npub fn f() {}",
        pass: true,
    },
];

crate::ast_rule!(
    subtractive_feature_cfg,
    "Flag `#[cfg(not(feature = \"...\"))]` on `pub` items — features must be additive.",
    "A feature that removes public surface when enabled breaks feature unification: any dependent enabling it silently changes the API for everyone else.",
    Medium,
);

fn is_not_feature_cfg(attr: &ast::Attr) -> bool {
    attr.simple_name().is_some_and(|name| name == "cfg")
        && attr
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
            .contains("not(feature")
}

fn is_fully_public(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|visibility| visibility.syntax().text().to_string().trim() == "pub")
}

fn check_subtractive_feature_cfg(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let public_items = ctx
        .nodes::<ast::Item>()
        .filter(|item| !matches!(item, ast::Item::Fn(_)))
        .filter(|item| !ctx.is_in_test(item));
    let item_attrs = public_items
        .filter(|item| {
            item.syntax()
                .children()
                .find_map(ast::Visibility::cast)
                .is_some_and(|visibility| is_fully_public(Some(visibility)))
        })
        .flat_map(|item| item.attrs())
        .filter(is_not_feature_cfg);
    let public_functions = ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter(|function| is_fully_public(function.visibility()));
    let function_attrs = public_functions
        .flat_map(|function| function.attrs())
        .filter(is_not_feature_cfg);

    item_attrs
        .chain(function_attrs)
        .map(|attr| {
            ctx.violation(
                &attr,
                "#[cfg(not(feature = ...))] on a pub item — enabling the feature removes public surface; features must be additive",
            )
        })
        .collect()
}

crate::rulewright_ast_test!(check_subtractive_feature_cfg, {
    crate::example_tests!(EXAMPLES, check_subtractive_feature_cfg);
});
