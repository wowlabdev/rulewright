use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use super::super::support::is_inside_trait;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "shared pair order flips",
        code: "fn create(tenant: u32, user: u32) {}\nfn delete(user: u32, tenant: u32) {}",
        pass: false,
    },
    Example {
        label: "flip with interleaved params",
        code: "fn f(user: u32, tenant: u32, extra: bool) {}\nfn g(flag: bool, tenant: u32, user: u32) {}",
        pass: false,
    },
    Example {
        label: "flip across impl fns",
        code: "struct S;\nimpl S {\n    fn f(&self, user: u32, tenant: u32) {}\n    fn g(&self, tenant: u32, user: u32) {}\n}",
        pass: false,
    },
    Example {
        label: "consistent order",
        code: "fn create(tenant: u32, user: u32) {}\nfn delete(tenant: u32, user: u32) {}",
        pass: true,
    },
    Example {
        label: "only one shared pair",
        code: "fn f(a: u32, b: String) {}\nfn g(b: String, c: u64) {}",
        pass: true,
    },
    Example {
        label: "same names different types",
        code: "fn f(id: u32, name: String) {}\nfn g(name: u64, id: String) {}",
        pass: true,
    },
    Example {
        label: "single-param fns",
        code: "fn f(x: u32) {}\nfn g(x: u32) {}",
        pass: true,
    },
    Example {
        label: "flip in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(user: u32, tenant: u32) {}\n    fn g(tenant: u32, user: u32) {}\n}",
        pass: true,
    },
];

crate::ast_rule!(
    param_order_consistency,
    "Flag fns whose shared parameters appear in a different order than an earlier fn in the file.",
    "The same conceptual parameters appearing in flipping orders across sibling functions invites transposed-argument bugs and raises call-site friction.",
    Low,
);

const MIN_SHARED_PAIRS: usize = 2;

type ParamPair = (String, String);

struct FnParams {
    name: String,
    params: Vec<ParamPair>,
}

fn param_pairs(function: &ast::Fn) -> Vec<ParamPair> {
    function
        .param_list()
        .into_iter()
        .flat_map(|parameters| parameters.params())
        .filter_map(|parameter| {
            let ast::Pat::IdentPat(pattern) = parameter.pat()? else {
                return None;
            };
            let ty: String = parameter
                .ty()?
                .syntax()
                .text()
                .to_string()
                .split_whitespace()
                .collect();

            Some((pattern.name()?.text().to_string(), ty))
        })
        .collect()
}

fn order_conflicts(earlier: &[ParamPair], later: &[ParamPair]) -> bool {
    let shared: Vec<&ParamPair> = earlier.iter().filter(|p| later.contains(p)).collect();

    if shared.len() < MIN_SHARED_PAIRS {
        return false;
    }

    let later_positions: Vec<usize> = shared
        .iter()
        .filter_map(|p| later.iter().position(|q| &q == p))
        .collect();

    later_positions
        .iter()
        .zip(later_positions.iter().skip(1))
        .any(|(a, b)| a > b)
}

// #rw(fn: rust_alloc_in_loop) function names are retained as owned cross-function comparison keys
fn check_param_order_consistency(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut seen = Vec::new();
    let mut violations = Vec::new();

    for function in ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function) && !is_inside_trait(function))
    {
        let params = param_pairs(&function);

        if params.len() < MIN_SHARED_PAIRS {
            continue;
        }

        let Some(name) = function.name() else {
            continue;
        };

        if let Some(earlier) = seen
            .iter()
            .find(|earlier: &&FnParams| order_conflicts(&earlier.params, &params))
        {
            violations
                .push(ctx.violation(&name, conflict_message(name.text().as_str(), &earlier.name)));
        }

        seen.push(FnParams {
            name: name.text().to_string(),
            params,
        });
    }

    violations
}

fn conflict_message(later: &str, earlier: &str) -> String {
    format!(
        "fn `{later}` orders shared parameters differently than fn `{earlier}` — keep parameter order consistent"
    )
}

crate::rulewright_ast_test!(check_param_order_consistency, {
    crate::example_tests!(EXAMPLES, check_param_order_consistency);
});
