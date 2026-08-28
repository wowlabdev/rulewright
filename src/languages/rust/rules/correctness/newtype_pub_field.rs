use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs, HasName, HasVisibility, VisibilityKind},
};

use super::support::weak_type_name;
use crate::languages::rust::rules::api::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "public field bypasses fallible constructor",
        code: "pub struct UserId(pub u64); impl UserId { pub fn try_new(raw: u64) -> Result<Self, Error> { Ok(Self(raw)) } }",
        pass: false,
    },
    Example {
        label: "plain transparent integer carrier",
        code: "pub struct UserId(pub u64);",
        pass: true,
    },
    Example {
        label: "plain transparent string carrier",
        code: "pub struct Name(pub String);",
        pass: true,
    },
    Example {
        label: "unrelated fallible associated function",
        code: "pub struct Name(pub String); impl Name { fn lookup() -> Option<String> { None } }",
        pass: true,
    },
    Example {
        label: "public fallible associated function returning another type",
        code: "pub struct Name(pub String); impl Name { pub fn lookup() -> Result<String, Error> { todo!() } }",
        pass: true,
    },
    Example {
        label: "same-named type in another module owns the constructor",
        code: "pub mod one { pub struct Id(pub u64); } pub mod two { pub struct Id; impl Id { pub fn parse() -> Result<Self, ()> { Ok(Self) } } }",
        pass: true,
    },
    Example {
        label: "pub tuple newtype with pub str ref",
        code: "pub struct Label<'a>(pub &'a str); impl Label<'_> { pub fn parse(raw: &str) -> Option<Label<'_>> { Some(Label(raw)) } }",
        pass: false,
    },
    Example {
        label: "pub named newtype with pub field",
        code: "pub struct Port { pub value: u16 } impl Port { pub fn new(value: u16) -> Result<Self, Error> { Ok(Self { value }) } }",
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
    "Flag public weak-typed newtype fields only when the type also exposes fallible construction.",
    "A public field bypasses validation when the type's own constructor returns Result or Option. Make that field private; plain transparent carriers without a validation contract are left alone.",
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

            if !has_fallible_constructor(ctx, &item, ident.text().as_ref()) {
                return None;
            }

            Some(ctx.violation(
                &ident,
                format!(
                    "pub newtype `{ident}` exposes a `pub {weak}` field that bypasses its fallible constructor — make the field private"
                ),
            ))
        })
        .collect()
}

fn has_fallible_constructor(ctx: &AstCtx<'_>, owner_item: &ast::Struct, owner: &str) -> bool {
    let owner_module = containing_module(owner_item);
    let owner_impls = ctx.nodes::<ast::Impl>().filter(|item| {
        item.trait_().is_none()
            && item.self_ty().and_then(|ty| type_name(&ty)).as_deref() == Some(owner)
            && containing_module(item) == owner_module
    });

    for item_impl in owner_impls {
        let Some(items) = item_impl.assoc_item_list() else {
            continue;
        };

        for item in items.assoc_items() {
            let ast::AssocItem::Fn(function) = item else {
                continue;
            };

            let is_public_associated = is_public(function.visibility())
                && function
                    .param_list()
                    .is_some_and(|params| params.self_param().is_none());
            let returns_owner = function
                .ret_type()
                .and_then(|ret| ret.ty())
                .is_some_and(|ty| is_fallible_owner_output(&ty, owner));

            if is_public_associated && returns_owner {
                return true;
            }
        }
    }

    false
}

fn containing_module(item: &impl AstNode) -> Option<ast::Module> {
    item.syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::Module::cast)
}

fn is_fallible_owner_output(ty: &ast::Type, owner: &str) -> bool {
    let ast::Type::PathType(path_type) = ty else {
        return false;
    };
    let Some(segment) = path_type.path().and_then(|path| path.segment()) else {
        return false;
    };
    let fallible = segment
        .name_ref()
        .is_some_and(|name| matches!(name.text().as_str(), "Result" | "Option"));

    fallible
        && segment
            .generic_arg_list()
            .and_then(|args| {
                args.generic_args().find_map(|argument| {
                    let ast::GenericArg::TypeArg(argument) = argument else {
                        return None;
                    };

                    argument.ty()
                })
            })
            .and_then(|output| type_name(&output))
            .is_some_and(|name| name == "Self" || name == owner)
}

fn is_public(visibility: Option<ast::Visibility>) -> bool {
    visibility.is_some_and(|visibility| matches!(visibility.kind(), VisibilityKind::Pub))
}

crate::rulewright_ast_test!(check_newtype_pub_field, {
    crate::example_tests!(EXAMPLES, check_newtype_pub_field);
});
