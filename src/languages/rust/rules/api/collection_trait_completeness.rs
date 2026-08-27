#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{ast, ast::HasName};
use std::collections::HashSet;

use super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "iter with IntoIterator for ref",
        code: "struct Bag(Vec<u32>);\nimpl Bag {\n    fn iter(&self) -> std::slice::Iter<'_, u32> { self.0.iter() }\n}\nimpl<'a> IntoIterator for &'a Bag {\n    type Item = &'a u32;\n    type IntoIter = std::slice::Iter<'a, u32>;\n    fn into_iter(self) -> Self::IntoIter { self.0.iter() }\n}",
        pass: true,
    },
    Example {
        label: "iter without IntoIterator for ref",
        code: "struct Bag(Vec<u32>);\nimpl Bag {\n    fn iter(&self) -> std::slice::Iter<'_, u32> { self.0.iter() }\n}",
        pass: false,
    },
    Example {
        label: "iter_mut without IntoIterator for mut ref",
        code: "struct Bag(Vec<u32>);\nimpl Bag {\n    fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> { self.0.iter_mut() }\n}",
        pass: false,
    },
    Example {
        label: "iter_mut with IntoIterator for mut ref",
        code: "struct Bag(Vec<u32>);\nimpl Bag {\n    fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> { self.0.iter_mut() }\n}\nimpl<'a> IntoIterator for &'a mut Bag {\n    type Item = &'a mut u32;\n    type IntoIter = std::slice::IterMut<'a, u32>;\n    fn into_iter(self) -> Self::IntoIter { self.0.iter_mut() }\n}",
        pass: true,
    },
    Example {
        label: "FromIterator without Extend",
        code: "struct Bag(Vec<u32>);\nimpl FromIterator<u32> for Bag {\n    fn from_iter<I: IntoIterator<Item = u32>>(iter: I) -> Self { Bag(iter.into_iter().collect()) }\n}",
        pass: false,
    },
    Example {
        label: "Extend without FromIterator",
        code: "struct Bag(Vec<u32>);\nimpl Extend<u32> for Bag {\n    fn extend<I: IntoIterator<Item = u32>>(&mut self, iter: I) { self.0.extend(iter) }\n}",
        pass: false,
    },
    Example {
        label: "FromIterator with Extend",
        code: "struct Bag(Vec<u32>);\nimpl FromIterator<u32> for Bag {\n    fn from_iter<I: IntoIterator<Item = u32>>(iter: I) -> Self { Bag(iter.into_iter().collect()) }\n}\nimpl Extend<u32> for Bag {\n    fn extend<I: IntoIterator<Item = u32>>(&mut self, iter: I) { self.0.extend(iter) }\n}",
        pass: true,
    },
    Example {
        label: "no collection surface",
        code: "struct Bag(Vec<u32>);\nimpl Bag {\n    fn len(&self) -> usize { self.0.len() }\n}",
        pass: true,
    },
    Example {
        label: "iter in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct Bag(Vec<u32>);\n    impl Bag {\n        fn iter(&self) -> std::slice::Iter<'_, u32> { self.0.iter() }\n    }\n}",
        pass: true,
    },
    Example {
        label: "associated fn iter without receiver",
        code: "struct Gen;\nimpl Gen {\n    fn iter() -> std::ops::Range<u32> { 0..4 }\n}",
        pass: true,
    },
    Example {
        label: "free fn iter",
        code: "fn iter() -> std::ops::Range<u32> { 0..4 }",
        pass: true,
    },
];

crate::ast_rule!(
    collection_trait_completeness,
    "Require collection trait counterparts: `iter()` needs `impl IntoIterator for &T`, `iter_mut()` needs `impl IntoIterator for &mut T`, and `FromIterator`/`Extend` come in pairs.",
    "Collections missing the matching std iterator traits break for-loops over references and generic code expecting the standard surface.",
    Low,
);

#[derive(Default)]
struct TraitImpls {
    into_iter_ref: HashSet<String>,
    into_iter_mut: HashSet<String>,
    from_iter: HashSet<String>,
    extend: HashSet<String>,
}

fn collect_trait_impls(ctx: &AstCtx<'_>) -> TraitImpls {
    let mut found = TraitImpls::default();

    for item in ctx.nodes::<ast::Impl>() {
        record_impl(&item, &mut found);
    }

    found
}

