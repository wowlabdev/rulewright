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
    "Flag `pub` enums without `#[non_exhaustive]` when downstream variants should remain extensible.",
    "Adding a variant can break downstream exhaustive matches. Use `#[non_exhaustive]` for externally reachable enums in publishable packages whose variant set may grow; packages that explicitly set `publish = false` are skipped. Keep the rule disabled or scoped away for private-module enums and deliberately closed protocol, state-machine, or schema vocabularies. Rulewright recognizes syntax-level `pub` but does not resolve re-export visibility.",
    Medium,
);

fn check_non_exhaustive_on_public(ctx: &AstCtx<'_>) -> Vec<Violation> {
    if ctx.file.is_explicitly_non_publishable() {
        return Vec::new();
    }

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
                    "`pub` enum `{name}` is exhaustively matchable downstream — add `#[non_exhaustive]` only when this externally reachable vocabulary should grow without a breaking release"
                ),
            ))
        })
        .collect()
}

crate::rulewright_ast_test!(check_non_exhaustive_on_public, {
    crate::example_tests!(EXAMPLES, check_non_exhaustive_on_public);

    #[test]
    fn package_publishability_controls_downstream_enum_stability() {
        let source = "pub enum Status { Ready, Waiting }";

        assert_eq!(
            crate::test_support::check_source_ast_at(
                "fixture.rs",
                source,
                check_non_exhaustive_on_public,
            )
            .len(),
            1
        );
        assert!(
            crate::test_support::check_source_ast_publishability(
                source,
                false,
                check_non_exhaustive_on_public,
            )
            .is_empty()
        );
        assert_eq!(
            crate::test_support::check_source_ast_publishability(
                source,
                true,
                check_non_exhaustive_on_public,
            )
            .len(),
            1
        );
    }
});
