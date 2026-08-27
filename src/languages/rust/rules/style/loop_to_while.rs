use ra_ap_syntax::{ast, ast::HasLoopBody};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "loop with break-if at start",
        code: "fn f() { loop { if !cond() { break; } do_work(); } }",
        pass: false,
    },
    Example {
        label: "loop with break-if-not at start",
        code: "fn f() { loop { if done() { break; } do_work(); } }",
        pass: false,
    },
    Example {
        label: "while loop is fine",
        code: "fn f() { while cond() { do_work(); } }",
        pass: true,
    },
    Example {
        label: "loop with break in middle",
        code: "fn f() { loop { do_work(); if done() { break; } more_work(); } }",
        pass: true,
    },
    Example {
        label: "loop with no break-if",
        code: "fn f() { loop { do_work(); break; } }",
        pass: true,
    },
    Example {
        label: "break-if after a let statement",
        code: "fn f() { loop { let value = next(); if value == 0 { break; } } }",
        pass: true,
    },
    Example {
        label: "break-if body has another statement",
        code: "fn f() { loop { if done() { note(); break; } do_work(); } }",
        pass: true,
    },
    Example {
        label: "loop with break value",
        code: "fn f() -> i32 { loop { if done() { break 42; } } }",
        pass: true,
    },
    Example {
        label: "loop in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f() { loop { if !cond() { break; } do_work(); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    loop_to_while,
    "Flag `loop { if cond { break; } ... }` — use `while` instead.",
    "A loop with a break-if guard as the first statement is a while loop in disguise. while is clearer and less error-prone.",
    Low,
);

fn check_loop_to_while(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::LoopExpr>()
        .filter(|loop_expr| {
            !ctx.is_in_test(loop_expr)
                && loop_expr.label().is_none()
                && loop_expr
                    .loop_body()
                    .is_some_and(|body| is_break_guard(&body))
        })
        .map(|loop_expr| {
            ctx.violation(&loop_expr, "loop with break-if guard — use `while` instead")
        })
        .collect()
}

fn is_bare_break(expr: &ast::Expr) -> bool {
    matches!(expr, ast::Expr::BreakExpr(expr) if expr.lifetime().is_none() && expr.expr().is_none())
}

fn is_break_guard(block: &ast::BlockExpr) -> bool {
    let Some(statements) = block.stmt_list() else {
        return false;
    };
    let Some(ast::Expr::IfExpr(if_expr)) = first_expr(&statements) else {
        return false;
    };

    if if_expr.else_branch().is_some() {
        return false;
    }

    let Some(then_statements) = if_expr.then_branch().and_then(|body| body.stmt_list()) else {
        return false;
    };

    sole_expr(&then_statements).is_some_and(|expr| is_bare_break(&expr))
}

fn first_expr(statements: &ast::StmtList) -> Option<ast::Expr> {
    match statements.statements().next() {
        Some(ast::Stmt::ExprStmt(statement)) => statement.expr(),
        Some(_) => None,
        None => statements.tail_expr(),
    }
}

fn sole_expr(statements: &ast::StmtList) -> Option<ast::Expr> {
    let mut body = statements.statements();

    match (body.next(), body.next(), statements.tail_expr()) {
        (Some(ast::Stmt::ExprStmt(statement)), None, None) => statement.expr(),
        (None, None, Some(expression)) => Some(expression),
        _ => None,
    }
}

crate::rulewright_ast_test!(check_loop_to_while, {
    crate::example_tests!(EXAMPLES, check_loop_to_while);
});
