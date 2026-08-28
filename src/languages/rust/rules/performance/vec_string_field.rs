use ra_ap_syntax::{
    AstNode, ast,
    ast::{HasGenericArgs, HasName, HasVisibility},
};

use crate::languages::rust::rules::api::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "private Vec<String> field",
        code: "struct S { names: Vec<String> }",
        pass: false,
    },
    Example {
        label: "private Vec<Vec<T>> field",
        code: "struct S { grid: Vec<Vec<u8>> }",
        pass: false,
    },
    Example {
        label: "nested vectors mutated after construction",
        code: "struct S { grid: Vec<Vec<u8>> } impl S { fn push(&mut self, row: usize, value: u8) { self.grid.get_mut(row).unwrap().push(value); } }",
        pass: true,
    },
    Example {
        label: "pub(crate) field is not fully public",
        code: "pub struct S { pub(crate) names: Vec<String> }",
        pass: false,
    },
    Example {
        label: "private tuple-struct field",
        code: "struct S(Vec<String>);",
        pass: false,
    },
    Example {
        label: "fully public field is user-visible",
        code: "pub struct S { pub names: Vec<String> }",
        pass: true,
    },
    Example {
        label: "already boxed str elements",
        code: "struct S { names: Vec<Box<str>> }",
        pass: true,
    },
    Example {
        label: "primitive element vector",
        code: "struct S { ids: Vec<u32> }",
        pass: true,
    },
    Example {
        label: "Vec<String> field in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct S { names: Vec<String> }\n}",
        pass: true,
    },
    Example {
        label: "record enum variant is outside the struct-only rule",
        code: "enum E { Names { values: Vec<String> } }",
        pass: true,
    },
    Example {
        label: "tuple enum variant is outside the struct-only rule",
        code: "enum E { Names(Vec<String>) }",
        pass: true,
    },
];

crate::ast_rule!(
    vec_string_field,
    "Flag non-pub struct fields typed `Vec<String>` or `Vec<Vec<T>>`.",
    "Immutable-after-construction sequences stored as Vec<Box<str>>/Vec<Box<[T]>> drop the capacity word and imply shrink-to-fit.",
);

fn check_vec_string_field(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let records = ctx
        .nodes::<ast::RecordField>()
        .filter(|field| !ctx.is_in_test(field) && is_struct_field(field))
        .filter_map(|field| {
            let message = field_message(field.visibility(), field.ty())?;
            let name = field.name()?.text().to_string();
            let owner = enclosing_struct_name(&field)?;

            (!field_elements_are_mutated(ctx, &owner, &name)).then_some((field, message))
        });
    let tuples = ctx
        .nodes::<ast::TupleField>()
        .filter(|field| !ctx.is_in_test(field) && is_struct_field(field))
        .filter_map(|field| field_message(field.visibility(), field.ty()).map(|msg| (field, msg)));

    records
        .map(|(field, message)| ctx.violation(&field, message))
        .chain(tuples.map(|(field, message)| ctx.violation(&field, message)))
        .collect()
}

fn enclosing_struct_name(field: &ast::RecordField) -> Option<String> {
    field
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::Struct::cast)?
        .name()
        .map(|name| name.text().to_string())
}

fn field_elements_are_mutated(ctx: &AstCtx<'_>, owner: &str, field: &str) -> bool {
    const MUTABLE_ACCESSORS: &[&str] = &[
        "as_mut_slice",
        "first_mut",
        "get_mut",
        "iter_mut",
        "last_mut",
    ];

    ctx.nodes::<ast::Impl>()
        .filter(|item| item.trait_().is_none())
        .filter(|item| item.self_ty().and_then(|ty| type_name(&ty)).as_deref() == Some(owner))
        .flat_map(|item| {
            item.syntax()
                .descendants()
                .filter_map(ast::MethodCallExpr::cast)
        })
        .any(|call| {
            let accesses_mutably = call
                .name_ref()
                .is_some_and(|method| MUTABLE_ACCESSORS.contains(&method.text().as_str()));
            let receiver = call
                .receiver()
                .map(|receiver| receiver.syntax().text().to_string());

            accesses_mutably
                && receiver.is_some_and(|receiver| {
                    receiver == format!("self.{field}")
                        || receiver.starts_with(&format!("self.{field}."))
                })
        })
}

fn is_struct_field<N>(field: &N) -> bool
where
    N: AstNode,
{
    field
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(|ancestor| {
            if ast::Struct::can_cast(ancestor.kind()) {
                Some(true)
            } else if ast::Variant::can_cast(ancestor.kind()) {
                Some(false)
            } else {
                None
            }
        })
        .unwrap_or(false)
}

const MSG_STRING: &str = "non-pub `Vec<String>` field — if immutable after construction, prefer `Vec<Box<str>>` (drops the capacity word, implies shrink-to-fit)";
const MSG_VEC: &str = "non-pub `Vec<Vec<T>>` field — if immutable after construction, prefer `Vec<Box<[T]>>` (drops the capacity word, implies shrink-to-fit)";

fn vec_inner_ident(ty: &ast::Type) -> Option<String> {
    let ast::Type::PathType(path_type) = ty else {
        return None;
    };
    let segment = path_type.path()?.segment()?;

    if segment.name_ref()?.text() != "Vec" {
        return None;
    }

    segment.generic_arg_list()?.generic_args().find_map(|arg| {
        let ast::GenericArg::TypeArg(arg) = arg else {
            return None;
        };
        let ast::Type::PathType(inner) = arg.ty()? else {
            return None;
        };

        inner
            .path()?
            .segment()?
            .name_ref()
            .map(|name| name.text().to_string())
    })
}

fn field_message(
    visibility: Option<ast::Visibility>,
    ty: Option<ast::Type>,
) -> Option<&'static str> {
    if visibility.is_some_and(|visibility| visibility.syntax().text() == "pub") {
        return None;
    }

    match vec_inner_ident(&ty?).as_deref() {
        Some("String") => Some(MSG_STRING),
        Some("Vec") => Some(MSG_VEC),
        _ => None,
    }
}

crate::rulewright_ast_test!(check_vec_string_field, {
    crate::example_tests!(EXAMPLES, check_vec_string_field);
});
