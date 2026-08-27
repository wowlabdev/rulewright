use ra_ap_syntax::ast::{self, HasGenericArgs};
use std::collections::HashSet;

use super::{super::support::type_name, support::weak_type_name};
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "From alongside TryFrom",
        code: "pub struct Month(u8);\nimpl TryFrom<u8> for Month {\n    type Error = String;\n    fn try_from(v: u8) -> Result<Self, String> { Err(String::new()) }\n}\nimpl From<u8> for Month {\n    fn from(v: u8) -> Self { Month(v) }\n}",
        pass: false,
    },
    Example {
        label: "From alongside inherent Result constructor",
        code: "pub struct Port(u16);\nimpl Port {\n    pub fn new(v: u16) -> Result<Self, String> { Ok(Port(v)) }\n}\nimpl From<u16> for Port {\n    fn from(v: u16) -> Self { Port(v) }\n}",
        pass: false,
    },
    Example {
        label: "From from str alongside TryFrom",
        code: "pub struct Tag(String);\nimpl TryFrom<String> for Tag {\n    type Error = String;\n    fn try_from(v: String) -> Result<Self, String> { Err(v) }\n}\nimpl From<&str> for Tag {\n    fn from(v: &str) -> Self { Tag(v.to_string()) }\n}",
        pass: false,
    },
    Example {
        label: "From without fallible construction",
        code: "pub struct Month(u8);\nimpl From<u8> for Month {\n    fn from(v: u8) -> Self { Month(v) }\n}",
        pass: true,
    },
    Example {
        label: "From from strong type",
        code: "pub struct Inner(u8);\npub struct Outer(Inner);\nimpl TryFrom<u8> for Outer {\n    type Error = String;\n    fn try_from(v: u8) -> Result<Self, String> { Err(String::new()) }\n}\nimpl From<Inner> for Outer {\n    fn from(v: Inner) -> Self { Outer(v) }\n}",
        pass: true,
    },
    Example {
        label: "infallible constructor only",
        code: "pub struct Port(u16);\nimpl Port {\n    pub fn new(v: u16) -> Self { Port(v) }\n}\nimpl From<u16> for Port {\n    fn from(v: u16) -> Self { Port(v) }\n}",
        pass: true,
    },
    Example {
        label: "From in test module",
        code: "#[cfg(test)]\nmod tests {\n    pub struct Month(u8);\n    impl TryFrom<u16> for Month {\n        type Error = String;\n        fn try_from(v: u16) -> Result<Self, String> { Err(String::new()) }\n    }\n    impl From<u8> for Month {\n        fn from(v: u8) -> Self { Month(v) }\n    }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    infallible_from_weak,
    "Flag `impl From<weak>` next to fallible construction of the same type.",
    "An infallible conversion from a weak type bypasses the invariant the fallible constructor exists to guard; offer only TryFrom.",
    Medium,
);

fn single_type_arg(segment: &ast::PathSegment) -> Option<ast::Type> {
    let args = segment.generic_arg_list()?;
    let mut iter = args.generic_args();

    match (iter.next(), iter.next()) {
        (Some(ast::GenericArg::TypeArg(arg)), None) => arg.ty(),
        _ => None,
    }
}

fn returns_result_self(function: &ast::Fn, self_name: &str) -> bool {
    let Some(ast::Type::PathType(path_type)) = function.ret_type().and_then(|ret| ret.ty()) else {
        return false;
    };
    let Some(segment) = path_type.path().and_then(|path| path.segment()) else {
        return false;
    };

    if segment
        .name_ref()
        .is_none_or(|name| name.text() != "Result")
    {
        return false;
    }

    let Some(ok) = segment.generic_arg_list().and_then(|args| {
        args.generic_args().find_map(|arg| match arg {
            ast::GenericArg::TypeArg(arg) => arg.ty(),
            _ => None,
        })
    }) else {
        return false;
    };

    match ok {
        ast::Type::PathType(path) => {
            let name = path
                .path()
                .and_then(|path| path.segment())
                .and_then(|segment| segment.name_ref());

            name.is_some_and(|name| {
                matches!(name.text().as_str(), "Self") || name.text() == self_name
            })
        }
        _ => false,
    }
}

fn check_infallible_from_weak(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut from_weak = Vec::new();
    let mut fallible = HashSet::default();

    for item_impl in ctx
        .nodes::<ast::Impl>()
        .filter(|item_impl| !ctx.is_in_test(item_impl))
    {
        collect_impl(&item_impl, &mut from_weak, &mut fallible);
    }

    let mut violations = Vec::new();

    for (self_name, weak, location) in from_weak {
        if fallible.contains(&self_name) {
            violations.push(ctx.violation(
                &location,
                format!(
                    "infallible `From<{weak}> for {self_name}` alongside fallible construction — route weak types through TryFrom"
                ),
            ));
        }
    }

    violations
}

fn collect_impl(
    item: &ast::Impl,
    from_weak: &mut Vec<(String, String, ast::NameRef)>,
    fallible: &mut HashSet<String>,
) {
    let Some(self_name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
        return;
    };
    let Some(trait_type) = item.trait_() else {
        let associated_items = item
            .assoc_item_list()
            .into_iter()
            .flat_map(|list| list.assoc_items());
        let has_fallible_ctor = associated_items
            .filter_map(|assoc| match assoc {
                ast::AssocItem::Fn(function) => Some(function),
                _ => None,
            })
            .any(|function| returns_result_self(&function, &self_name));

        if has_fallible_ctor {
            fallible.insert(self_name);
        }

        return;
    };
    let ast::Type::PathType(path_type) = trait_type else {
        return;
    };
    let Some(segment) = path_type.path().and_then(|path| path.segment()) else {
        return;
    };
    let Some(name) = segment.name_ref() else {
        return;
    };

    if name.text() == "TryFrom" {
        fallible.insert(self_name);
    } else if name.text() == "From"
        && let Some(weak) = single_type_arg(&segment).as_ref().and_then(weak_type_name)
    {
        from_weak.push((self_name, weak, name));
    }
}

crate::rulewright_ast_test!(check_infallible_from_weak, {
    crate::example_tests!(EXAMPLES, check_infallible_from_weak);
});
