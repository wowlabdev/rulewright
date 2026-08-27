use ra_ap_syntax::{AstNode, ast, ast::HasName};

use super::{super::support::type_name, support};
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Vec::new filled from xs.iter()",
        code: "fn f(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter() { out.push(*x); } }",
        pass: false,
    },
    Example {
        label: "HashMap::new filled from &v",
        code: "fn f(v: Vec<u32>) { let mut m = std::collections::HashMap::new(); for x in &v { m.insert(*x, 1); } }",
        pass: false,
    },
    Example {
        label: "String::new filled over a range",
        code: "fn f(n: usize) { let mut s = String::new(); for _ in 0..n { s.push_str(\"x\"); } }",
        pass: false,
    },
    Example {
        label: "annotated Default::default() filled in sized loop",
        code: "fn f(xs: &[u32]) { let mut out: Vec<u32> = Default::default(); for x in xs.iter() { out.push(*x); } }",
        pass: false,
    },
    Example {
        label: "push nested deeper in loop body",
        code: "fn f(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter() { if *x > 0 { out.push(*x); } } }",
        pass: false,
    },
    Example {
        label: "with_capacity already used",
        code: "fn f(xs: &[u32]) { let mut out = Vec::with_capacity(xs.len()); for x in xs.iter() { out.push(*x); } }",
        pass: true,
    },
    Example {
        label: "consecutive pushes belong to rust_vec_init_then_push, not this rule",
        code: "fn f() { let mut v = Vec::new(); v.push(1); v.push(2); }",
        pass: true,
    },
    Example {
        label: "adapter chain has no knowable length",
        code: "fn f(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter().filter(|x| **x > 0) { out.push(**x); } }",
        pass: true,
    },
    Example {
        label: "collect already sizes via size_hint",
        code: "fn f(xs: &[u32]) -> Vec<u32> { xs.iter().map(|x| x + 1).collect() }",
        pass: true,
    },
    Example {
        label: "sized fill loop in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t(xs: &[u32]) { let mut out = Vec::new(); for x in xs.iter() { out.push(*x); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    missing_capacity,
    "Flag collections built with `new()`/`default()` then grown inside a loop over a sized source.",
    "When the final size is knowable at construction, with_capacity or collect avoids repeated reallocation and copying.",
);

fn check_missing_capacity(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let mut violations = Vec::new();

    for block in ctx.nodes::<ast::StmtList>() {
        if ctx.is_in_test(&block) {
            continue;
        }

        let entries: Vec<ra_ap_syntax::SyntaxNode> = block.syntax().children().collect();

        for (index, entry) in entries.iter().enumerate() {
            // #rw(rust_clone_in_loop) SyntaxNode clone is reference-counted and O(1)
            let Some(local) = ast::LetStmt::cast(entry.clone()) else {
                continue;
            };
            let Some(name) = collection_binding(&local) else {
                continue;
            };

            if grown_in_sized_loop(entries.iter().skip(index + 1), &name) {
                violations.push(ctx.violation(&local, MSG));
            }
        }
    }

    violations
}

const SIZED_COLLECTIONS: &[&str] = &["HashMap", "HashSet", "String", "Vec"];
const GROW_METHODS: &[&str] = &["insert", "push", "push_str"];

const MSG: &str = "collection built without capacity then grown in a loop over a sized source — use with_capacity(...) or collect()";

fn collection_binding(local: &ast::LetStmt) -> Option<String> {
    let ast::Pat::IdentPat(pattern) = local.pat()? else {
        return None;
    };

    pattern.mut_token()?;
    let name = pattern.name()?.text().to_string();
    let initializer = local.initializer()?;

    is_empty_ctor(&initializer, local.ty().as_ref()).then_some(name)
}

fn is_empty_ctor(expr: &ast::Expr, annotated: Option<&ast::Type>) -> bool {
    let ast::Expr::CallExpr(call) = expr else {
        return false;
    };

    if !support::has_no_args(call) {
        return false;
    }

    let Some(ast::Expr::PathExpr(function)) = call.expr() else {
        return false;
    };
    let Some(path) = function.path() else {
        return false;
    };
    let names = support::path_names(&path);
    let Some(last) = names.last() else {
        return false;
    };

    if last == "new" {
        return names
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|ty| SIZED_COLLECTIONS.contains(&ty.as_str()));
    }

    if last == "default" {
        return annotated
            .and_then(type_name)
            .is_some_and(|name| SIZED_COLLECTIONS.contains(&name.as_str()));
    }

    false
}

fn sized_source(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::RefExpr(_) | ast::Expr::RangeExpr(_) => true,
        ast::Expr::MethodCallExpr(call) => support::method_name(call).as_deref() == Some("iter"),
        _ => false,
    }
}

fn grown_in_sized_loop<'a>(
    rest: impl Iterator<Item = &'a ra_ap_syntax::SyntaxNode>,
    name: &str,
) -> bool {
    for entry in rest {
        // #rw(rust_clone_in_loop) SyntaxNode clone is reference-counted and O(1)
        let loop_expr = ast::ForExpr::cast(entry.clone()).or_else(|| {
            // #rw(rust_clone_in_loop) SyntaxNode clone is reference-counted and O(1)
            ast::ExprStmt::cast(entry.clone()).and_then(|statement| match statement.expr()? {
                ast::Expr::ForExpr(loop_expr) => Some(loop_expr),
                _ => None,
            })
        });
        let Some(loop_expr) = loop_expr else {
            continue;
        };

        if !support::loop_source(&loop_expr).is_some_and(|source| sized_source(&source)) {
            continue;
        }

        let Some(body) = support::loop_body(&loop_expr) else {
            continue;
        };

        if body
            .syntax()
            .descendants()
            .filter_map(ast::MethodCallExpr::cast)
            .any(|call| grows_collection(&call, name))
        {
            return true;
        }
    }

    false
}

fn grows_collection(call: &ast::MethodCallExpr, name: &str) -> bool {
    if support::method_name(call).is_none_or(|method| !GROW_METHODS.contains(&method.as_str())) {
        return false;
    }

    call.receiver()
        .and_then(|receiver| support::path_expr_name(&receiver))
        .is_some_and(|receiver| receiver == name)
}

crate::rulewright_ast_test!(check_missing_capacity, {
    crate::example_tests!(EXAMPLES, check_missing_capacity);
});
