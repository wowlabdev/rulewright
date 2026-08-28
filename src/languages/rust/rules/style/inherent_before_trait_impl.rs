use ra_ap_syntax::{AstNode, ast};
use std::collections::HashMap;

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "inherent impl first",
        code: "struct Item;\nimpl Item { fn new() -> Self { Self } }\nimpl Default for Item { fn default() -> Self { Self } }",
        pass: true,
    },
    Example {
        label: "trait impl first",
        code: "struct Item;\nimpl Default for Item { fn default() -> Self { Self } }\nimpl Item { fn new() -> Self { Self } }",
        pass: false,
    },
    Example {
        label: "different types",
        code: "struct One; struct Two;\nimpl Default for One { fn default() -> Self { Self } }\nimpl Two {}",
        pass: true,
    },
    Example {
        label: "trait impl only",
        code: "struct Item;\nimpl Default for Item { fn default() -> Self { Self } }",
        pass: true,
    },
];

crate::ast_rule!(
    inherent_before_trait_impl,
    "Require an inherent impl to precede trait impls for the same local type.",
    "Putting the type's own API first makes its primary behavior easier to discover.",
    Low,
);

fn check_inherent_before_trait_impl(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut first_inherent = HashMap::<String, usize>::default();
    let mut trait_impls = Vec::new();

    for item_impl in ctx.nodes::<ast::Impl>() {
        let Some(self_type) = item_impl.self_ty() else {
            continue;
        };
        let key: String = self_type
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect();
        let offset: usize = item_impl.syntax().text_range().start().into();

        if item_impl.trait_().is_none() {
            first_inherent.entry(key).or_insert(offset);
        } else {
            trait_impls.push((key, offset, item_impl));
        }
    }

    trait_impls
        .into_iter()
        .filter_map(|(key, offset, item_impl)| {
            first_inherent
                .get(&key)
                .is_some_and(|inherent_offset| offset < *inherent_offset)
                .then(|| {
                    ctx.violation(
                        &item_impl,
                        "trait impl appears before the inherent impl for this type",
                    )
                })
        })
        .collect()
}

crate::rulewright_ast_test!(check_inherent_before_trait_impl, {
    crate::example_tests!(EXAMPLES, check_inherent_before_trait_impl);
});
