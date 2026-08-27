use ra_ap_syntax::{
    ast,
    ast::{HasArgList, HasName},
};

use crate::{AstCtx, Example, Violation};

const ADJACENT_STATEMENTS: usize = 2;

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Vec::new then push",
        code: "fn f() { let mut v = Vec::new(); v.push(1); v.push(2); }",
        pass: false,
    },
    Example {
        label: "vec! macro is fine",
        code: "fn f() { let v = vec![1, 2]; }",
        pass: true,
    },
    Example {
        label: "Vec::new with_capacity is fine",
        code: "fn f() { let mut v = Vec::with_capacity(10); v.push(1); }",
        pass: true,
    },
    Example {
        label: "Vec::new then push in test",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { let mut v = Vec::new(); v.push(1); }\n}",
        pass: true,
    },
    Example {
        label: "Vec::new with logic between",
        code: "fn f() { let mut v = Vec::new(); let x = 1; if x > 0 { v.push(x); } }",
        pass: true,
    },
    Example {
        label: "annotated Vec binding is outside the simple-pattern rule",
        code: "fn f() { let mut v: Vec<u32> = Vec::new(); v.push(1); }",
        pass: true,
    },
];

crate::ast_rule!(
    vec_init_then_push,
    "Flag `Vec::new()` immediately followed by `.push()` calls (use `vec![]` or `with_capacity`).",
    "Vec::new() followed by push() calls can be replaced with vec![...] or Vec::with_capacity for clarity and performance.",
);

fn check_vec_init_then_push(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for statements in ctx.nodes::<ast::StmtList>() {
        let entries: Vec<ast::Stmt> = statements.statements().collect();

        for pair in entries.windows(ADJACENT_STATEMENTS) {
            let ast::Stmt::LetStmt(local) = &pair[0] else {
                continue;
            };

            if ctx.is_in_test(local) || !local.initializer().is_some_and(|expr| is_vec_new(&expr)) {
                continue;
            }

            let Some(name) = let_mut_name(local) else {
                continue;
            };

            if is_push_on(&pair[1], &name) {
                violations.push(ctx.violation(
                    local,
                    "Vec::new() immediately followed by .push() — use vec![...] or Vec::with_capacity()",
                ));
            }
        }
    }

    violations
}

fn is_vec_new(expr: &ast::Expr) -> bool {
    let ast::Expr::CallExpr(call) = expr else {
        return false;
    };
    let Some(ast::Expr::PathExpr(function)) = call.expr() else {
        return false;
    };
    let Some(path) = function.path() else {
        return false;
    };
    let last = path
        .segment()
        .and_then(|segment| segment.name_ref())
        .map(|name| name.text().to_string());
    let qualifier = path.qualifier().and_then(|qualifier| qualifier.segment());
    let owner = qualifier
        .and_then(|segment| segment.name_ref())
        .map(|name| name.text().to_string());

    owner.as_deref() == Some("Vec")
        && last.as_deref() == Some("new")
        && call
            .arg_list()
            .is_none_or(|arguments| arguments.args().next().is_none())
}

fn let_mut_name(local: &ast::LetStmt) -> Option<String> {
    if local.ty().is_some() {
        return None;
    }

    let ast::Pat::IdentPat(pattern) = local.pat()? else {
        return None;
    };

    pattern.mut_token()?;

    pattern.name().map(|name| name.text().to_string())
}

fn is_push_on(statement: &ast::Stmt, name: &str) -> bool {
    let ast::Stmt::ExprStmt(statement) = statement else {
        return false;
    };
    let Some(ast::Expr::MethodCallExpr(call)) = statement.expr() else {
        return false;
    };

    if call.name_ref().is_none_or(|method| method.text() != "push") {
        return false;
    }

    let Some(ast::Expr::PathExpr(receiver)) = call.receiver() else {
        return false;
    };

    let name_ref = receiver
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());

    name_ref.is_some_and(|receiver| receiver.text() == name)
}

crate::rulewright_ast_test!(check_vec_init_then_push, {
    crate::example_tests!(EXAMPLES, check_vec_init_then_push);
});
