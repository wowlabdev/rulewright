use ra_ap_syntax::{
    AstNode,
    ast::{self, HasArgList, HasAttrs, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "thin function-like shim",
        code: "#[proc_macro]\npub fn my_macro(input: TokenStream) -> TokenStream {\n    my_macro_impl::my_macro(input.into()).into()\n}",
        pass: true,
    },
    Example {
        label: "thin attribute shim with two args",
        code: "#[proc_macro_attribute]\npub fn route(attr: TokenStream, item: TokenStream) -> TokenStream {\n    route_impl::route(attr.into(), item.into()).into()\n}",
        pass: true,
    },
    Example {
        label: "thin derive shim",
        code: "#[proc_macro_derive(Model, attributes(model))]\npub fn derive_model(input: TokenStream) -> TokenStream {\n    model_impl::derive_model(input.into()).into()\n}",
        pass: true,
    },
    Example {
        label: "inline expansion logic",
        code: "#[proc_macro]\npub fn my_macro(input: TokenStream) -> TokenStream {\n    let text = input.to_string();\n    text.parse().unwrap_or_default()\n}",
        pass: false,
    },
    Example {
        label: "delegation without impl crate",
        code: "#[proc_macro]\npub fn my_macro(input: TokenStream) -> TokenStream {\n    expand(input)\n}",
        pass: false,
    },
    Example {
        label: "plain fn with any body",
        code: "fn helper(input: String) -> String { input }",
        pass: true,
    },
];

crate::ast_rule!(
    proc_macro_thin_shim,
    "Require proc-macro entry points to be thin `impl_crate::name(arg.into()).into()` shims.",
    "Token-stream logic living behind #[proc_macro] cannot be unit- or snapshot-tested — it belongs in a separate impl crate.",
    Low,
);

const PROC_MACRO_ATTRS: &[&str] = &["proc_macro", "proc_macro_derive", "proc_macro_attribute"];
const MIN_DELEGATE_SEGMENTS: usize = 2;

fn check_proc_macro_thin_shim(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::Fn>()
        .filter(|function| {
            !ctx.is_in_test(function) && is_proc_macro_fn(function) && !is_thin_shim(function)
        })
        .map(|function| {
            let name = function
                .name()
                .map_or_else(String::new, |name| name.text().to_string());

            ctx.violation(
                &function,
                format!(
                    "proc macro `{name}` is not a thin shim — delegate to a separate impl \
                     crate: `foo_impl::{name}(input.into()).into()`"
                ),
            )
        })
        .collect()
}

fn is_proc_macro_fn(function: &ast::Fn) -> bool {
    function.attrs().any(|attr| {
        attr.simple_name()
            .is_some_and(|name| PROC_MACRO_ATTRS.contains(&name.as_str()))
    })
}

fn is_thin_shim(function: &ast::Fn) -> bool {
    let Some(statements) = function.body().and_then(|body| body.stmt_list()) else {
        return false;
    };

    if statements.statements().next().is_some() {
        return false;
    }

    let Some(ast::Expr::MethodCallExpr(into_call)) = statements.tail_expr() else {
        return false;
    };

    if into_call
        .name_ref()
        .is_none_or(|name| name.text() != "into")
        || into_call
            .arg_list()
            .is_some_and(|args| args.args().next().is_some())
    {
        return false;
    }

    let Some(ast::Expr::CallExpr(delegate)) = into_call.receiver() else {
        return false;
    };
    let Some(ast::Expr::PathExpr(path_expr)) = delegate.expr() else {
        return false;
    };

    if path_expr
        .path()
        .is_none_or(|path| path.syntax().to_string().split("::").count() < MIN_DELEGATE_SEGMENTS)
    {
        return false;
    }

    delegate.arg_list().is_some_and(|arguments| {
        arguments
            .args()
            .all(|argument| is_into_adapted_arg(&argument))
    })
}

fn is_into_adapted_arg(arg: &ast::Expr) -> bool {
    match arg {
        ast::Expr::PathExpr(_) => true,

        ast::Expr::MethodCallExpr(call) => {
            call.name_ref().is_some_and(|name| name.text() == "into")
                && call
                    .arg_list()
                    .is_none_or(|args| args.args().next().is_none())
                && matches!(call.receiver(), Some(ast::Expr::PathExpr(_)))
        }

        _ => false,
    }
}

crate::rulewright_ast_test!(check_proc_macro_thin_shim, {
    crate::example_tests!(EXAMPLES, check_proc_macro_thin_shim);
});
