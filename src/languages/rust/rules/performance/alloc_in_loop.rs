#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{AstNode, ast};

use super::support;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "loop-invariant format in for loop",
        code: r#"fn f(prefix: &str) { for _ in 0..10 { let _ = format!("{} header", prefix); } }"#,
        pass: false,
    },
    Example {
        label: "loop-invariant to_string in while loop",
        code: "fn f(limit: u64) { let mut i = 0; while i < limit { let _ = limit.to_string(); i += 1; } }",
        pass: false,
    },
    Example {
        label: "per-item format is required output",
        code: r#"fn f(items: &[u64]) { for item in items { emit(format!("item {}", item)); } }"#,
        pass: true,
    },
    Example {
        label: "while condition value varies by iteration",
        code: "fn f() { let mut i = 0; while i < 10 { emit(i.to_string()); i += 1; } }",
        pass: true,
    },
    Example {
        label: "lazy error formatting uses its closure binding",
        code: r#"fn f(items: &[Item]) { for item in items { convert(item).map_err(|error| Problem::new(format!("conversion failed: {error}")))?; } }"#,
        pass: true,
    },
    Example {
        label: "formatting on an exiting branch runs at most once",
        code: r#"fn f(items: &[Item], domain: &str) -> Result<(), Error> { for item in items { if invalid(item) { return Err(Error::new(format!("invalid {domain}"))); } } Ok(()) }"#,
        pass: true,
    },
    Example {
        label: "push_str does not necessarily allocate",
        code: "fn f() { let mut s = String::new(); for _ in 0..10 { s.push_str(\"x\"); } }",
        pass: true,
    },
    Example {
        label: "format_args borrows its arguments",
        code: r#"fn f() { for i in 0..10 { let _ = format_args!("item {}", i); } }"#,
        pass: true,
    },
    Example {
        label: "format! outside loop",
        code: r#"fn f() { let _ = format!("hello"); }"#,
        pass: true,
    },
    Example {
        label: "to_string outside loop",
        code: "fn f() { let _ = 42.to_string(); }",
        pass: true,
    },
    Example {
        label: "push_str outside loop",
        code: "fn f() { let mut s = String::new(); s.push_str(\"x\"); }",
        pass: true,
    },
    Example {
        label: "format! in loop in test",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { for i in 0..10 { let _ = format!(\"x{}\", i); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    alloc_in_loop,
    "Flag loop-invariant `format!()` and `.to_string()` work repeated inside loops.",
    "A String built only from values invariant across the nearest loop can be created once before that loop. Per-item strings and lazy error messages are not reported merely because they allocate; use Clippy's format-append lints or profiling when the engineering concern is avoidable allocation rather than loop placement.",
    Medium,
    default = false,
);

fn check_alloc_in_loop(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let methods = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| {
            !ctx.is_in_test(call)
                && loop_invariant(call)
                && !inside_diagnostic(call)
                && !inside_loop_exit(call)
                && !inside_lazy_error_closure(call)
                && !is_mandatory_owned_output(call)
        })
        .filter_map(|call| {
            let method = support::method_name(&call)?;
            let message = if method == "to_string" && support::has_no_args(&call) {
                ".to_string() inside a loop — allocates each iteration"
            } else {
                return None;
            };

            Some(ctx.violation(&call, message))
        });
    let macros = ctx
        .nodes::<ast::MacroCall>()
        .filter(|call| {
            !ctx.is_in_test(call)
                && loop_invariant(call)
                && !inside_diagnostic(call)
                && !inside_loop_exit(call)
                && !inside_lazy_error_closure(call)
                && !is_mandatory_owned_output(call)
        })
        .filter_map(|call| {
            let path = call.path()?;

            if path.qualifier().is_some() {
                return None;
            }

            let name = path.segment()?.name_ref()?.text().to_string();

            (name == "format")
                .then(|| ctx.violation(&call, "format!() inside a loop — allocates each iteration"))
        });

    methods.chain(macros).collect()
}

fn loop_invariant<N>(node: &N) -> bool
where
    N: AstNode,
{
    let Some(body) = support::enclosing_loop_body(node) else {
        return false;
    };
    let bindings = support::loop_variant_bindings(node, &body);

    !support::syntax_references_binding(node.syntax(), &bindings)
}

fn inside_loop_exit<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax().ancestors().skip(1).any(|ancestor| {
        ast::ReturnExpr::can_cast(ancestor.kind()) || ast::BreakExpr::can_cast(ancestor.kind())
    })
}

fn inside_diagnostic<N>(node: &N) -> bool
where
    N: AstNode,
{
    node.syntax().ancestors().skip(1).any(|ancestor| {
        if let Some(call) = ast::MethodCallExpr::cast(ancestor.clone()) {
            return call
                .name_ref()
                .is_some_and(|name| name.text() == "violation");
        }

        let function = ast::CallExpr::cast(ancestor)
            .and_then(|call| call.expr())
            .and_then(|expr| support::path_expr_name(&expr));

        function.is_some_and(|name| name == "violation")
    })
}

fn inside_lazy_error_closure<N>(node: &N) -> bool
where
    N: AstNode,
{
    let Some(closure) = node
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::ClosureExpr::cast)
    else {
        return false;
    };

    closure
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::MethodCallExpr::cast)
        .and_then(|call| call.name_ref())
        .is_some_and(|name| {
            matches!(
                name.text().as_str(),
                "map_err" | "ok_or_else" | "with_context"
            )
        })
}

