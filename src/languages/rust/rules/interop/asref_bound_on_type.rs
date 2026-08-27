use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams, HasName, HasTypeBounds, TypeBoundKind},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "inline AsRef bound stored in field",
        code: "struct User<T: AsRef<str>> { name: T }",
        pass: false,
    },
    Example {
        label: "where-clause AsRef bound stored in field",
        code: "struct User<T> where T: AsRef<str> { name: T }",
        pass: false,
    },
    Example {
        label: "bounded parameter nested in field type",
        code: "struct User<T: AsRef<str>> { names: Vec<T> }",
        pass: false,
    },
    Example {
        label: "enum variant stores AsRef-bounded parameter",
        code: "enum E<T: AsRef<str>> { A(T) }",
        pass: false,
    },
    Example {
        label: "qualified AsRef bound",
        code: "struct User<T: std::convert::AsRef<str>> { name: T }",
        pass: false,
    },
    Example {
        label: "concrete owned field",
        code: "struct User { name: String }",
        pass: true,
    },
    Example {
        label: "non-AsRef bound is fine",
        code: "struct User<T: Clone> { name: T }",
        pass: true,
    },
    Example {
        label: "AsRef bound on fn generic is fine",
        code: "fn print<T: AsRef<str>>(x: T) {}",
        pass: true,
    },
    Example {
        label: "bounded parameter not stored in fields",
        code: "struct S<T: AsRef<str>> { count: usize }",
        pass: true,
    },
    Example {
        label: "AsRef-bounded struct in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct User<T: AsRef<str>> { name: T }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    asref_bound_on_type,
    "Flag struct/enum generic parameters bounded by `AsRef<…>` and stored in fields.",
    "AsRef bounds on data-carrying type parameters infect every use of the type; own concrete data like `String` instead.",
    Low,
);

fn check_asref_bound_on_type(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for item in ctx
        .nodes::<ast::Struct>()
        .filter(|item| !ctx.is_in_test(item))
    {
        if let (Some(name), Some(fields)) = (item.name(), item.field_list()) {
            check_type_item(
                ctx,
                "struct",
                &name,
                asref_bounded_params(&item),
                &super::support::field_types(fields),
                &mut violations,
            );
        }
    }

    for item in ctx
        .nodes::<ast::Enum>()
        .filter(|item| !ctx.is_in_test(item))
    {
        if let (Some(name), Some(variants)) = (item.name(), item.variant_list()) {
            let field_types: Vec<ast::Type> = variants
                .variants()
                .filter_map(|variant| variant.field_list())
                .flat_map(super::support::field_types)
                .collect();

            check_type_item(
                ctx,
                "enum",
                &name,
                asref_bounded_params(&item),
                &field_types,
                &mut violations,
            );
        }
    }

    violations
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "ra_ap_syntax bound iterators yield owned facade nodes and this predicate is used directly by Iterator::any"
)]
fn is_asref_bound(bound: ast::TypeBound) -> bool {
    matches!(
        bound.kind(),
        Some(TypeBoundKind::PathType(_, path_type))
            if path_type.path().is_some_and(|path| super::support::path_last_is(path, "AsRef"))
    )
}

fn asref_bounded_params(item: &impl HasGenericParams) -> Vec<String> {
    let mut params = Vec::new();

    if let Some(generics) = item.generic_param_list() {
        params.extend(generics.generic_params().filter_map(|param| {
            let ast::GenericParam::TypeParam(type_param) = param else {
                return None;
            };

            type_param
                .type_bound_list()?
                .bounds()
                .any(is_asref_bound)
                .then(|| type_param.name().map(|name| name.text().to_string()))?
        }));
    }

    let Some(where_clause) = item.where_clause() else {
        return params;
    };

    for predicate in where_clause.predicates() {
        let Some(bounds) = predicate.type_bound_list() else {
            continue;
        };

        if !bounds.bounds().any(is_asref_bound) {
            continue;
        }

        if let Some(name) = predicate.ty().and_then(super::support::type_name) {
            params.push(name);
        }
    }

    params
}

fn type_mentions_param(ty: &ast::Type, param: &str) -> bool {
    ty.syntax()
        .descendants()
        .filter_map(ast::Path::cast)
        .filter(|path| path.syntax().parent().and_then(ast::Path::cast).is_none())
        .any(|path| {
            super::support::path_names(path)
                .first()
                .is_some_and(|name| name == param)
        })
}

fn check_type_item(
    ctx: &AstCtx<'_>,
    kind: &str,
    name: &ast::Name,
    params: Vec<String>,
    field_types: &[ast::Type],
    violations: &mut Vec<Violation>,
) {
    for param in params {
        if field_types.iter().any(|ty| type_mentions_param(ty, &param)) {
            violations.push(ctx.violation(
                name,
                format!(
                    "{kind} `{name}` stores `AsRef`-bounded parameter `{param}` in a field — own concrete data (e.g. `String`) instead"
                ),
            ));
        }
    }
}

crate::rulewright_ast_test!(check_asref_bound_on_type, {
    crate::example_tests!(EXAMPLES, check_asref_bound_on_type);
});
