use ra_ap_syntax::ast::{self, HasName, HasVisibility, VisibilityKind};

use super::support::weak_type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pub tuple newtype with pub integer",
        code: "pub struct UserId(pub u64);",
        pass: false,
    },
    Example {
        label: "pub tuple newtype with pub String",
        code: "pub struct Name(pub String);",
        pass: false,
    },
    Example {
        label: "pub tuple newtype with pub str ref",
        code: "pub struct Label<'a>(pub &'a str);",
        pass: false,
    },
    Example {
        label: "pub named newtype with pub field",
        code: "pub struct Port { pub value: u16 }",
        pass: false,
    },
    Example {
        label: "private field",
        code: "pub struct UserId(u64);",
        pass: true,
    },
    Example {
        label: "two fields",
        code: "pub struct Point(pub f32, pub f32);",
        pass: true,
    },
    Example {
        label: "non-pub struct",
        code: "struct Id(pub u32);",
        pass: true,
    },
    Example {
        label: "non-primitive field",
        code: "pub struct Wrapper(pub Vec<u8>);",
        pass: true,
    },
    Example {
        label: "newtype in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub struct UserId(pub u64);\n}",
        pass: true,
    },
];

crate::ast_rule!(
    newtype_pub_field,
    "Flag pub single-field structs exposing a pub primitive/`&str`/`String` field.",
    "A public weak-typed field bypasses any constructor-enforced invariant; make the field private and construct fallibly.",
    Medium,
);

fn check_newtype_pub_field(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item) && is_public(item.visibility()))
        .filter_map(|item| {
            let fields: Vec<(Option<ast::Visibility>, ast::Type)> = match item.field_list()? {
                ast::FieldList::RecordFieldList(list) => list
                    .fields()
                    .filter_map(|field| Some((field.visibility(), field.ty()?)))
                    .collect(),

                ast::FieldList::TupleFieldList(list) => list
                    .fields()
                    .filter_map(|field| Some((field.visibility(), field.ty()?)))
                    .collect(),
            };
            let [(visibility, ty)] = fields.as_slice() else {
                return None;
            };

            if !is_public(visibility.clone()) {
                return None;
            }

            let weak = weak_type_name(ty)?;
            let ident = item.name()?;

            Some(ctx.violation(
                &ident,
                format!(
                    "pub newtype `{ident}` exposes a `pub {weak}` field — make it private so the constructor guards the invariant"
                ),
            ))
        })
        .collect()
}

fn is_public(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
}

crate::rulewright_ast_test!(check_newtype_pub_field, {
    crate::example_tests!(EXAMPLES, check_newtype_pub_field);
});
