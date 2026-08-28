#[cfg(test)]
use googletest::prelude::*;
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
    Example {
        label: "method calling a free function with the same name",
        code: "fn quote(_: &str) {} struct S; impl S { fn quote(&self, text: &str) { quote(text); } }",
        pass: true,
    },
    Example {
        label: "method recursion through self",
        code: "struct S; impl S { fn go(&self) { self.go(); } }",
        pass: false,
    },
    Example {
        label: "trait method forwarding to an inherent method",
        code: "struct Client; trait Remote { fn unlink(&self); } impl Remote for Client { fn unlink(&self) { self.unlink(); } }",
        pass: true,
    },
    Example {
        label: "bounded structural recursion still needs an explicit suppression",
        code: "fn visit(node: &Node, depth: usize, max_depth: usize) { if depth > max_depth { return; } for child in node.children() { visit(child, depth + 1, max_depth); } }",
        pass: false,
    },
];

crate::ast_rule!(
    recursive_fn,
    "Flag direct self-recursion (stack overflow risk, especially in WASM).",
    "Direct recursion risks stack overflow, especially in WASM with a fixed stack. Use iteration or trampolining when that preserves the function's contracts; bounded structural recursion may instead be suppressed with a reason that names the enforced depth limit.",
    High,
);

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

        let recursive_path = function
            .syntax()
            .descendants()
            .filter_map(ast::CallExpr::cast)
            .filter(|call| enclosing_function(call).as_ref() == Some(&function))
            .filter_map(|call| call.expr())
            .filter_map(|callee| match callee {
                ast::Expr::PathExpr(path) => path.path(),
                _ => None,
            })
            .any(|path| is_self_call(&path, &function_name, impl_type.as_deref()));
        let recursive_method = !is_in_trait_impl(&function)
            && function
                .syntax()
                .descendants()
                .filter_map(ast::MethodCallExpr::cast)
                .filter(|call| enclosing_function(call).as_ref() == Some(&function))
                .any(|call| is_self_method_call(&call, &function_name));

        if recursive_path || recursive_method {
            violations.push(ctx.violation(
                &name,
                format!(
                    "direct self-recursion in `{function_name}` — risk of stack overflow (especially in WASM)"
                ),
            ));
        }
    }

    violations
}

fn is_in_trait_impl(function: &ast::Fn) -> bool {
    function
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::Impl::cast)
        .is_some_and(|item| item.trait_().is_some())
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
        return impl_type.is_none();
    };

    if qualifier.qualifier().is_some() {
        return false;
    }

    qualifier
        .segment()
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| name.text() == "Self" || impl_type.is_some_and(|ty| name.text() == ty))
}

fn is_self_method_call(call: &ast::MethodCallExpr, fn_name: &str) -> bool {
    call.name_ref().is_some_and(|name| name.text() == fn_name)
        && call.receiver().is_some_and(|receiver| {
            let ast::Expr::PathExpr(path) = receiver else {
                return false;
            };

            path.path()
                .and_then(|path| path.as_single_name_ref())
                .is_some_and(|name| name.text() == "self")
        })
}

crate::rulewright_ast_test!(check_recursive_fn, {
    crate::example_tests!(EXAMPLES, check_recursive_fn);

    #[gtest]
    fn one_function_with_multiple_recursive_calls_reports_once() -> Result<()> {
        let violations =
            run("fn visit(value: bool) { if value { visit(false); } else { visit(true); } }");

        verify_eq!(violations.len(), 1)?;

        Ok(())
    }
});
