#[cfg(test)]
use googletest::prelude::*;
use std::collections::HashSet;

use ra_ap_syntax::{AstNode, ast, ast::HasArgList, ast::HasName};

use super::support::path_expr_name;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "clone of loop item varies by iteration",
        code: "fn f(v: Vec<String>) { for x in &v { let y = x.clone(); } }",
        pass: true,
    },
    Example {
        label: "clone in while loop",
        code: "fn f(s: &String) { while true { let y = s.clone(); } }",
        pass: false,
    },
    Example {
        label: "clone in loop loop",
        code: "fn f(s: &String) { loop { let y = s.clone(); break; } }",
        pass: false,
    },
    Example {
        label: "to_owned in loop",
        code: "fn f(s: &str) { loop { let y = s.to_owned(); break; } }",
        pass: false,
    },
    Example {
        label: "clone outside loop",
        code: "fn f(s: &String) { let y = s.clone(); }",
        pass: true,
    },
    Example {
        label: "clone in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn f(v: Vec<String>) { for x in &v { let y = x.clone(); } }\n}",
        pass: true,
    },
];

crate::ast_rule!(
    clone_in_loop,
    "Flag `.clone()` and `.to_owned()` calls on loop-invariant receivers inside loop bodies.",
    "A loop-invariant ownership conversion may be avoidable or movable outside the loop. Borrow or restructure when possible. \
     Conversions of values bound inside the loop are not reported because they commonly express a required per-item ownership transfer. \
     This syntax-only rule cannot distinguish heap copies from cheap Arc, Rc, or Copy clones, so intentional cheap clones should be suppressed with a reason.",
    Medium,
    default = false,
);

fn check_clone_in_loop(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let clone_calls = ctx
        .nodes::<ast::MethodCallExpr>()
        .filter(|call| !ctx.is_in_test(call))
        .filter(|call| {
            call.name_ref()
                .is_some_and(|name| matches!(name.text().as_str(), "clone" | "to_owned"))
                && call
                    .arg_list()
                    .is_none_or(|arguments| arguments.args().next().is_none())
        })
        .filter(|call| enclosing_loop_body(call).is_some_and(|body| !receiver_varies(call, &body)));

    clone_calls
        .filter_map(|call| {
            let method = call.name_ref()?.text().to_string();

            Some(ctx.violation(
                &call,
                format!(".{method}() inside a loop — consider borrowing or restructuring to avoid repeated ownership conversions"),
            ))
        })
        .collect()
}

fn enclosing_loop_body(call: &ast::MethodCallExpr) -> Option<ast::BlockExpr> {
    call.syntax()
        .ancestors()
        .filter_map(ast::BlockExpr::cast)
        .find(|body| {
            body.syntax().parent().is_some_and(|parent| {
                ast::ForExpr::can_cast(parent.kind())
                    || ast::WhileExpr::can_cast(parent.kind())
                    || ast::LoopExpr::can_cast(parent.kind())
            })
        })
}

fn receiver_varies(call: &ast::MethodCallExpr, body: &ast::BlockExpr) -> bool {
    let mut bindings: HashSet<String> = body
        .syntax()
        .descendants()
        .filter(|node| node.text_range().start() < call.syntax().text_range().start())
        .filter_map(ast::IdentPat::cast)
        .filter_map(|pattern| pattern.name().map(|name| name.text().to_string()))
        .collect();

    if let Some(for_loop) = body.syntax().parent().and_then(ast::ForExpr::cast)
        && let Some(pattern) = for_loop.pat()
    {
        bindings.extend(
            pattern
                .syntax()
                .descendants()
                .filter_map(ast::IdentPat::cast)
                .filter_map(|pattern| pattern.name().map(|name| name.text().to_string())),
        );
    }

    call.receiver().is_some_and(|receiver| {
        receiver
            .syntax()
            .descendants()
            .filter_map(ast::PathExpr::cast)
            .filter_map(|path| path_expr_name(&ast::Expr::PathExpr(path)))
            .any(|name| bindings.contains(&name))
    })
}

crate::rulewright_ast_test!(check_clone_in_loop, {
    crate::example_tests!(EXAMPLES, check_clone_in_loop);

    #[gtest]
    fn clone_in_for_iterator_is_evaluated_once() -> Result<()> {
        verify_true!(run("fn f(tokens: Tokens) { for token in tokens.clone() {} }").is_empty())?;

        Ok(())
    }

    #[gtest]
    fn clone_of_local_created_inside_loop_is_not_reported() -> Result<()> {
        verify_true!(
            run("fn f(values: &[&str]) { for value in values { let owned = value.to_string(); let copy = owned.clone(); } }")
                .is_empty()
        )?;

        Ok(())
    }

    #[gtest]
    fn clone_of_outer_value_inside_nested_loop_is_reported() -> Result<()> {
        verify_eq!(
            run("fn f(values: &[String]) { for value in values { for _ in 0..2 { let copy = value.clone(); } } }")
                .len(),
            1
        )
    }

    #[gtest]
    fn syntax_only_rule_is_opt_in() -> Result<()> {
        let rule = crate::all_rules()
            .into_iter()
            .find(|rule| rule.name == "rust_clone_in_loop")
            .or_fail()?;

        verify_false!(rule.default_enabled)
    }
});
