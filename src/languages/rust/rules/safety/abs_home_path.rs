use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "Users path in string",
        code: r#"let p = "/Users/john/file";"#,
        pass: false,
    },
    Example {
        label: "home path in string",
        code: r#"let p = "/home/user/data";"#,
        pass: false,
    },
    Example {
        label: "Windows path in string",
        code: r#"let p = "C:\\Users\\john\\file";"#,
        pass: false,
    },
    Example {
        label: "tmp path",
        code: r#"let p = "/tmp/file";"#,
        pass: true,
    },
    Example {
        label: "comment with path",
        code: "// /Users/foo",
        pass: true,
    },
    Example {
        label: "no quotes",
        code: "let users = get_users();",
        pass: true,
    },
];

crate::line_rule!(
    abs_home_path,
    "Ban hardcoded home directory paths like `/Users/` or `/home/` in string literals.",
    "Absolute home paths break on other machines and in CI. Use environment variables or relative paths.",
    Medium,
);

const HOME_PATTERNS: &[&str] = &["/Users/", "/home/", "C:\\\\Users\\\\", "C:\\Users\\"];

fn check_abs_home_path(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let lineno = i + 1;
        let trimmed = line.trim();

        if parse::is_comment(trimmed) {
            continue;
        }

        if !line.contains('"') {
            continue;
        }

        for pattern in HOME_PATTERNS {
            if line.contains(pattern) {
                out.push(violation(
                    ctx.rel,
                    lineno,
                    format!(
                        "hardcoded home directory path `{pattern}` in string literal \
                         (use env vars or config)"
                    ),
                ));
                break;
            }
        }
    }

    out
}

crate::rulewright_test!(check_abs_home_path, {
    crate::example_tests!(EXAMPLES, check_abs_home_path);
});
