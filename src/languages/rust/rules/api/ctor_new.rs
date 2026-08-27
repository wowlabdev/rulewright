use ra_ap_syntax::ast::{self, HasName, HasVisibility, VisibilityKind};
use std::collections::HashSet;

use super::support::{has_derive, type_name};
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "derived Default without new",
        code: "#[derive(Default)]\npub struct Pool { size: u32 }",
        pass: false,
    },
    Example {
        label: "manual Default without new",
        code: "pub struct Pool { size: u32 }\nimpl Default for Pool {\n    fn default() -> Self {\n        Pool { size: 0 }\n    }\n}",
        pass: false,
    },
    Example {
        label: "private new only",
        code: "#[derive(Default)]\npub struct Pool { size: u32 }\nimpl Pool {\n    fn new() -> Self {\n        Pool { size: 0 }\n    }\n}",
        pass: false,
    },
    Example {
        label: "Default with pub new",
        code: "#[derive(Default)]\npub struct Pool { size: u32 }\nimpl Pool {\n    pub fn new() -> Self {\n        Pool { size: 0 }\n    }\n}",
        pass: true,
    },
    Example {
        label: "no Default",
        code: "pub struct Pool { size: u32 }",
        pass: true,
    },
    Example {
        label: "struct-literal constructible",
        code: "#[derive(Default)]\npub struct Point { pub x: f32, pub y: f32 }",
        pass: true,
    },
    Example {
        label: "private struct",
        code: "#[derive(Default)]\nstruct Pool { size: u32 }",
        pass: true,
    },
    Example {
        label: "Default without new in test module",
        code: "#[cfg(test)]\nmod tests {\n    #[derive(Default)]\n    pub struct Pool { size: u32 }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    ctor_new,
    "Flag public structs with `Default` but no `pub fn new` — constructors are static inherent methods (C-CTOR).",
    "Users reach for X::new() first; a type offering only Default surprises them and breaks the upstream C-CTOR convention.",
    Low,
);

fn check_ctor_new(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let (default_impls, pub_new) = collect_ctor_info(ctx);

    let public_structs = ctx
        .nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item))
        .filter(|item| is_pub(item.visibility()))
        .filter(has_private_field);

    public_structs
        .filter(|item| {
            item.name().is_some_and(|name| {
                has_derive(item, "Default") || default_impls.contains(name.text().as_str())
            })
        })
        .filter_map(|item| {
            let name = item.name()?;

            (!pub_new.contains(name.text().as_str())).then(|| {
                ctx.violation(
                    &name,
                    format!(
                        "public struct `{name}` implements Default but has no `pub fn new` (C-CTOR)"
                    ),
                )
            })
        })
        .collect()
}

fn collect_ctor_info(ctx: &AstCtx<'_>) -> (HashSet<String>, HashSet<String>) {
    let mut default_impls = HashSet::default();
    let mut pub_new = HashSet::default();

    for item in ctx.nodes::<ast::Impl>() {
        let Some(ty) = item.self_ty().and_then(|ty| type_name(&ty)) else {
            continue;
        };

        match item.trait_().and_then(|ty| type_name(&ty)).as_deref() {
            Some("Default") => {
                default_impls.insert(ty);
            }
            None if has_pub_new(&item) => {
                pub_new.insert(ty);
            }
            _ => {}
        }
    }

    (default_impls, pub_new)
}

fn has_pub_new(item: &ast::Impl) -> bool {
    item.assoc_item_list().is_some_and(|list| {
        list.assoc_items().any(|assoc| match assoc {
            ast::AssocItem::Fn(function) => {
                function.name().is_some_and(|name| name.text() == "new")
                    && is_pub(function.visibility())
            }
            _ => false,
        })
    })
}

fn has_private_field(item: &ast::Struct) -> bool {
    item.field_list().is_some_and(|list| match list {
        ast::FieldList::RecordFieldList(list) => {
            list.fields().any(|field| !is_pub(field.visibility()))
        }
        ast::FieldList::TupleFieldList(list) => {
            list.fields().any(|field| !is_pub(field.visibility()))
        }
    })
}

fn is_pub(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|vis| matches!(vis.kind(), VisibilityKind::Pub))
}

crate::rulewright_ast_test!(check_ctor_new, {
    crate::example_tests!(EXAMPLES, check_ctor_new);
});
