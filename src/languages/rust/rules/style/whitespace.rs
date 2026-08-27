#[cfg(test)]
use googletest::prelude::*;

use crate::{Example, FileCtx, Fix, Violation, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "clean file",
        code: "fn main() {}",
        pass: true,
    },
    Example {
        label: "trailing whitespace",
        code: "let x = 1;   \nlet y = 2;",
        pass: false,
    },
    Example {
        label: "tab character",
        code: "\tlet x = 1;",
        pass: false,
    },
];

crate::line_rule!(
    style,
    "Enforce no trailing whitespace, no tabs, no CRLF line endings.",
    "Trailing whitespace, tabs, and CRLF endings cause noisy diffs and merge conflicts.",
    Low,
    fix_style,
);

fn check_style(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;

        if *line != line.trim_end() {
            out.push(violation(ctx.rel, lineno, "trailing whitespace"));
        }

        if line.contains('\t') {
            out.push(violation(ctx.rel, lineno, "tab character (use spaces)"));
        }

        if line.contains('\r') {
            out.push(violation(ctx.rel, lineno, "CR line ending (use LF)"));
        }
    }

    out
}

fn fix_style(ctx: &FileCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.line(v.line)?;
    let fixed = line.replace('\t', "    ").replace('\r', "");

    Some(Fix::replace_line(v.line, fixed.trim_end()))
}

crate::rulewright_test!(check_style, {
    crate::example_tests!(EXAMPLES, check_style);
    crate::fix_tests!(line, check_style, fix_style);

    #[gtest]
    fn cr_line_ending() -> Result<()> {
        let v = run("let x = 1;\r");
        verify_false!(v.is_empty())?;
        verify_true!(v.iter().any(|v| v.message.contains("CR line ending")))?;

        Ok(())
    }

    #[gtest]
    fn multiple_violations_on_one_line() -> Result<()> {
        let v = run("\tlet x = 1;  ");
        verify_eq!(v.len(), 2)?;

        Ok(())
    }
});
