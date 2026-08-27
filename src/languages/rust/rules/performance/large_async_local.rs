use ra_ap_syntax::ast;

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "large repeat-literal array in async fn",
        code: "async fn f() { let buf = [0u8; 2048]; consume(&buf).await; }",
        pass: false,
    },
    Example {
        label: "large annotated array local in async fn",
        code: "async fn f() { let buf: [u64; 512] = [0; 512]; consume(&buf).await; }",
        pass: false,
    },
    Example {
        label: "large by-value array parameter on async fn",
        code: "async fn f(buf: [u8; 4096]) { consume(&buf).await; }",
        pass: false,
    },
    Example {
        label: "large array local in async block",
        code: "fn g() { let _fut = async { let buf = [0u8; 2048]; consume(&buf).await; }; }",
        pass: false,
    },
    Example {
        label: "small array local in async fn",
        code: "async fn f() { let buf = [0u8; 128]; consume(&buf).await; }",
        pass: true,
    },
    Example {
        label: "sync fn is rust_large_stack_array territory",
        code: "fn f() { let _buf = [0u8; 4096]; }",
        pass: true,
    },
    Example {
        label: "array behind a reference parameter",
        code: "async fn f(buf: &[u8; 4096]) { consume(buf).await; }",
        pass: true,
    },
    Example {
        label: "large array inside sync closure in async fn",
        code: "async fn f() { let g = || { let _buf = [0u8; 4096]; }; g(); }",
        pass: true,
    },
    Example {
        label: "large async local in test module",
        code: "#[cfg(test)]\nmod tests {\n    async fn t() { let _buf = [0u8; 2048]; }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    large_async_local,
    "Flag by-value `[T; N]` locals and parameters over threshold bytes inside async fns and blocks.",
    "Async locals and parameters embed in the future's state machine, inflating every task and spawn memcpy.",
    Medium,
    params {
        threshold: i64 = 1024
    },
);

fn check_large_async_local(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let threshold = ctx
        .file
        .config
        .get_u64("rust_large_async_local", &PARAMS[0]);
    let locals = ctx
        .nodes::<ast::LetStmt>()
        .filter(|local| !ctx.is_in_test(local) && support::is_in_async_context(local))
        .filter_map(|local| {
            let size = local_array_size(&local)?;

            (size > threshold).then(|| {
                ctx.violation(
                    &local,
                    format!(
                        "large array local ({size} bytes) in async context — lives in the future state machine; Box it or shrink it"
                    ),
                )
            })
        });
    let parameters = ctx
        .nodes::<ast::Fn>()
        .filter(|function| {
            !ctx.is_in_test(function)
                && function.body().is_some()
                && function.async_token().is_some()
        })
        .flat_map(|function| {
            function
                .param_list()
                .into_iter()
                .flat_map(|parameters| parameters.params())
                .filter_map(move |parameter| param_violation(ctx, &parameter, threshold))
        });

    locals.chain(parameters).collect()
}

fn local_array_size(local: &ast::LetStmt) -> Option<u64> {
    if let Some(ty @ ast::Type::ArrayType(_)) = local.ty() {
        return support::estimate_type_size(&ty);
    }

    let ast::Expr::ArrayExpr(array) = local.initializer()? else {
        return None;
    };

    array.semicolon_token()?;

    let mut expressions = array.exprs();
    let element = expressions.next()?;
    let length = expressions.next()?;

    Some(support::literal_elem_size(&element)?.saturating_mul(support::parse_int_expr(&length)?))
}

fn param_violation(ctx: &AstCtx<'_>, parameter: &ast::Param, threshold: u64) -> Option<Violation> {
    let ty @ ast::Type::ArrayType(_) = parameter.ty()? else {
        return None;
    };
    let size = support::estimate_type_size(&ty)?;

    if size <= threshold {
        return None;
    }

    Some(ctx.violation(
        parameter,
        format!(
            "by-value array parameter ({size} bytes) on async fn — embedded in every future instance; pass a reference or Box"
        ),
    ))
}

crate::rulewright_ast_test!(check_large_async_local, {
    crate::example_tests!(EXAMPLES, check_large_async_local);
});