fn is_mandatory_owned_output<N>(node: &N) -> bool
where
    N: AstNode,
{
    let mut borrowed = false;

    for ancestor in node.syntax().ancestors().skip(1) {
        if ast::RefExpr::can_cast(ancestor.kind()) {
            borrowed = true;
            continue;
        }

        if ast::LetStmt::can_cast(ancestor.kind()) || ast::ExprStmt::can_cast(ancestor.kind()) {
            return false;
        }

        if ast::ArgList::can_cast(ancestor.kind())
            || ast::RecordExprField::can_cast(ancestor.kind())
            || ast::ArrayExpr::can_cast(ancestor.kind())
            || ast::TupleExpr::can_cast(ancestor.kind())
        {
            return !borrowed;
        }

        if ast::MacroCall::can_cast(ancestor.kind()) {
            return !borrowed;
        }

        if let Some(binary) = ast::BinExpr::cast(ancestor)
            && matches!(binary.op_kind(), Some(ast::BinaryOp::Assignment { .. }))
        {
            return !borrowed;
        }
    }

    false
}

crate::rulewright_ast_test!(check_alloc_in_loop, {
    crate::example_tests!(EXAMPLES, check_alloc_in_loop);

    #[gtest]
    fn diagnostic_messages_may_be_built_in_loops() -> Result<()> {
        let source = r#"fn check(ctx: &Ctx) {
            for name in names {
                out.push(ctx.violation(format!("bad {name}")));
                out.push(violation(path, format!("bad {name}")));
            }
        }"#;
        verify_true!(run(source).is_empty())?;

        Ok(())
    }

    #[gtest]
    fn lazy_error_closures_do_not_report_invariant_messages() -> Result<()> {
        let source = r#"fn check(items: &[Item], context: &str) -> Result<(), Error> {
            for item in items {
                lookup(item).ok_or_else(|| {
                    let message = format!("missing {context}");
                    Error::new(message)
                })?;
                convert(item).map_err(|_| {
                    let message = context.to_string();
                    Error::new(message)
                })?;
                load(item).with_context(|| {
                    let message = format!("failed to load {context}");
                    message
                })?;
            }
            Ok(())
        }"#;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn loop_and_match_bindings_make_allocations_variant() -> Result<()> {
        let source = r#"fn check(items: &[Item]) {
            for item in items {
                let label = item.name.to_string();
                inspect(&label);
                match item.kind() {
                    Kind::Named(name) => {
                        let detail = format!("kind {name}");
                        inspect(&detail);
                    }
                    Kind::Other => {}
                }
            }
        }"#;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn while_let_bindings_make_allocations_variant() -> Result<()> {
        let source = r#"fn check(mut items: Items) {
            while let Some(item) = items.next() {
                let label = format!("item {}", item.name);
                inspect(&label);
            }
        }"#;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn mandatory_owned_outputs_are_not_avoidable_allocations() -> Result<()> {
        let source = r#"fn check(items: &[Item], context: &str, rows: &mut Vec<Row>) {
            for item in items {
                rows.push(Row { label: context.to_string() });
                emit(format!("context {context}"));
                rows.push(Row::new(context.to_string()));
                item.label = context.to_string();
            }
        }"#;

        verify_true!(run(source).is_empty())
    }

    #[gtest]
    fn invariant_borrowed_allocations_still_report() -> Result<()> {
        let source = r#"fn check(items: &[Item], context: &str) {
            for item in items {
                inspect(&format!("context {context}"));
                inspect(&context.to_string());
                use_item(item);
            }
        }"#;

        verify_eq!(run(source).len(), 2)
    }
});
