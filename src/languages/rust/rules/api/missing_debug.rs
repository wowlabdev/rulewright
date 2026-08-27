// #rw(file: rust_default_hasher) small per-file scan set on a cold path; capacity and hasher tuning buy nothing

#[cfg(test)]
use googletest::prelude::*;

use std::collections::HashSet;

use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName, HasVisibility, VisibilityKind},
};

use super::support::{has_derive, type_name};
use crate::{AstCtx, Example, Fix, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub struct without Debug",
        code: "pub struct Foo {}",
        pass: false,
    },
    Example {
        label: "pub struct with Debug",
        code: "#[derive(Debug)]\npub struct Foo {}",
        pass: true,
    },
    Example {
        label: "private struct",
        code: "struct Foo {}",
        pass: true,
    },
    Example {
        label: "pub(crate) struct",
        code: "pub(crate) struct Foo {}",
        pass: true,
    },
    Example {
        label: "pub enum without Debug",
        code: "pub enum E { A, B }",
        pass: false,
    },
    Example {
        label: "pub enum with Debug",
        code: "#[derive(Debug)]\npub enum E { A, B }",
        pass: true,
    },
    Example {
        label: "pub struct in test module",
        code: "#[cfg(test)]\nmod tests {\n  pub struct Foo {}\n}",
        pass: true,
    },
    Example {
        label: "derive with other traits",
        code: "#[derive(Clone, Debug, PartialEq)]\npub struct Foo {}",
        pass: true,
    },
    Example {
        label: "manual Debug impl",
        code: "pub struct Bar {}\nimpl std::fmt::Debug for Bar {\n  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n    f.debug_struct(\"Bar\").finish()\n  }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    missing_debug,
    "Require `#[derive(Debug)]` on public structs and enums.",
    "Public types without Debug are hard to inspect during development and cannot be used in assert messages.",
    Low,
    fix_missing_debug,
);

fn check_missing_debug(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let manual_impls = collect_manual_debug_impls(ctx);
    let structs = ctx
        .nodes::<ast::Struct>()
        .filter_map(|item| missing_debug(ctx, &item, "struct", &manual_impls));
    let enums = ctx
        .nodes::<ast::Enum>()
        .filter_map(|item| missing_debug(ctx, &item, "enum", &manual_impls));

    structs.chain(enums).collect()
}

fn collect_manual_debug_impls(ctx: &AstCtx<'_>) -> HashSet<String> {
    ctx.nodes::<ast::Impl>()
        .filter(|item| {
            item.trait_()
                .and_then(|ty| type_name(&ty))
                .is_some_and(|name| name == "Debug")
        })
        .filter_map(|item| item.self_ty().and_then(|ty| type_name(&ty)))
        .collect()
}

fn missing_debug<T>(
    ctx: &AstCtx<'_>,
    item: &T,
    kind: &str,
    manual_impls: &HashSet<String>,
) -> Option<Violation>
where
    T: AstNode + HasAttrs + HasName + HasVisibility,
{
    let name = item.name()?;

    if ctx.is_in_test(item)
        || !item
            .visibility()
            .is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
        || has_derive(item, "Debug")
        || manual_impls.contains(name.text().as_str())
    {
        return None;
    }

    Some(ctx.violation(
        &name,
        format!("public {kind} `{name}` is missing #[derive(Debug)]"),
    ))
}

fn fix_missing_debug(ctx: &AstCtx<'_>, v: &Violation) -> Option<Fix> {
    if ctx.nodes::<ast::Struct>().any(|item| {
        item.name().is_some_and(|name| ctx.line_of(&name) == v.line)
            && super::super::safety::has_sensitive_fields(ctx, &item)
    }) {
        return None;
    }

    let line = ctx.file.line(v.line)?;
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let derive = format!("{indent}#[derive(Debug)]");

    Some(Fix::replace_line(v.line, format!("{derive}\n{line}")))
}

crate::rulewright_ast_test!(check_missing_debug, {
    crate::example_tests!(EXAMPLES, check_missing_debug);
    crate::fix_tests!(ast, check_missing_debug, fix_missing_debug);

    #[gtest]
    fn sensitive_public_struct_requires_a_manual_debug_impl() -> Result<()> {
        let source = "pub struct Credentials { password: String }";
        let fixed = crate::apply_ast_fixes(source, check_missing_debug, fix_missing_debug);

        verify_eq!(fixed, source)
    }
});
