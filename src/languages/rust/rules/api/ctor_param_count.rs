use ra_ap_syntax::{ast, ast::HasName};

use super::support::type_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "constructor with four parameters",
        code: "struct Deposit;\nimpl Deposit {\n    pub fn new(account: Account, amount: Currency, memo: Memo, clock: Clock) -> Self {\n        Deposit\n    }\n}",
        pass: false,
    },
    Example {
        label: "three consecutive str parameters",
        code: "struct Deposit;\nimpl Deposit {\n    pub fn new(bank: &str, customer: &str, currency: &str) -> Self {\n        Deposit\n    }\n}",
        pass: false,
    },
    Example {
        label: "with_ constructor returning type name",
        code: "struct Deposit;\nimpl Deposit {\n    pub fn with_parts(a: Account, b: Currency, c: Memo, d: Clock) -> Deposit {\n        Deposit\n    }\n}",
        pass: false,
    },
    Example {
        label: "constructor with two parameters",
        code: "struct Deposit;\nimpl Deposit {\n    pub fn new(account: Account, amount: Currency) -> Self {\n        Deposit\n    }\n}",
        pass: true,
    },
    Example {
        label: "mixed types below threshold",
        code: "struct Deposit;\nimpl Deposit {\n    pub fn new(amount: u64, bank: &str, id: u32) -> Self {\n        Deposit\n    }\n}",
        pass: true,
    },
    Example {
        label: "non-constructor name",
        code: "struct Acc;\nimpl Acc {\n    fn combine(a: u32, b: u32, c: u32, d: u32) -> u32 {\n        a + b + c + d\n    }\n}",
        pass: true,
    },
    Example {
        label: "chainable with_ method has receiver",
        code: "struct Acc;\nimpl Acc {\n    pub fn with_size(mut self, a: u32, b: u32, c: u32, d: u32) -> Self {\n        self\n    }\n}",
        pass: true,
    },
    Example {
        label: "trait impl constructor",
        code: "struct Acc;\ntrait Make {\n    fn new_from(a: u32, b: u32, c: u32, d: u32) -> Self;\n}\nimpl Make for Acc {\n    fn new_from(a: u32, b: u32, c: u32, d: u32) -> Self {\n        Acc\n    }\n}",
        pass: true,
    },
    Example {
        label: "wide constructor in test module",
        code: "#[cfg(test)]\nmod tests {\n    struct Acc;\n    impl Acc {\n        pub fn new(a: u32, b: u32, c: u32, d: u32) -> Self {\n            Acc\n        }\n    }\n}",
        pass: true,
    },
];

const MIN_SAME_TYPE_RUN: usize = 3;

const PRIMITIVES: &[&str] = &[
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32",
    "u64", "u128", "usize",
];

crate::ast_rule!(
    ctor_param_count,
    "Flag constructors with too many parameters or runs of identically-typed primitives — cascade construction through helper types.",
    "Long or same-typed constructor parameter lists invite silent argument mix-ups; grouping parameters semantically makes construction self-checking.",
    Medium,
    params { threshold: i64 = 4 },
);

fn check_ctor_param_count(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx
        .file
        .config
        .get_usize("rust_ctor_param_count", &PARAMS[0]);

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
            let constructors = associated_items
                .filter_map(|assoc| match assoc {
                    ast::AssocItem::Fn(function) => Some(function),
                    _ => None,
                })
                .filter(|function| is_ctor(function, &ty));

            constructors
                .flat_map(|function| ctor_violations(ctx, &function, &ty, threshold))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn is_ctor(function: &ast::Fn, ty: &str) -> bool {
    if function
        .param_list()
        .is_some_and(|params| params.self_param().is_some())
    {
        return false;
    }

    let Some(name) = function.name().map(|name| name.text().to_string()) else {
        return false;
    };
    let ctor_name = name == "new" || name.starts_with("new_") || name.starts_with("with_");

    let return_type = function
        .ret_type()
        .and_then(|ret| ret.ty())
        .and_then(|ty| type_name(&ty));

    ctor_name && return_type.is_some_and(|name| name == "Self" || name == ty)
}

fn longest_primitive_run(function: &ast::Fn) -> Option<(&'static str, usize)> {
    let mut best: Option<(&'static str, usize)> = None;
    let mut current: Option<(&'static str, usize)> = None;

    for param in function
        .param_list()
        .into_iter()
        .flat_map(|params| params.params())
    {
        let key = param.ty().and_then(|ty| primitive_key(&ty));

        current = match (current, key) {
            (Some((run, len)), Some(next)) if run == next => Some((run, len + 1)),
            (_, Some(next)) => Some((next, 1)),
            (_, None) => None,
        };

        if let Some((run, len)) = current
            && best.is_none_or(|(_, best_len)| len > best_len)
        {
            best = Some((run, len));
        }
    }

    best
}

fn primitive_key(ty: &ast::Type) -> Option<&'static str> {
    if let ast::Type::RefType(reference) = ty {
        if reference.ty().and_then(|ty| type_name(&ty)).as_deref() == Some("str") {
            return Some("&str");
        }

        return None;
    }

    let name = type_name(ty)?;

    PRIMITIVES
        .iter()
        .copied()
        .find(|primitive| *primitive == name)
}

fn ctor_violations(
    ctx: &AstCtx<'_>,
    function: &ast::Fn,
    ty: &str,
    threshold: usize,
) -> Vec<Violation> {
    let Some(name) = function.name() else {
        return Vec::new();
    };
    let count = function
        .param_list()
        .map_or(0, |params| params.params().count());
    let mut out = Vec::new();

    if count >= threshold {
        out.push(ctx.violation(&name, format!(
            "constructor `{ty}::{name}` takes {count} parameters (max {}) — group them semantically via helper types", threshold - 1
        )));
    }

    if let Some((run_ty, run_len)) = longest_primitive_run(function)
        && run_len >= MIN_SAME_TYPE_RUN
    {
        out.push(ctx.violation(&name, format!(
            "constructor `{ty}::{name}` has {run_len} consecutive `{run_ty}` parameters — mix-up risk, use distinct types"
        )));
    }

    out
}

crate::rulewright_ast_test!(check_ctor_param_count, {
    crate::example_tests!(EXAMPLES, check_ctor_param_count);
});
