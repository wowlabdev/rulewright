use ra_ap_syntax::ast::{self, HasGenericParams, HasName, HasTypeBounds, TypeBoundKind};
use std::collections::HashSet;

use super::super::support::{is_inside_trait, type_name};
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "closure before value param",
        code: "fn f(cb: impl Fn(), x: u32) {}",
        pass: false,
    },
    Example {
        label: "generic closure before value param",
        code: "fn f<F: FnMut()>(cb: F, x: u32) {}",
        pass: false,
    },
    Example {
        label: "where-bounded closure before value param",
        code: "fn f<F>(cb: F, x: u32) where F: Fn() -> u32 {}",
        pass: false,
    },
    Example {
        label: "two closure params",
        code: "fn f(a: impl Fn(), b: impl FnOnce()) {}",
        pass: false,
    },
    Example {
        label: "closure last",
        code: "fn f(x: u32, cb: impl Fn()) {}",
        pass: true,
    },
    Example {
        label: "where-bounded closure last",
        code: "fn f<F>(x: u32, cb: F) where F: FnOnce() {}",
        pass: true,
    },
    Example {
        label: "fn pointer is not a closure",
        code: "fn f(cb: fn(), x: u32) {}",
        pass: true,
    },
    Example {
        label: "no closure params",
        code: "fn f(x: u32, y: u32) {}",
        pass: true,
    },
    Example {
        label: "closure first in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(cb: impl Fn(), x: u32) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    closure_param_position,
    "Flag closure parameters that are not last, and fns taking more than one closure.",
    "Closures go last so multi-line closure arguments read naturally at call sites; more than one closure parameter makes calls unreadable and argument order ambiguous.",
    Low,
);

#[expect(
    clippy::needless_pass_by_value,
    reason = "ra_ap_syntax bound iterators yield owned facade nodes and this predicate is used directly by Iterator::any"
)]
fn is_fn_bound(bound: ast::TypeBound) -> bool {
    matches!(
        bound.kind(),
        Some(TypeBoundKind::PathType(_, path_type))
            if path_type.path().and_then(|path| path.segment()).and_then(|segment| segment.name_ref()).is_some_and(|name| matches!(name.text().as_str(), "Fn" | "FnMut" | "FnOnce"))
    )
}

fn closure_generics(function: &ast::Fn) -> HashSet<String> {
    let mut names = HashSet::default();

    if let Some(generics) = function.generic_param_list() {
        names.extend(generics.generic_params().filter_map(|parameter| {
            let ast::GenericParam::TypeParam(parameter) = parameter else {
                return None;
            };

            parameter
                .type_bound_list()?
                .bounds()
                .any(is_fn_bound)
                .then(|| parameter.name().map(|name| name.text().to_string()))?
        }));
    }

    if let Some(where_clause) = function.where_clause() {
        names.extend(where_clause.predicates().filter_map(|predicate| {
            predicate
                .type_bound_list()?
                .bounds()
                .any(is_fn_bound)
                .then(|| type_name(&predicate.ty()?))?
        }));
    }

    names
}

fn is_closure_type(mut ty: ast::Type, generics: &HashSet<String>) -> bool {
    while let ast::Type::RefType(reference) = ty {
        let Some(inner) = reference.ty() else {
            return false;
        };

        ty = inner;
    }

    match ty {
        ast::Type::ImplTraitType(impl_trait) => impl_trait
            .type_bound_list()
            .is_some_and(|bounds| bounds.bounds().any(is_fn_bound)),

        ast::Type::PathType(_) => type_name(&ty).is_some_and(|name| generics.contains(&name)),

        _ => false,
    }
}

fn check_closure_param_position(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for function in ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function) && !is_inside_trait(function))
    {
        let generics = closure_generics(&function);
        let parameters = function
            .param_list()
            .into_iter()
            .flat_map(|parameters| parameters.params());
        let flags: Vec<bool> = parameters
            .filter_map(|parameter| parameter.ty())
            .map(|ty| is_closure_type(ty, &generics))
            .collect();
        let Some(name) = function.name() else {
            continue;
        };

        if flags.iter().filter(|&&flag| flag).count() > 1 {
            violations.push(ctx.violation(
                &name,
                format!("fn `{name}` takes more than one closure parameter — accept at most one"),
            ));
        }

        if flags
            .iter()
            .zip(flags.iter().skip(1))
            .any(|(current, next)| *current && !*next)
        {
            violations.push(ctx.violation(
                &name,
                format!(
                    "fn `{name}` has a closure parameter before a non-closure parameter — closures go last"
                ),
            ));
        }
    }

    violations
}

crate::rulewright_ast_test!(check_closure_param_position, {
    crate::example_tests!(EXAMPLES, check_closure_param_position);
});
