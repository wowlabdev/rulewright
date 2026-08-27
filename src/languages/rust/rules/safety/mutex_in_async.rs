use ra_ap_syntax::{
    AstNode,
    ast::{self},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "std Mutex in async fn",
        code: "async fn f() { let _m = std::sync::Mutex::new(0); }",
        pass: false,
    },
    Example {
        label: "std Mutex in sync fn",
        code: "fn f() { let _m = std::sync::Mutex::new(0); }",
        pass: true,
    },
    Example {
        label: "std Mutex type in async fn",
        code: "async fn f() { let _m: std::sync::Mutex<i32> = std::sync::Mutex::new(0); }",
        pass: false,
    },
    Example {
        label: "Mutex in async in test",
        code: "#[cfg(test)]\nmod tests {\n    async fn t() { let _m = std::sync::Mutex::new(0); }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    mutex_in_async,
    "Flag `std::sync::Mutex` usage in async functions under an async-mutex-only policy.",
    "Repositories that prohibit blocking mutexes in async code can opt into an explicit scheduler policy.",
    High,
    default = false,
);

fn check_mutex_in_async(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let expression_paths = ctx
        .nodes::<ast::PathExpr>()
        .filter(|path| is_in_async_fn(path) && !ctx.is_in_test(path))
        .filter(|path| path.path().is_some_and(|path| is_std_mutex_path(&path)));
    let expression_violations = expression_paths
        .map(|path| {
            ctx.violation(
                &path,
                "std::sync::Mutex in async function — use tokio::sync::Mutex to avoid blocking the runtime",
            )
        });
    let type_paths = ctx
        .nodes::<ast::PathType>()
        .filter(|path| is_in_async_fn(path) && !ctx.is_in_test(path))
        .filter(|path| path.path().is_some_and(|path| is_std_mutex_path(&path)));
    let type_violations = type_paths.map(|path| {
        ctx.violation(
            &path,
            "std::sync::Mutex type in async function — use tokio::sync::Mutex",
        )
    });

    expression_violations.chain(type_violations).collect()
}

fn is_in_async_fn<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::Fn::cast)
        .is_some_and(|function| function.async_token().is_some())
}

// #rw(fn: rust_recursive_fn) syntax path qualifier depth is parser-bounded and naturally recursive
fn is_std_mutex_path(path: &ast::Path) -> bool {
    let current_is_mutex = path
        .segment()
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| name.text() == "Mutex");
    let sync_qualifier = path.qualifier().filter(|qualifier| {
        qualifier
            .segment()
            .and_then(|segment| segment.name_ref())
            .is_some_and(|name| name.text() == "sync")
    });

    if current_is_mutex && sync_qualifier.is_some() {
        let parent_segment = sync_qualifier
            .and_then(|sync| sync.qualifier())
            .and_then(|qualifier| qualifier.segment());
        let parent = parent_segment.and_then(|segment| segment.name_ref());

        return parent.is_none_or(|name| name.text() != "tokio");
    }

    path.qualifier()
        .is_some_and(|qualifier| is_std_mutex_path(&qualifier))
}

crate::rulewright_ast_test!(check_mutex_in_async, {
    crate::example_tests!(EXAMPLES, check_mutex_in_async);
});
