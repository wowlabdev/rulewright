use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unrelated helper in impl",
        code: "struct Db;\nimpl Db {\n    fn check_parameters(input: &str) -> bool {\n        input.is_empty()\n    }\n}",
        pass: false,
    },
    Example {
        label: "constructor",
        code: "struct Db;\nimpl Db {\n    fn new() -> Self {\n        Db\n    }\n}",
        pass: true,
    },
    Example {
        label: "returns the impl type",
        code: "struct Db;\nimpl Db {\n    fn connect(url: &str) -> Db {\n        Db\n    }\n}",
        pass: true,
    },
    Example {
        label: "takes Self parameters",
        code: "struct Db;\nimpl Db {\n    fn merge(a: Self, b: Self) -> u32 {\n        0\n    }\n}",
        pass: true,
    },
    Example {
        label: "method with receiver",
        code: "struct Db;\nimpl Db {\n    fn query(&self) {}\n}",
        pass: true,
    },
    Example {
        label: "from_ prefixed",
        code: "struct Db;\nimpl Db {\n    fn from_code(code: u32) -> u32 {\n        code\n    }\n}",
        pass: true,
    },
    Example {
        label: "parameter type mentioning impl type",
        code: "struct Config;\nimpl Config {\n    fn validate_rule(rule: &RuleConfig) -> bool {\n        true\n    }\n}",
        pass: true,
    },
    Example {
        label: "trait impl",
        code: "struct S;\ntrait Calc {\n    fn calc(x: u32) -> u32;\n}\nimpl Calc for S {\n    fn calc(x: u32) -> u32 {\n        x\n    }\n}",
        pass: true,
    },
    Example {
        label: "unrelated helper in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct Db;\n    impl Db {\n        fn check_parameters(input: &str) -> bool {\n            input.is_empty()\n        }\n    }\n}",
        pass: true,
    },
];

const EXEMPT_NAMES: &[&str] = &["new", "default", "builder"];
const EXEMPT_PREFIXES: &[&str] = &["from_", "with_", "try_", "new_"];

crate::ast_rule!(
    assoc_fn_no_self,
    "Flag inherent associated fns that neither take nor return the impl type — make them free functions.",
    "Regular functions are first-class in Rust; computation unrelated to a receiver hosted in an impl block adds Type:: noise for no benefit.",
    Low,
);

fn check_assoc_fn_no_self(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Impl>()
        .filter(|item| !ctx.is_in_test(item) && item.trait_().is_none())
        .flat_map(|item| {
            let Some(ty) = item.self_ty().and_then(|ty| type_name(&ty)) else {
                return Vec::new();
            };

            let associated_items = item
                .assoc_item_list()
                .into_iter()
                .flat_map(|list| list.assoc_items());
            let unrelated_functions = associated_items
                .filter_map(|assoc| match assoc { ast::AssocItem::Fn(function) => Some(function), _ => None })
                .filter(|function| is_unrelated_assoc_fn(function, &ty));

            unrelated_functions
                .filter_map(|function| {
                    let name = function.name()?;

                    Some(ctx.violation(
                        &name,
                        format!(
                            "associated fn `{ty}::{name}` neither takes nor returns `{ty}` — make it a free function"
                        ),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn is_unrelated_assoc_fn(function: &ast::Fn, ty: &str) -> bool {
    if function
        .param_list()
        .is_some_and(|params| params.self_param().is_some())
    {
        return false;
    }

    let Some(name) = function.name().map(|name| name.text().to_string()) else {
        return false;
    };

    if EXEMPT_NAMES.contains(&name.as_str())
        || EXEMPT_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
    {
        return false;
    }

    !signature_mentions_type(function, ty)
}

fn signature_mentions_type(function: &ast::Fn, ty: &str) -> bool {
    let inputs = function
        .param_list()
        .map_or_else(String::new, |params| params.syntax().text().to_string());

    if inputs.contains("Self") || inputs.contains(ty) {
        return true;
    }

    function.ret_type().is_some_and(|ret| {
        let text = ret.syntax().text().to_string();

        text.contains("Self") || text.contains(ty)
    })
}

crate::rulewright_ast_test!(check_assoc_fn_no_self, {
    crate::example_tests!(EXAMPLES, check_assoc_fn_no_self);
});
