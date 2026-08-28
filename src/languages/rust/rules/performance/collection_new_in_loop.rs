use ra_ap_syntax::{AstNode, ast};

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
        label: "allocation-free empty string used as state",
        code: "fn f(xs: &[String]) { for _ in 0..2 { let mut previous = String::new(); for value in xs { if value != &previous { previous = value.clone(); } } } }",
        pass: true,
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
        label: "returned collection must remain iteration-local",
        code: "fn f(n: usize) -> Vec<u32> { for _ in 0..n { let v = Vec::new(); if !v.is_empty() { return v; } } Vec::new() }",
        pass: true,
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
        label: "escapes through assignment into an output value",
        code: "struct Row { values: Vec<u32> } fn f(rows: &mut [Row]) { for row in rows { let mut values = Vec::with_capacity(4); values.push(1); row.values = values; } }",
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
    "A fresh collection on every iteration may be avoidable allocation. Reuse one buffer with clear() only when iterations do not retain or return its contents and retained capacity is acceptable; otherwise keep the ownership boundary and suppress or tune the rule.",
    Medium,
);

fn check_collection_new_in_loop(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::LetStmt>()
        .filter(|local| !ctx.is_in_test(local) && support::is_inside_loop_body(local))
        .filter_map(|local| {
            let initializer = local.initializer()?;

            let allocation = collection_ctor(&initializer)?;
            let name = local_ident(local.pat()?)?;
            let block = local.syntax().parent().and_then(ast::StmtList::cast)?;

            if allocation == Allocation::Deferred && !grows_collection(&block, &name) {
                return None;
            }

            (!escapes_from_block(&block, &name)).then(|| ctx.violation(&local, MSG))
        })
        .collect()
}

const COLLECTION_TYPES: &[&str] = &[
    "BTreeMap", "HashMap", "HashSet", "String", "Vec", "VecDeque",
];
const MIN_CTOR_SEGMENTS: usize = 2;

const MSG: &str = "scratch collection allocates inside a loop — consider hoisting it and calling .clear() between iterations, but keep it local when an iteration retains or returns the collection";

#[derive(Clone, Copy, Eq, PartialEq)]
enum Allocation {
    Deferred,
    Immediate,
}

fn collection_ctor(expr: &ast::Expr) -> Option<Allocation> {
    match expr {
        ast::Expr::CallExpr(call) => {
            let Some(ast::Expr::PathExpr(function)) = call.expr() else {
                return None;
            };
            let path = function.path()?;
            let names = support::path_names(&path);
            let last = names.last()?;

            if last == "with_capacity" {
                return (names.len() >= MIN_CTOR_SEGMENTS).then_some(Allocation::Immediate);
            }

            if last == "new" && support::has_no_args(call) {
                return names
                    .iter()
                    .rev()
                    .nth(1)
                    .is_some_and(|ty| COLLECTION_TYPES.contains(&ty.as_str()))
                    .then_some(Allocation::Deferred);
            }

            None
        }

        ast::Expr::MacroExpr(mac) => mac
            .macro_call()
            .filter(|call| {
                call.path().is_some_and(|path| {
                    path.qualifier().is_none()
                        && path
                            .segment()
                            .and_then(|segment| segment.name_ref())
                            .is_some_and(|name| name.text() == "vec")
                })
            })
            .and_then(|call| {
                let tokens = call.token_tree()?.syntax().text().to_string();
                let contents = tokens
                    .strip_prefix('[')
                    .and_then(|tokens| tokens.strip_suffix(']'))?
                    .trim();

                (!contents.is_empty()).then_some(Allocation::Immediate)
            }),

        _ => None,
    }
}

fn grows_collection(block: &ast::StmtList, name: &str) -> bool {
    const GROWING_METHODS: &[&str] = &[
        "extend",
        "insert",
        "push",
        "push_back",
        "push_front",
        "push_str",
        "reserve",
        "reserve_exact",
        "resize",
        "resize_with",
    ];

    block
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
        .any(|call| {
            call.name_ref()
                .is_some_and(|method| GROWING_METHODS.contains(&method.text().as_str()))
                && call.receiver().is_some_and(|receiver| {
                    support::path_expr_name(&receiver).is_some_and(|receiver| receiver == name)
                })
        })
}

fn local_ident(pattern: ast::Pat) -> Option<String> {
    support::ident_pattern_name(pattern)
}

fn escapes_from_block(block: &ast::StmtList, name: &str) -> bool {
    block
        .syntax()
        .descendants()
        .filter_map(ast::PathExpr::cast)
        .filter(|path| {
            support::path_expr_name(&ast::Expr::PathExpr(path.clone())).as_deref() == Some(name)
        })
        .any(|path| !is_non_escaping_method_receiver(&path))
}

fn is_non_escaping_method_receiver(path: &ast::PathExpr) -> bool {
    const CONSUMING_METHODS: &[&str] = &[
        "into_boxed_slice",
        "into_iter",
        "into_keys",
        "into_values",
        "leak",
    ];

    path.syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::MethodCallExpr::cast)
        .is_some_and(|call| {
            call.receiver()
                .is_some_and(|receiver| receiver.syntax() == path.syntax())
                && call
                    .name_ref()
                    .is_none_or(|method| !CONSUMING_METHODS.contains(&method.text().as_str()))
        })
}

crate::rulewright_ast_test!(check_collection_new_in_loop, {
    crate::example_tests!(EXAMPLES, check_collection_new_in_loop);
});
