use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Rc::new held across await",
        code: "async fn f() { let rc = Rc::new(1); g().await; }",
        pass: false,
    },
    Example {
        label: "RefCell::new held across await",
        code: "async fn f() { let cell = RefCell::new(1); g().await; }",
        pass: false,
    },
    Example {
        label: "Rc::clone held across await",
        code: "async fn f(other: &Rc<u8>) { let rc = Rc::clone(other); g().await; }",
        pass: false,
    },
    Example {
        label: "Rc in async block before await",
        code: "fn f() { let fut = async { let rc = Rc::new(1); g().await; }; }",
        pass: false,
    },
    Example {
        label: "await before the Rc binding",
        code: "async fn f() { g().await; let rc = Rc::new(1); }",
        pass: true,
    },
    Example {
        label: "Arc is Send",
        code: "async fn f() { let arc = Arc::new(1); g().await; }",
        pass: true,
    },
    Example {
        label: "Rc in sync fn",
        code: "fn f() { let rc = Rc::new(1); }",
        pass: true,
    },
    Example {
        label: "await only inside a nested async block",
        code: "async fn f() { let rc = Rc::new(1); let fut = async { g().await; }; }",
        pass: true,
    },
    Example {
        label: "Rc across await in test module",
        code: "#[cfg(test)]\nmod tests {\n    async fn f() { let rc = Rc::new(1); g().await; }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    nonsend_across_await,
    "Flag `Rc`/`RefCell` bindings in async code when an `.await` occurs later in the same block.",
    "!Send values held across an await point make the entire future !Send, breaking Tokio and other work-stealing runtimes.",
    Medium,
);

fn check_nonsend_across_await(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for function in ctx.nodes::<ast::Fn>().filter(|function| {
        super::support::is_item_or_impl_fn(function)
            && !ctx.is_in_test(function)
            && function.async_token().is_some()
    }) {
        if let Some(block) = function.body() {
            scan_block(ctx, &block, &mut violations);
        }
    }

    for block in ctx
        .nodes::<ast::BlockExpr>()
        .filter(|block| !ctx.is_in_test(block) && block.async_token().is_some())
    {
        scan_block(ctx, &block, &mut violations);
    }

    violations
}

fn ctor_label(path: ast::Path) -> Option<&'static str> {
    let names = super::support::path_names(path);
    let mut rev = names.iter().rev();
    let method = rev.next()?;
    let ty = rev.next()?;

    if ty == "Rc" && method == "new" {
        return Some("Rc::new");
    }

    if ty == "Rc" && method == "clone" {
        return Some("Rc::clone");
    }

    if ty == "RefCell" && method == "new" {
        return Some("RefCell::new");
    }

    None
}

fn nonsend_init(stmt: &ast::Stmt) -> Option<(String, &'static str)> {
    let ast::Stmt::LetStmt(local) = stmt else {
        return None;
    };
    let ast::Expr::CallExpr(call) = local.initializer()? else {
        return None;
    };
    let ast::Expr::PathExpr(func) = call.expr()? else {
        return None;
    };
    let ctor = ctor_label(func.path()?)?;
    let name = match local.pat()? {
        ast::Pat::IdentPat(pat) => pat.name()?.text().to_string(),
        _ => "binding".to_string(),
    };

    Some((name, ctor))
}

fn stmt_awaits(stmt: &ast::Stmt) -> bool {
    node_awaits(stmt.syntax())
}

fn node_awaits(root: &SyntaxNode) -> bool {
    root.descendants()
        .filter_map(ast::AwaitExpr::cast)
        .any(|await_expr| {
            !await_expr
                .syntax()
                .ancestors()
                .skip(1)
                .take_while(|node| node != root)
                .any(|node| {
                    ast::ClosureExpr::cast(node.clone()).is_some()
                        || ast::Item::cast(node.clone()).is_some()
                        || ast::BlockExpr::cast(node)
                            .is_some_and(|block| block.async_token().is_some())
                })
        })
}

fn scan_block(ctx: &AstCtx<'_>, block: &ast::BlockExpr, violations: &mut Vec<Violation>) {
    let Some(list) = block.stmt_list() else {
        return;
    };
    let statements: Vec<_> = list.statements().collect();
    let mut awaits: Vec<bool> = statements.iter().map(stmt_awaits).collect();

    awaits.extend(
        list.tail_expr()
            .map(|expression| node_awaits(expression.syntax())),
    );
    let found: Vec<Violation> = statements
        .iter()
        .enumerate()
        .filter(|&(i, _)| awaits.iter().skip(i + 1).any(|&later| later))
        .filter_map(|(_, stmt)| {
            nonsend_init(stmt).map(|(name, ctor)| {
                ctx.violation(
                    stmt,
                    format!(
                        "`{name}` is created with `{ctor}` and held across a later `.await` — !Send values across await points make the whole future !Send"
                    ),
                )
            })
        })
        .collect();

    violations.extend(found);
}

crate::rulewright_ast_test!(check_nonsend_across_await, {
    crate::example_tests!(EXAMPLES, check_nonsend_across_await);
});