fn record_impl(item: &ast::Impl, found: &mut TraitImpls) {
    let Some(trait_name) = item.trait_().and_then(|ty| type_name(&ty)) else {
        return;
    };

    if trait_name == "IntoIterator" {
        let Some(ast::Type::RefType(reference)) = item.self_ty() else {
            return;
        };
        let Some(name) = reference.ty().and_then(|ty| type_name(&ty)) else {
            return;
        };

        if reference.mut_token().is_some() {
            found.into_iter_mut.insert(name);
        } else {
            found.into_iter_ref.insert(name);
        }
    } else if trait_name == "FromIterator" || trait_name == "Extend" {
        let Some(name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
            return;
        };

        if trait_name == "FromIterator" {
            found.from_iter.insert(name);
        } else {
            found.extend.insert(name);
        }
    }
}

fn check_collection_trait_completeness(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let impls = collect_trait_impls(ctx);

    ctx.nodes::<ast::Impl>()
        .filter(|item| !ctx.is_in_test(item))
        .flat_map(|item| match item.trait_() {
            None => check_inherent_impl(ctx, &item, &impls),
            Some(_) => check_pair_impl(ctx, &item, &impls),
        })
        .collect()
}

fn check_inherent_impl(ctx: &AstCtx<'_>, item: &ast::Impl, impls: &TraitImpls) -> Vec<Violation> {
    let Some(name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
        return Vec::new();
    };

    let associated_items = item
        .assoc_item_list()
        .into_iter()
        .flat_map(|list| list.assoc_items());

    associated_items
        .filter_map(|assoc| match assoc {
            ast::AssocItem::Fn(method) => Some(method),
            _ => None,
        })
        .filter_map(|method| {
            let method_name = method.name()?;
            let mutable = ref_receiver_mutability(&method)?;

            if method_name.text() == "iter" && !mutable && !impls.into_iter_ref.contains(&name) {
                Some(missing_counterpart(
                    ctx,
                    &method_name,
                    &name,
                    "iter()",
                    "impl IntoIterator for &",
                ))
            } else if method_name.text() == "iter_mut"
                && mutable
                && !impls.into_iter_mut.contains(&name)
            {
                Some(missing_counterpart(
                    ctx,
                    &method_name,
                    &name,
                    "iter_mut()",
                    "impl IntoIterator for &mut ",
                ))
            } else {
                None
            }
        })
        .collect()
}

fn ref_receiver_mutability(function: &ast::Fn) -> Option<bool> {
    let receiver = function.param_list()?.self_param()?;

    receiver.amp_token().map(|_| receiver.mut_token().is_some())
}

fn check_pair_impl(ctx: &AstCtx<'_>, item: &ast::Impl, impls: &TraitImpls) -> Vec<Violation> {
    let Some(trait_ty) = item.trait_() else {
        return Vec::new();
    };
    let Some(trait_name) = type_name(&trait_ty) else {
        return Vec::new();
    };
    let Some(name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
        return Vec::new();
    };

    if trait_name == "FromIterator" && !impls.extend.contains(&name) {
        vec![missing_counterpart(
            ctx,
            &trait_ty,
            &name,
            "impl FromIterator",
            "impl Extend<_> for ",
        )]
    } else if trait_name == "Extend" && !impls.from_iter.contains(&name) {
        vec![missing_counterpart(
            ctx,
            &trait_ty,
            &name,
            "impl Extend",
            "impl FromIterator<_> for ",
        )]
    } else {
        Vec::new()
    }
}

fn missing_counterpart<N>(
    ctx: &AstCtx<'_>,
    node: &N,
    type_name: &str,
    has: &str,
    needs: &str,
) -> Violation
where
    N: ra_ap_syntax::AstNode,
{
    ctx.violation(
        node,
        format!("type `{type_name}` has `{has}` but no `{needs}{type_name}` in this file"),
    )
}

crate::rulewright_ast_test!(check_collection_trait_completeness, {
    crate::example_tests!(EXAMPLES, check_collection_trait_completeness);

    #[gtest]
    fn one_violation_per_missing_counterpart() -> Result<()> {
        let v = run("struct Bag(Vec<u32>);\n\
             impl Bag {\n\
                 fn iter(&self) -> std::slice::Iter<'_, u32> { self.0.iter() }\n\
                 fn iter_mut(&mut self) -> std::slice::IterMut<'_, u32> { self.0.iter_mut() }\n\
             }");
        verify_eq!(v.len(), 2)?;

        Ok(())
    }
});
