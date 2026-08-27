use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "direct recursion (bare call)",
        code: "fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }",
        pass: false,
    },
    Example {
        label: "Self:: recursion",
        code: "struct S;\nimpl S { fn go(&self) { Self::go(self); } }",
        pass: false,
    },
    Example {
        label: "no recursion",
        code: "fn add(a: u64, b: u64) -> u64 { a + b }",
        pass: true,
    },
    Example {
        label: "recursion in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }\n}",
        pass: true,
    },
    Example {
        label: "calling different function",
        code: "fn foo() { bar(); }\nfn bar() {}",
        pass: true,
    },
    Example {
        label: "constructor calling other type constructors",
        code: "struct S { v: Vec<u8> }\nimpl S { fn new() -> Self { Self { v: Vec::new() } } }",
        pass: true,
    },
    Example {
        label: "Default impl calling other defaults",
        code: "struct S { v: Vec<u8> }\nimpl Default for S { fn default() -> Self { Self { v: Vec::default() } } }",
        pass: true,
    },
    Example {
        label: "qualified trait delegation",
        code: "struct S; trait T { fn go(); } impl T for S { fn go() { <u8 as T>::go(); } }",
        pass: true,
    },
];

crate::ast_rule!(
    recursive_fn,
    "Flag direct self-recursion (stack overflow risk, especially in WASM).",
    "Direct recursion risks stack overflow, especially in WASM with its fixed 1MB stack. Use iteration or trampolining.",
    High,
);

// #rw(fn: rust_alloc_in_loop) diagnostics own function names and messages after a confirmed recursive call
fn check_recursive_fn(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for function in ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
    {
        let Some(name) = function.name() else {
            continue;
        };
        let function_name = name.text().to_string();
        let impl_type = function
            .syntax()
            .ancestors()
            .skip(1)
            .find_map(ast::Impl::cast)
            .and_then(|item_impl| item_impl.self_ty())
            .and_then(|ty| support::self_type_name(&ty));

        for call in function
            .syntax()
            .descendants()
            .filter_map(ast::CallExpr::cast)
        {
            if enclosing_function(&call).as_ref() != Some(&function) {
                continue;
            }

            let Some(path) = call.expr().and_then(|callee| match callee {
                ast::Expr::PathExpr(path) => path.path(),
                _ => None,
            }) else {
                continue;
            };

            if is_self_call(&path, &function_name, impl_type.as_deref()) {
                let message = format!(
                    "direct self-recursion in `{function_name}` — risk of stack overflow (especially in WASM)"
                );

                if let Some(location) = path.segment().and_then(|segment| segment.name_ref()) {
                    violations.push(ctx.violation(&location, message));
                } else {
                    violations.push(ctx.violation(&call, message));
                }
            }
        }
    }

    violations
}

fn enclosing_function<N>(node: &N) -> Option<ast::Fn>
where
    N: AstNode,
{
    node.syntax().ancestors().skip(1).find_map(ast::Fn::cast)
}

fn is_self_call(path: &ast::Path, fn_name: &str, impl_type: Option<&str>) -> bool {
    if path
        .syntax()
        .descendants()
        .any(|node| ast::TypeAnchor::cast(node).is_some())
    {
        return false;
    }

    let Some(called) = path.segment().and_then(|segment| segment.name_ref()) else {
        return false;
    };

    if called.text() != fn_name {
        return false;
    }

    let Some(qualifier) = path.qualifier() else {
        return true;
    };

    if qualifier.qualifier().is_some() {
        return false;
    }

    qualifier
        .segment()
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| name.text() == "Self" || impl_type.is_some_and(|ty| name.text() == ty))
}

crate::rulewright_ast_test!(check_recursive_fn, {
    crate::example_tests!(EXAMPLES, check_recursive_fn);
});
