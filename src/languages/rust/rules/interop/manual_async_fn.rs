use ra_ap_syntax::ast::{self, HasName, TypeBoundKind};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "impl Future wrapping one async block",
        code: "fn f() -> impl Future<Output = u8> { async { 1 } }",
        pass: false,
    },
    Example {
        label: "qualified Future with async move block",
        code: "fn f() -> impl std::future::Future<Output = ()> { async move { } }",
        pass: false,
    },
    Example {
        label: "async fn",
        code: "async fn f() -> u8 { 1 }",
        pass: true,
    },
    Example {
        label: "body does more than one async block",
        code: "fn f() -> impl Future<Output = u8> { let x = 1; async move { x } }",
        pass: true,
    },
    Example {
        label: "trait declaration",
        code: "trait T { fn f() -> impl Future<Output = ()>; }",
        pass: true,
    },
    Example {
        label: "trait default body",
        code: "trait T { fn f() -> impl Future<Output = ()> { async { } } }",
        pass: true,
    },
    Example {
        label: "trait impl block",
        code: "struct S;\nimpl T for S { fn f() -> impl Future<Output = ()> { async { } } }",
        pass: true,
    },
    Example {
        label: "inherent impl method",
        code: "struct S;\nimpl S { fn f() -> impl Future<Output = ()> { async { } } }",
        pass: false,
    },
    Example {
        label: "non-Future impl trait return",
        code: "fn f() -> impl Iterator<Item = u8> { std::iter::once(1) }",
        pass: true,
    },
    Example {
        label: "manual future in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f() -> impl Future<Output = ()> { async { } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    manual_async_fn,
    "Flag non-async functions that return `impl Future` by wrapping the whole body in one `async` block.",
    "`async fn` reads normally and avoids signature noise; explicit `impl Future` returns are only warranted in traits or for hot-path size control.",
    Low,
);

fn check_manual_async_fn(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let eligible_functions = ctx
        .nodes::<ast::Fn>()
        .filter(|function| {
            super::support::is_item_or_impl_fn(function) && !ctx.is_in_test(function)
        })
        .filter(|function| !super::support::is_inside_trait(function));
    let future_functions = eligible_functions
        .filter(|function| {
            super::support::enclosing_impl(function)
                .is_none_or(|item_impl| item_impl.trait_().is_none())
        })
        .filter(|function| function.async_token().is_none() && returns_impl_future(function));

    future_functions
        .filter(body_is_single_async_block)
        .filter_map(|function| {
            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "fn `{name}` returns `impl Future` from a single `async` block — declare it `async fn` instead"
                ),
            ))
        })
        .collect()
}

fn returns_impl_future(function: &ast::Fn) -> bool {
    let Some(ast::Type::ImplTraitType(impl_trait)) = function.ret_type().and_then(|ret| ret.ty())
    else {
        return false;
    };

    impl_trait.type_bound_list().is_some_and(|bounds| {
        bounds.bounds().any(|bound| {
            matches!(
                bound.kind(),
                Some(TypeBoundKind::PathType(_, path_type))
                    if path_type.path().is_some_and(|path| super::support::path_last_is(path, "Future"))
            )
        })
    })
}

fn body_is_single_async_block(function: &ast::Fn) -> bool {
    let Some(statements) = function.body().and_then(|body| body.stmt_list()) else {
        return false;
    };
    let stmts: Vec<_> = statements.statements().collect();

    match (stmts.as_slice(), statements.tail_expr()) {
        ([], Some(ast::Expr::BlockExpr(block))) => block.async_token().is_some(),

        ([ast::Stmt::ExprStmt(statement)], None) => {
            matches!(statement.expr(), Some(ast::Expr::BlockExpr(block)) if block.async_token().is_some())
        }

        _ => false,
    }
}

crate::rulewright_ast_test!(check_manual_async_fn, {
    crate::example_tests!(EXAMPLES, check_manual_async_fn);
});
