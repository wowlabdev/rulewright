use ra_ap_syntax::{AstNode, ast, syntax_editor::SyntaxEditor};

use super::support::parse_expr;
use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "dbg! left in code",
        code: "fn f() { dbg!(1); }",
        pass: false,
    },
    Example {
        label: "no dbg",
        code: "fn f() { let x = 1; }",
        pass: true,
    },
    Example {
        label: "dbg in string literal",
        code: r#"fn f() { let s = "dbg!(value)"; }"#,
        pass: true,
    },
    Example {
        label: "dbg in test module",
        code: "#[cfg(test)]\nmod tests {\n    fn t() { dbg!(1); }\n}",
        pass: true,
    },
    Example {
        label: "production dbg with test module",
        code: "fn prod() { dbg!(1); }\n#[cfg(test)]\nmod tests {\n    fn t() { dbg!(2); }\n}",
        pass: false,
    },
];

crate::ast_tree_rule!(
    dbg,
    "Ban `dbg!()` macro calls in production code.",
    "dbg!() writes to stderr and is meant for temporary debugging. Leaving it in production pollutes output.",
    Medium,
    fix_dbg,
);

fn check_dbg(ctx: &AstCtx<'_>) -> Vec<Violation> {
    ctx.nodes::<ast::MacroCall>()
        .filter(|call| !ctx.is_in_test(call) && is_unqualified_macro(call, "dbg"))
        .map(|call| ctx.violation(&call, "dbg!() call left in code"))
        .collect()
}

fn is_unqualified_macro(call: &ast::MacroCall, expected: &str) -> bool {
    call.path().is_some_and(|path| {
        path.qualifier().is_none()
            && path
                .segment()
                .and_then(|segment| segment.name_ref())
                .is_some_and(|name| name.text() == expected)
    })
}

// #rw(fn: rust_clone_in_loop) SyntaxEditor owns both replacement syntax nodes for each edit
fn fix_dbg(ctx: &AstCtx<'_>, _violations: &[Violation]) -> Option<String> {
    let (editor, root) = SyntaxEditor::with_ast_node(ctx.root);
    let mut changed = false;

    for call in root
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter(|call| !ctx.is_in_test(call) && is_unqualified_macro(call, "dbg"))
    {
        let inner = macro_inner(&call)?;
        let replacement = parse_expr(&inner)?;

        editor.replace(call.syntax().clone(), replacement.syntax().clone());
        changed = true;
    }

    changed.then(|| editor.finish().new_root().to_string())
}

fn macro_inner(call: &ast::MacroCall) -> Option<String> {
    let text = call.token_tree()?.syntax().text().to_string();
    let mut chars = text.chars();
    let open = chars.next()?;
    let close = text.chars().last()?;

    if !matches!((open, close), ('(', ')') | ('[', ']') | ('{', '}')) {
        return None;
    }

    text.get(open.len_utf8()..text.len().checked_sub(close.len_utf8())?)
        .map(str::to_owned)
}

crate::rulewright_ast_test!(check_dbg, {
    crate::example_tests!(EXAMPLES, check_dbg);
    crate::fix_tests!(ast_tree, check_dbg, fix_dbg);
});
