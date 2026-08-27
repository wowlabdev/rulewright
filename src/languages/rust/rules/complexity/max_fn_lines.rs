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
    "Flag functions longer than threshold lines.",
    "Functions over 150 lines are hard to understand, test, and review. Break them into smaller focused functions.",
    Medium,
    params {
        threshold: i64 = 150
    },
);

fn check_max_fn_lines(ctx: &AstCtx<'_>) -> Vec<Violation> {
    let max_fn_lines = ctx.file.config.get_usize("rust_max_fn_lines", &PARAMS[0]);

    ctx.nodes::<ast::Fn>()
        .filter(|function| !ctx.is_in_test(function))
        .filter_map(|function| {
            let body = function.body()?;
            let range = body.syntax().text_range();
            let start = ctx.line_index.line_col(range.start()).line as usize;
            let end = ctx.line_index.line_col(range.end()).line as usize;
            let lines = end.saturating_sub(start);

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
});
