#[cfg(test)]
use googletest::prelude::*;
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};

use crate::{AstCtx, Example, Violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "short function",
        code: "fn f() { let x = 1; }",
        pass: true,
    },
];

crate::ast_rule!(
    max_fn_lines,
    "Flag functions longer than the configured nonblank-line threshold.",
    "Very long functions are hard to understand, test, and review. Extract cohesive operations with honest names or split distinct responsibilities; do not create forwarding helpers that merely move arbitrary line ranges, and tune the threshold when the domain is clearer as one procedure.",
    Medium,
    params {
        threshold: i64 = 150
    },
);

fn check_max_fn_lines(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let max_fn_lines = ctx
        .file
        .config
        .get_usize("rust_max_fn_lines", &MAX_FN_LINES_PARAMS[0]);

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            let body = function.body()?;
            let source = body.syntax().text().to_string();
            let lines = source
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count();

            (lines > max_fn_lines).then(|| {
                let name = function.name()?;

                Some(ctx.violation(
                    &name,
                    format!("function `{name}` is {lines} lines long (max {max_fn_lines})"),
                ))
            })?
        })
        .collect()
}

crate::rulewright_ast_test!(check_max_fn_lines, {
    use std::fmt::Write as _;

    crate::example_tests!(EXAMPLES, check_max_fn_lines);

    #[gtest]
    fn long_fn_fails() -> Result<()> {
        let mut src = String::from("fn long() {\n");
        for i in 0..155 {
            let _ = writeln!(src, "    let _x{i} = {i};");
        }
        src.push_str("}\n");
        let v = run(&src);
        verify_eq!(v.len(), 1)?;
        verify_true!(v[0].message.contains("long"))?;
        verify_true!(v[0].message.contains("lines long"))?;

        Ok(())
    }

    #[gtest]
    fn blank_lines_do_not_consume_the_function_budget() -> Result<()> {
        let mut source = String::from("fn spaced() {\n");

        for i in 0..140 {
            let _ = writeln!(source, "    let _x{i} = {i};\n");
        }

        source.push_str("}\n");

        verify_true!(run(&source).is_empty())
    }
});
