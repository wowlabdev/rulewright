use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "unwrap in map closure",
        code: r"fn f(v: Vec<Option<i32>>) -> Vec<i32> { v.into_iter().map(|x| x.unwrap()).collect() }",
        pass: false,
    },
    Example {
        label: "expect in filter_map",
        code: r#"fn f(v: Vec<&str>) -> Vec<i32> { v.iter().filter_map(|x| Some(x.parse::<i32>().expect("bad"))).collect() }"#,
        pass: false,
    },
    Example {
        label: "unwrap in for_each",
        code: r"fn f(v: Vec<Option<i32>>) { v.iter().for_each(|x| { x.unwrap(); }); }",
        pass: false,
    },
    Example {
        label: "no unwrap in map",
        code: "fn f(v: Vec<i32>) -> Vec<i32> { v.into_iter().map(|x| x + 1).collect() }",
        pass: true,
    },
    Example {
        label: "unwrap outside iterator",
        code: "fn f() { Some(1).unwrap(); }",
        pass: true,
    },
    Example {
        label: "unwrap in iterator in test",
        code: "#[cfg(test)]\nmod tests {\n    fn t(v: Vec<Option<i32>>) { v.into_iter().map(|x| x.unwrap()); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    fallible_in_iterator,
    "Flag `.unwrap()`/`.expect()` inside iterator adapter closures.",
    "unwrap/expect inside iterator adapters panics mid-iteration with no recovery. Use filter_map or collect::<Result<_>>.",
    Medium,
);

fn check_fallible_in_iterator(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let method = call.name_ref()?;

            if !matches!(method.text().as_str(), "unwrap" | "expect")
                || !inside_iterator_closure(&call)
            {
                return None;
            }

            Some(ctx.violation(
                &method,
                format!(
                    ".{}() inside iterator adapter — use ? or filter_map instead",
                    method.text()
                ),
            ))
        })
        .collect()
}

const ITERATOR_METHODS: &[&str] = &[
    "map",
    "filter",
    "filter_map",
    "flat_map",
    "for_each",
    "find",
    "find_map",
    "any",
    "all",
    "inspect",
    "scan",
    "fold",
    "reduce",
    "try_fold",
    "try_for_each",
    "partition",
];

fn inside_iterator_closure(call: &ast::MethodCallExpr) -> bool {
    let closure = call
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::ClosureExpr::cast);
    let adapter = closure.and_then(|closure| {
        closure
            .syntax()
            .ancestors()
            .skip(1)
            .find_map(ast::MethodCallExpr::cast)
    });

    adapter
        .and_then(|adapter| adapter.name_ref())
        .is_some_and(|name| ITERATOR_METHODS.contains(&name.text().as_str()))
}

crate::rulewright_ast_test!(check_fallible_in_iterator, {
    crate::example_tests!(EXAMPLES, check_fallible_in_iterator);
});
