use ra_ap_syntax::{ast, ast::HasName};
use std::collections::{HashMap, HashSet};

use super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "forwarding trait impl",
        code: "trait Download { fn get(&self); }\nstruct Client;\nimpl Client {\n    fn get(&self) {\n        let a = 1;\n        let b = a + 1;\n        let c = b + 1;\n        let _ = c;\n    }\n}\nimpl Download for Client {\n    fn get(&self) { Self::get(self) }\n}",
        pass: true,
    },
    Example {
        label: "logic in trait impl without inherent method",
        code: "trait Download { fn get(&self); }\nstruct Client;\nimpl Download for Client {\n    fn get(&self) {\n        let a = 1;\n        let b = a + 1;\n        let c = b + 1;\n        let _ = c;\n    }\n}",
        pass: false,
    },
    Example {
        label: "logic with same-named inherent method elsewhere",
        code: "trait Download { fn get(&self); }\nstruct Client;\nimpl Client {\n    fn get(&self) {}\n}\nimpl Download for Client {\n    fn get(&self) {\n        let a = 1;\n        let b = a + 1;\n        let c = b + 1;\n        let _ = c;\n    }\n}",
        pass: true,
    },
    Example {
        label: "foreign trait impl exempt",
        code: "struct Client;\nimpl std::fmt::Display for Client {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        let a = 1;\n        let b = a + 1;\n        let c = b + 1;\n        write!(f, \"{c}\")\n    }\n}",
        pass: true,
    },
    Example {
        label: "body at threshold",
        code: "trait Download { fn get(&self); }\nstruct Client;\nimpl Download for Client {\n    fn get(&self) {\n        let a = 1;\n        let b = a + 1;\n        let _ = b;\n    }\n}",
        pass: true,
    },
    Example {
        label: "logic in test module",
        code: "#[cfg(test)]\nmod tests {\n    trait Download { fn get(&self); }\n    struct Client;\n    impl Download for Client {\n        fn get(&self) {\n            let a = 1;\n            let b = a + 1;\n            let c = b + 1;\n            let _ = c;\n        }\n    }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    trait_logic_not_inherent,
    "Flag substantial logic in impls of locally-defined traits when the type has no same-named inherent method.",
    "Essential functionality buried in trait impls forces users to import the trait to call it; implement inherently and forward from the trait.",
    Low,
    params { threshold: i64 = 3 },
);

struct LocalFacts {
    traits: HashSet<String>,
    inherent: HashMap<String, HashSet<String>>,
}

// #rw(fn: rust_alloc_in_loop) the collector owns each trait name it stores.
fn collect_local_facts(ctx: &AstCtx<'_>) -> LocalFacts {
    let mut facts = LocalFacts {
        traits: HashSet::default(),
        inherent: HashMap::default(),
    };

    for item in ctx.nodes::<ast::Trait>() {
        if let Some(name) = item.name() {
            facts.traits.insert(name.text().to_string());
        }
    }

    for item in ctx
        .nodes::<ast::Impl>()
        .filter(|item| item.trait_().is_none())
    {
        record_inherent_methods(&item, &mut facts);
    }

    facts
}

// #rw(fn: rust_alloc_in_loop) the collector owns each method name it stores.
fn record_inherent_methods(item: &ast::Impl, facts: &mut LocalFacts) {
    let Some(name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
        return;
    };
    let methods = facts.inherent.entry(name).or_default();

    for method in item
        .assoc_item_list()
        .into_iter()
        .flat_map(|list| list.assoc_items())
        .filter_map(|assoc| match assoc {
            ast::AssocItem::Fn(method) => Some(method),
            _ => None,
        })
    {
        if let Some(name) = method.name() {
            methods.insert(name.text().to_string());
        }
    }
}

fn check_trait_logic_not_inherent(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx
        .file
        .config
        .get_usize("rust_trait_logic_not_inherent", &PARAMS[0]);
    let facts = collect_local_facts(ctx);

    ctx.nodes::<ast::Impl>()
        .filter(|item| !ctx.is_in_test(item))
        .flat_map(|item| check_impl(ctx, &item, &facts, threshold))
        .collect()
}

fn check_impl(
    ctx: &AstCtx<'_>,
    item: &ast::Impl,
    facts: &LocalFacts,
    threshold: usize,
) -> Vec<Violation> {
    let Some(trait_name) = item.trait_().and_then(|ty| type_name(&ty)) else {
        return Vec::new();
    };

    if !facts.traits.contains(&trait_name) {
        return Vec::new();
    }

    let Some(self_name) = item.self_ty().and_then(|ty| type_name(&ty)) else {
        return Vec::new();
    };
    let inherent = facts.inherent.get(&self_name);

    let associated_items = item
        .assoc_item_list()
        .into_iter()
        .flat_map(|list| list.assoc_items());

    associated_items
        .filter_map(|assoc| match assoc { ast::AssocItem::Fn(method) => Some(method), _ => None })
        .filter_map(|method| {
            let name = method.name()?;
            let statements = method.body()?.stmt_list()?.statements().count();

            if statements <= threshold || inherent.is_some_and(|methods| methods.contains(name.text().as_str())) { return None }

            Some(ctx.violation(&name, format!(
                "trait method `{trait_name}::{name}` for `{self_name}` holds {statements} statements of logic — implement inherently and forward"
            )))
        }).collect()
}

crate::rulewright_ast_test!(check_trait_logic_not_inherent, {
    crate::example_tests!(EXAMPLES, check_trait_logic_not_inherent);
});
