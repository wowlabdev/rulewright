use ra_ap_syntax::{AstNode, ast, ast::HasArgList};

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Vec::new inside for loop, receiver-only use",
        code: "fn f(n: usize) { for _ in 0..n { let mut buf = Vec::new(); buf.push(1); } }",
        pass: false,
    },
    Example {
        label: "String::new inside while loop",
        code: "fn f(n: usize) { let mut i = 0; while i < n { let mut s = String::new(); s.push('x'); i += 1; } }",
        pass: false,
    },
    Example {
        label: "vec! macro inside loop",
        code: "fn f(n: usize) { for _ in 0..n { let v = vec![1, 2]; let _ = v.len(); } }",
        pass: false,
    },
    Example {
        label: "with_capacity inside loop",
        code: "fn f(n: usize) { for _ in 0..n { let mut m = std::collections::HashMap::with_capacity(8); m.insert(1, 1); } }",
        pass: false,
    },
    Example {
        label: "binding in nested block inside loop",
        code: "fn f(xs: &[u32]) { for x in xs { if *x > 0 { let mut v = Vec::new(); v.push(*x); } } }",
        pass: false,
    },
    Example {
        label: "return escape still flagged (approximation: only call-argument use counts as escaping)",
        code: "fn f(n: usize) -> Vec<u32> { for _ in 0..n { let v = Vec::new(); if !v.is_empty() { return v; } } Vec::new() }",
        pass: false,
    },
    Example {
        label: "escapes as method-call argument into outer collection",
        code: "fn f(n: usize, out: &mut Vec<Vec<u32>>) { for i in 0..n { let mut row = Vec::new(); row.push(i as u32); out.push(row); } }",
        pass: true,
    },
    Example {
        label: "escapes as plain function-call argument",
        code: "fn g(v: Vec<u32>) { drop(v); } fn f(n: usize) { for _ in 0..n { let v = Vec::new(); g(v); } }",
        pass: true,
    },
    Example {
        label: "hoisted allocation cleared per iteration",
        code: "fn f(n: usize) { let mut buf = Vec::new(); for _ in 0..n { buf.push(1); buf.clear(); } }",
        pass: true,
    },
    Example {
        label: "constructor outside any loop",
        code: "fn f() { let mut v = Vec::new(); v.push(1); }",
        pass: true,
    },
    Example {
        label: "constructor in loop in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t(n: usize) { for _ in 0..n { let mut v = Vec::new(); v.push(1); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    collection_new_in_loop,
    "Flag collection constructors (`Vec::new()`, `vec![]`, `with_capacity`, ...) bound via `let` inside loops.",
    "Allocating a fresh collection per iteration is invisible overhead — hoist it out of the loop and .clear() each round.",
    Medium,
);

fn check_collection_new_in_loop(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::LetStmt>()
        .filter(|local| !ctx.is_in_test(local) && support::is_inside_loop_body(local))
        .filter_map(|local| {
            let initializer = local.initializer()?;

            if !is_collection_ctor(&initializer) {
                return None;
            }

            let name = local_ident(local.pat()?)?;
            let block = local.syntax().parent().and_then(ast::StmtList::cast)?;

            (!escapes_from_block(&block, &name)).then(|| ctx.violation(&local, MSG))
        })
        .collect()
}

const COLLECTION_TYPES: &[&str] = &[
    "BTreeMap", "HashMap", "HashSet", "String", "Vec", "VecDeque",
];
const MIN_CTOR_SEGMENTS: usize = 2;

const MSG: &str = "collection allocated inside a loop — hoist it out and .clear() per iteration (escape check is best-effort: only call-argument use counts as escaping)";

fn is_collection_ctor(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::CallExpr(call) => {
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

            if last == "with_capacity" {
                return names.len() >= MIN_CTOR_SEGMENTS;
            }

            if last == "new" && support::has_no_args(call) {
                return names
                    .iter()
                    .rev()
                    .nth(1)
                    .is_some_and(|ty| COLLECTION_TYPES.contains(&ty.as_str()));
            }

            false
        }
        ast::Expr::MacroExpr(mac) => {
            mac.macro_call()
                .and_then(|call| call.path())
                .is_some_and(|path| {
                    path.qualifier().is_none()
                        && path
                            .segment()
                            .and_then(|segment| segment.name_ref())
                            .is_some_and(|name| name.text() == "vec")
                })
        }
        _ => false,
    }
}

fn local_ident(pattern: ast::Pat) -> Option<String> {
    support::ident_pattern_name(pattern)
}

fn escapes_from_block(block: &ast::StmtList, name: &str) -> bool {
    let plain_calls = block.syntax().descendants().filter_map(ast::CallExpr::cast);
    let method_calls = block
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast);

    let mut arguments = plain_calls
        .filter_map(|call| call.arg_list())
        .chain(method_calls.filter_map(|call| call.arg_list()))
        .flat_map(|arguments| arguments.args());

    arguments.any(|argument| support::syntax_contains_ident(argument.syntax(), name))
}

crate::rulewright_ast_test!(check_collection_new_in_loop, {
    crate::example_tests!(EXAMPLES, check_collection_new_in_loop);
});
