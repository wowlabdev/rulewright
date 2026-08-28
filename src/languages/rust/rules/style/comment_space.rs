use crate::{Example, FileCtx, Fix, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "no space after //",
        code: "//comment",
        pass: false,
    },
    Example {
        label: "space after //",
        code: "// comment",
        pass: true,
    },
    Example {
        label: "doc comment",
        code: "/// doc comment",
        pass: true,
    },
    Example {
        label: "inner doc comment",
        code: "//! inner doc",
        pass: true,
    },
    Example {
        label: "hash prefix",
        code: "//# header",
        pass: true,
    },
    Example {
        label: "bare double slash",
        code: "//",
        pass: true,
    },
    Example {
        label: "not a comment",
        code: "let x = 1;",
        pass: true,
    },
];

crate::line_rule!(
    comment_space,
    "Require a space after `//` in comments (`//bad` -> `// good`).",
    "Missing space after // makes comments harder to read and looks like accidentally commented-out code.",
    Low,
    fix_comment_space,
);

fn check_comment_space(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        if !parse::is_comment(trimmed) {
            continue;
        }

        let Some(ch) = parse::char_after_slashes(trimmed) else {
            continue;
        };

        if matches!(ch, ' ' | '/' | '!' | '#' | '[') {
            continue;
        }

        out.push(violation(
            ctx.rel,
            lineno,
            "missing space after `//` in comment",
        ));
    }

    out
}

fn fix_comment_space(ctx: &FileCtx<'_>, v: &Violation) -> Option<Fix> {
    let line = ctx.line(v.line)?;

    Some(Fix::replace_line(v.line, line.replacen("//", "// ", 1)))
}

crate::rulewright_test!(check_comment_space, {
    crate::example_tests!(EXAMPLES, check_comment_space);
    crate::fix_tests!(EXAMPLES, line, check_comment_space, fix_comment_space);
});
