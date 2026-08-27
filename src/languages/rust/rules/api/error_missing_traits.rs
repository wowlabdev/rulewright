use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName, HasVisibility, VisibilityKind},
};
use std::collections::HashSet;

use super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "error struct without traits",
        code: "pub struct ParseError { line: usize }",
        pass: false,
    },
    Example {
        label: "error struct with Display only",
        code: "pub struct ParseError;\nimpl std::fmt::Display for ParseError {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.write_str(\"parse error\")\n    }\n}",
        pass: false,
    },
    Example {
        label: "error struct with Display and Error",
        code: "pub struct ParseError;\nimpl std::fmt::Display for ParseError {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.write_str(\"parse error\")\n    }\n}\nimpl std::error::Error for ParseError {}",
        pass: true,
    },
    Example {
        label: "thiserror derive on struct",
        code: "#[derive(Debug, thiserror::Error)]\n#[error(\"parse failed\")]\npub struct ParseError;",
        pass: true,
    },
    Example {
        label: "thiserror derive on enum",
        code: "#[derive(Debug, Error)]\npub enum FetchError {\n    #[error(\"io failed\")]\n    Io,\n}",
        pass: true,
    },
    Example {
        label: "private error struct",
        code: "struct ParseError;",
        pass: true,
    },
    Example {
        label: "pub struct without error suffix",
        code: "pub struct Parser { pos: usize }",
        pass: true,
    },
    Example {
        label: "error struct in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub struct ParseError;\n}",
        pass: true,
    },
];

crate::ast_rule!(
    error_missing_traits,
    "Require `Display` and `std::error::Error` on public `*Error` types.",
    "std::error::Error mandates Display, and error types without both cannot participate in ?-chains, error reporting, or dyn Error composition.",
    Medium,
);

fn check_error_missing_traits(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let (display_impls, error_impls) = collect_trait_impls(ctx);
    let structs = ctx
        .nodes::<ast::Struct>()
        .flat_map(|item| check_error_item(ctx, &item, false, &display_impls, &error_impls));
    let enums = ctx.nodes::<ast::Enum>().flat_map(|item| {
        let variant_error_attrs = item
            .variant_list()
            .is_some_and(|variants| variants.variants().any(|variant| has_error_attr(&variant)));

        check_error_item(
            ctx,
            &item,
            variant_error_attrs,
            &display_impls,
            &error_impls,
        )
    });

    structs.chain(enums).collect()
}

fn collect_trait_impls(ctx: &AstCtx<'_>) -> (HashSet<String>, HashSet<String>) {
    let mut display_impls = HashSet::default();
    let mut error_impls = HashSet::default();

    for item in ctx.nodes::<ast::Impl>() {
        let Some(trait_name) = item.trait_().and_then(|ty| type_name(&ty)) else {
            continue;
        };
        let Some(type_name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
            continue;
        };

        if trait_name == "Display" {
            display_impls.insert(type_name);
        } else if trait_name == "Error" {
            error_impls.insert(type_name);
        }
    }

    (display_impls, error_impls)
}

fn has_error_derive(item: &impl HasAttrs) -> bool {
    item.attrs().any(|attr| {
        attr.as_simple_call().is_some_and(|(name, tokens)| {
            name == "derive"
                && tokens.syntax().text().to_string().split(',').any(|entry| {
                    entry
                        .trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ')'))
                        .rsplit("::")
                        .next()
                        == Some("Error")
                })
        })
    })
}

fn has_error_attr(item: &impl HasAttrs) -> bool {
    item.attrs()
        .any(|attr| attr.simple_name().as_deref() == Some("error"))
}

fn check_error_item<T>(
    ctx: &AstCtx<'_>,
    item: &T,
    variant_error_attrs: bool,
    display_impls: &HashSet<String>,
    error_impls: &HashSet<String>,
) -> Vec<Violation>
where
    T: AstNode + HasAttrs + HasName + HasVisibility,
{
    let Some(name_node) = item.name() else {
        return Vec::new();
    };
    let name = name_node.text().to_string();

    if ctx.is_in_test(item)
        || !item
            .visibility()
            .is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
        || !name.ends_with("Error")
    {
        return Vec::new();
    }

    let derive_error = has_error_derive(item);
    let display_ok = derive_error
        || variant_error_attrs
        || has_error_attr(item)
        || display_impls.contains(&name);
    let error_ok = derive_error || error_impls.contains(&name);
    let mut out = Vec::new();

    if !display_ok {
        out.push(ctx.violation(
            &name_node,
            format!("public error type `{name}` must implement `Display`"),
        ));
    }

    if !error_ok {
        out.push(ctx.violation(
            &name_node,
            format!("public error type `{name}` must implement `std::error::Error`"),
        ));
    }

    out
}

crate::rulewright_ast_test!(check_error_missing_traits, {
    crate::example_tests!(EXAMPLES, check_error_missing_traits);
});
