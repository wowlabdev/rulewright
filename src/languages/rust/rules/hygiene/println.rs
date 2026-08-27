use ra_ap_syntax::{AstNode, ast, syntax_editor::SyntaxEditor};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "println in library",
        code: "fn f() { println!(\"hello\"); }",
        pass: false,
    },
    Example {
        label: "eprintln in library",
        code: "fn f() { eprintln!(\"error\"); }",
        pass: false,
    },
    Example {
        label: "print in library",
        code: "fn f() { print!(\"hello\"); }",
        pass: false,
    },
    Example {
        label: "eprint in library",
        code: "fn f() { eprint!(\"error\"); }",
        pass: false,
    },
    Example {
        label: "println in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { println!(\"debug\"); }\n}",
        pass: true,
    },
    Example {
        label: "no println",
        code: "fn f() { let x = 1; }",
        pass: true,
    },
    Example {
        label: "println in string literal",
        code: r#"fn f() { let s = "println!(value)"; }"#,
        pass: true,
    },
];

crate::ast_tree_rule!(
    println,
    "Ban `println!`/`eprintln!`/`print!`/`eprint!` in library code.",
    "Console printing bypasses structured logging. Use tracing or the output module for consistent, filterable output.",
    Medium,
    fix_println,
);

const BANNED_MACROS: &[&str] = &["println", "eprintln", "print", "eprint"];

fn check_println(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call))
        .filter_map(|call| {
            let name = unqualified_macro_name(&call)?;

            BANNED_MACROS.contains(&name.as_str()).then(|| {
                ctx.violation(
                    &call,
                    format!("{name}!() in library code (use tracing or return errors)"),
                )
            })
        })
        .collect()
}

fn unqualified_macro_name(call: &ast::MacroCall) -> Option<String> {
    let path = call.path()?;

    if path.qualifier().is_some() {
        return None;
    }

    path.segment()?
        .name_ref()
        .map(|name| name.text().to_string())
}

// #rw(fn: rust_clone_in_loop) SyntaxEditor requires an owned deletion target per macro call
fn fix_println(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for call in root
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter(|call| !ctx.is_in_test(call))
        .filter(|call| {
            unqualified_macro_name(call).is_some_and(|name| BANNED_MACROS.contains(&name.as_str()))
        })
    {
        let target = call
            .syntax()
            .parent()
            .and_then(ast::ExprStmt::cast)
            .map_or_else(
                || call.syntax().clone(),
                |statement| statement.syntax().clone(),
            );

        editor.delete(target);
        changed = true;
    }

    changed.then(|| editor.finish().new_root().to_string())
}

crate::rulewright_ast_test!(check_println, {
    crate::example_tests!(EXAMPLES, check_println);
    crate::fix_tests!(ast_tree, check_println, fix_println);
});
