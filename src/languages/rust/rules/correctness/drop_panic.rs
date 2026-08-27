use ra_ap_syntax::{AstNode, ast};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "panic in Drop",
        code: "struct Foo;\nimpl Drop for Foo {\n  fn drop(&mut self) { panic!(\"oh no\"); }\n}",
        pass: false,
    },
    Example {
        label: "unwrap in Drop",
        code: "struct Foo;\nimpl Drop for Foo {\n  fn drop(&mut self) { Some(1).unwrap(); }\n}",
        pass: false,
    },
    Example {
        label: "expect in Drop",
        code: "struct Foo;\nimpl Drop for Foo {\n  fn drop(&mut self) { Some(1).expect(\"msg\"); }\n}",
        pass: false,
    },
    Example {
        label: "println in Drop",
        code: "struct Foo;\nimpl Drop for Foo {\n  fn drop(&mut self) { println!(\"dropping\"); }\n}",
        pass: true,
    },
    Example {
        label: "panic outside Drop",
        code: "struct Foo;\nimpl Foo {\n  fn bar(&self) { panic!(\"ok\"); }\n}",
        pass: true,
    },
    Example {
        label: "todo in Drop",
        code: "struct Foo;\nimpl Drop for Foo {\n  fn drop(&mut self) { todo!(); }\n}",
        pass: false,
    },
    Example {
        label: "panic in Drop in test module",
        code: "#[cfg(test)]\nmod tests {\n  struct Foo;\n  impl Drop for Foo {\n    fn drop(&mut self) { panic!(\"test\"); }\n  }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    drop_panic,
    "Ban `panic!`, `.unwrap()`, `.expect()` inside `impl Drop`.",
    "Panicking in Drop causes a double-panic abort. Drop must be infallible to avoid crashing the entire process.",
    High,
);

fn check_drop_panic(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let macros = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call) && inside_drop(call))
        .filter_map(|call| {
            let name = call.path()?.segment()?.name_ref()?.text().to_string();

            matches!(name.as_str(), "panic" | "todo" | "unimplemented").then(|| {
                ctx.violation(
                    &call,
                    "panic-family macro in Drop impl — this can cause a double-panic abort",
                )
            })
        });
    let methods = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call) && inside_drop(call))
        .filter_map(|call| {
            let name = call.name_ref()?;

            matches!(name.text().as_str(), "unwrap" | "expect").then(|| {
                ctx.violation(
                    &name,
                    format!(
                        ".{}() in Drop impl — this can cause a double-panic abort",
                        name.text()
                    ),
                )
            })
        });

    macros.chain(methods).collect()
}

fn inside_drop<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax()
        .ancestors()
        .filter_map(ast::Impl::cast)
        .filter_map(|item_impl| item_impl.trait_())
        .any(|ty| {
            ty.syntax()
                .descendants()
                .filter_map(ast::NameRef::cast)
                .last()
                .is_some_and(|name| name.text() == "Drop")
        })
}

crate::rulewright_ast_test!(check_drop_panic, {
    crate::example_tests!(EXAMPLES, check_drop_panic);
});
