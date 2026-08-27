use ra_ap_syntax::{
    AstNode, ast,
    ast::{HasGenericParams, HasName},
};

use crate::{AstCtx, Example, Fix, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "pure arithmetic function",
        code: "fn double(x: u32) -> u32 { x * 2 }",
        pass: false,
    },
    Example {
        label: "already const fn",
        code: "const fn double(x: u32) -> u32 { x * 2 }",
        pass: true,
    },
    Example {
        label: "function with method calls",
        code: "fn f(s: &str) -> usize { s.len() }",
        pass: true,
    },
    Example {
        label: "function with macro call",
        code: r#"fn f() -> String { format!("hello") }"#,
        pass: true,
    },
    Example {
        label: "const candidate in test",
        code: "#[cfg(test)]\nmod tests {\n    fn double(x: u32) -> u32 { x * 2 }\n}",
        pass: true,
    },
    Example {
        label: "function with if-else on params",
        code: "fn max_val(a: u32, b: u32) -> u32 { if a > b { a } else { b } }",
        pass: false,
    },
    Example {
        label: "function with loop",
        code: "fn f() -> u32 { let mut i = 0; while i < 10 { i += 1; } i }",
        pass: true,
    },
    Example {
        label: "empty function",
        code: "fn f() {}",
        pass: true,
    },
    Example {
        label: "unsafe function",
        code: "unsafe fn f(x: u32) -> u32 { x * 2 }",
        pass: true,
    },
    Example {
        label: "function with where clause",
        code: "fn f<T>(x: T) -> T where T: Copy { x }",
        pass: true,
    },
    Example {
        label: "function with impl Trait param",
        code: "fn f(x: impl Into<u32>) -> u32 { 1 }",
        pass: true,
    },
    Example {
        label: "match with literal arms is const-eligible",
        code: "fn f(a: u32) -> u32 { match a { _ => 1 } }",
        pass: false,
    },
    Example {
        label: "struct literal is const-eligible",
        code: "fn f() -> Foo { Foo { x: 1 } }",
        pass: false,
    },
    Example {
        label: "index expression is const-eligible",
        code: "fn f(a: [u32; 2]) -> u32 { a[0] }",
        pass: false,
    },
    Example {
        label: "field access is const-eligible",
        code: "fn f(p: Point) -> u32 { p.x }",
        pass: false,
    },
    Example {
        label: "tuple is const-eligible",
        code: "fn f(a: u32, b: u32) -> (u32, u32) { (a, b) }",
        pass: false,
    },
    Example {
        label: "cast is const-eligible",
        code: "fn f(a: u32) -> u8 { a as u8 }",
        pass: false,
    },
    Example {
        label: "nested block is const-eligible",
        code: "fn f() -> u32 { { 1 } }",
        pass: false,
    },
    Example {
        label: "return of literal is const-eligible",
        code: "fn f() -> u32 { return 1; }",
        pass: false,
    },
    Example {
        label: "function call is not const-eligible",
        code: "fn f() -> u32 { g() }",
        pass: true,
    },
    Example {
        label: "await is not const-eligible",
        code: "fn f() -> u32 { a.await }",
        pass: true,
    },
];

crate::ast_rule!(
    const_fn_candidate,
    "Flag pure functions that could be `const fn`.",
    "Functions that only use const-compatible operations can be const fn, enabling compile-time evaluation.",
    Low,
    fix_const_fn_candidate,
);

fn check_const_fn_candidate(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let free_functions = ctx
        .nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function) && is_free_function(function))
        .filter(|function| {
            function.const_token().is_none()
                && function.async_token().is_none()
                && function.unsafe_token().is_none()
                && function.generic_param_list().is_none()
                && function.where_clause().is_none()
                && !has_impl_trait_params(function)
                && function.ret_type().is_some()
                && function
                    .body()
                    .is_some_and(|body| is_const_eligible_block(&body))
        });

    free_functions
        .filter_map(|function| {
            let name = function.name()?;

            Some(ctx.violation(
                &name,
                format!(
                    "`{name}` could be a `const fn` — it only uses const-compatible operations"
                ),
            ))
        })
        .collect()
}

fn is_const_eligible_block(block: &ast::BlockExpr) -> bool {
    let blocks_nonempty = block
        .syntax()
        .descendants()
        .filter_map(ast::BlockExpr::cast)
        .all(|block| {
            block.stmt_list().is_some_and(|list| {
                list.statements().next().is_some() || list.tail_expr().is_some()
            })
        });
    let no_items = !block
        .syntax()
        .descendants()
        .filter_map(ast::Stmt::cast)
        .any(|stmt| matches!(stmt, ast::Stmt::Item(_)));

    blocks_nonempty
        && no_items
        && block
            .syntax()
            .descendants()
            .filter_map(ast::Expr::cast)
            .all(|expr| {
                matches!(
                    expr,
                    ast::Expr::Literal(_)
                        | ast::Expr::PathExpr(_)
                        | ast::Expr::BinExpr(_)
                        | ast::Expr::PrefixExpr(_)
                        | ast::Expr::ParenExpr(_)
                        | ast::Expr::IfExpr(_)
                        | ast::Expr::BlockExpr(_)
                        | ast::Expr::CastExpr(_)
                        | ast::Expr::TupleExpr(_)
                        | ast::Expr::IndexExpr(_)
                        | ast::Expr::FieldExpr(_)
                        | ast::Expr::RecordExpr(_)
                        | ast::Expr::ReturnExpr(_)
                        | ast::Expr::MatchExpr(_)
                )
            })
}

fn has_impl_trait_params(function: &ast::Fn) -> bool {
    function
        .param_list()
        .into_iter()
        .flat_map(|params| params.params())
        .any(|param| matches!(param.ty(), Some(ast::Type::ImplTraitType(_))))
}

fn is_free_function(function: &ast::Fn) -> bool {
    function
        .syntax()
        .parent()
        .is_none_or(|parent| !ast::AssocItemList::can_cast(parent.kind()))
}

fn fix_const_fn_candidate(ctx: &AstCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.file.line(v.line)?;

    Some(Fix::replace_line(
        v.line,
        line.replacen("fn ", "const fn ", 1),
    ))
}

crate::rulewright_ast_test!(check_const_fn_candidate, {
    crate::example_tests!(EXAMPLES, check_const_fn_candidate);
    crate::fix_tests!(ast, check_const_fn_candidate, fix_const_fn_candidate);
});
