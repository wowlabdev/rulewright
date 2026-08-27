use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "panic in Result fn",
        code: r#"fn f() -> Result<(), String> { panic!("x"); }"#,
        pass: false,
    },
    Example {
        label: "panic in non-Result fn",
        code: r#"fn f() -> i32 { panic!("x"); }"#,
        pass: true,
    },
    Example {
        label: "Result fn with Err",
        code: r#"fn f() -> Result<(), String> { Err("x".into()) }"#,
        pass: true,
    },
    Example {
        label: "unwrap in Result fn",
        code: "fn f() -> Result<(), String> { Some(1).unwrap(); Ok(()) }",
        pass: false,
    },
    Example {
        label: "expect in Result fn",
        code: r#"fn f() -> Result<(), String> { Some(1).expect("msg"); Ok(()) }"#,
        pass: false,
    },
    Example {
        label: "panic in Result fn in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f() -> Result<(), String> { panic!(\"x\"); }\n}",
        pass: true,
    },
    Example {
        label: "todo in Result fn",
        code: "fn f() -> Result<(), String> { todo!(); }",
        pass: false,
    },
];

crate::ast_rule!(
    panic_in_result_fn,
    "Ban `panic!`, `.unwrap()`, `.expect()` in functions returning `Result`.",
    "A function returning Result promises fallible error handling. Panicking inside it breaks that contract.",
    High,
);

fn check_panic_in_result_fn(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let macros = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call) && inside_result_fn(call))
        .filter_map(|call| {
            let name = call.path()?.segment()?.name_ref()?.text().to_string();

            PANIC_MACROS.contains(&name.as_str()).then(|| {
                ctx.violation(
                    &call,
                    format!("{name}!() inside a function returning Result — use `?` or return Err"),
                )
            })
        });
    let methods = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call) && inside_result_fn(call))
        .filter_map(|call| {
            let name = call.name_ref()?;

            matches!(name.text().as_str(), "unwrap" | "expect").then(|| {
                ctx.violation(
                    &name,
                    format!(
                        ".{}() inside a function returning Result — use `?` or return Err",
                        name.text()
                    ),
                )
            })
        });

    macros.chain(methods).collect()
}

fn returns_result(function: &ast::Fn) -> bool {
    let Some(ast::Type::PathType(path)) = function.ret_type().and_then(|ret| ret.ty()) else {
        return false;
    };

    let name = path
        .path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref());

    name.is_some_and(|name| name.text() == "Result")
}

const PANIC_MACROS: &[&str] = &["panic", "todo", "unimplemented"];

fn inside_result_fn<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax()
        .ancestors()
        .filter_map(ast::Fn::cast)
        .any(|function| returns_result(&function))
}

crate::rulewright_ast_test!(check_panic_in_result_fn, {
    crate::example_tests!(EXAMPLES, check_panic_in_result_fn);
});
