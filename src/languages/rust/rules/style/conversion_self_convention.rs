use std::collections::BTreeSet;

use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "as_ takes self by value",
        code: "struct S;\nimpl S {\n    fn as_str(self) {}\n}",
        pass: false,
    },
    Example {
        label: "to_ takes self by value",
        code: "struct S;\nimpl S {\n    fn to_vec(self) {}\n}",
        pass: false,
    },
    Example {
        label: "into_ borrows self",
        code: "struct S;\nimpl S {\n    fn into_parts(&self) {}\n}",
        pass: false,
    },
    Example {
        label: "into_ borrows self mutably",
        code: "struct S;\nimpl S {\n    fn into_inner(&mut self) {}\n}",
        pass: false,
    },
    Example {
        label: "wrong convention on trait method",
        code: "trait T {\n    fn as_bytes(self);\n}",
        pass: false,
    },
    Example {
        label: "to_ consumes a same-file Copy type",
        code: "#[derive(Clone, Copy)]\nstruct Id(u32);\nimpl Id {\n    fn to_raw(self) -> u32 { self.0 }\n}",
        pass: true,
    },
    Example {
        label: "to_ consumes a type with a manual Copy impl",
        code: "struct Id(u32);\nimpl Clone for Id { fn clone(&self) -> Self { *self } }\nimpl Copy for Id {}\nimpl Id {\n    fn to_raw(self) -> u32 { self.0 }\n}",
        pass: true,
    },
    Example {
        label: "as_ consumes a Copy type",
        code: "#[derive(Clone, Copy)]\nstruct Id(u32);\nimpl Id {\n    fn as_raw(self) -> u32 { self.0 }\n}",
        pass: false,
    },
    Example {
        label: "as_ borrows",
        code: "struct S;\nimpl S {\n    fn as_str(&self) {}\n}",
        pass: true,
    },
    Example {
        label: "as_ borrows mutably",
        code: "struct S;\nimpl S {\n    fn as_mut_slice(&mut self) {}\n}",
        pass: true,
    },
    Example {
        label: "to_ borrows",
        code: "struct S;\nimpl S {\n    fn to_vec(&self) {}\n}",
        pass: true,
    },
    Example {
        label: "into_ consumes",
        code: "struct S;\nimpl S {\n    fn into_parts(self) {}\n}",
        pass: true,
    },
    Example {
        label: "no receiver is exempt",
        code: "struct S;\nimpl S {\n    fn into_config() -> S { S }\n}",
        pass: true,
    },
    Example {
        label: "non-conversion method",
        code: "struct S;\nimpl S {\n    fn assemble(self) {}\n}",
        pass: true,
    },
    Example {
        label: "wrong convention in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct S;\n    impl S {\n        fn as_str(self) {}\n    }\n}",
        pass: false,
    },
];

crate::ast_rule!(
    conversion_self_convention,
    "Enforce C-CONV receivers: `as_`/`to_` methods borrow (`&self`), `into_` methods consume (`self`); `to_` may consume a `Copy` type declared in the same file.",
    "The `as_`/`to_`/`into_` prefixes promise a cost and ownership contract; a consuming `to_` or borrowing `into_` misleads every caller.",
    Medium,
);

fn check_conversion_self_convention(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let copy_types = copy_types(ctx);

    ctx.nodes::<ast::Fn>()
        .filter_map(|function| {
            let receiver = function.param_list()?.self_param()?;

            if receiver.colon_token().is_some() {
                return None;
            }

            let name = function.name()?;
            let name_text = name.text();
            let borrows = receiver.amp_token().is_some();
            let copy_receiver = || {
                function
                    .syntax()
                    .ancestors()
                    .find_map(ast::Impl::cast)
                    .and_then(|item| item.self_ty())
                    .and_then(|ty| super::super::support::type_name(&ty))
                    .is_some_and(|name| copy_types.contains(&name))
            };
            let message = if (name_text.starts_with("as_")
                || name_text.starts_with("to_") && !copy_receiver())
                && !borrows
            {
                format!(
                    "conversion `{name_text}` consumes `self` — `as_`/`to_` methods take `&self`; use `into_` for consuming conversions (C-CONV)"
                )
            } else if name_text.starts_with("into_") && borrows {
                format!(
                    "conversion `{name_text}` borrows `self` — `into_` methods consume `self` by value (C-CONV)"
                )
            } else {
                return None;
            };

            Some(ctx.violation(&name, message))
        })
        .collect()
}

/// Names of types in this file that derive or implement `Copy`.
fn copy_types(ctx: &AstCtx<'_>) -> BTreeSet<String> {
    let derived = ctx
        .nodes::<ast::Adt>()
        .filter(|item| {
            item.attrs().any(|attr| {
                attr.simple_name().is_some_and(|name| name == "derive")
                    && attr.syntax().text().to_string().contains("Copy")
            })
        })
        .filter_map(|item| item.name().map(|name| name.text().to_string()));
    let implemented = ctx
        .nodes::<ast::Impl>()
        .filter(|item| {
            item.trait_()
                .and_then(|ty| super::super::support::type_name(&ty))
                .is_some_and(|name| name == "Copy")
        })
        .filter_map(|item| super::super::support::type_name(&item.self_ty()?));

    derived.chain(implemented).collect()
}

crate::rulewright_ast_test!(check_conversion_self_convention, {
    crate::example_tests!(EXAMPLES, check_conversion_self_convention);
});
