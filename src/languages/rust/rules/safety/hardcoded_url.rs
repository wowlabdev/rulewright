use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "hardcoded https URL",
        code: r#"fn f() { let url = "https://api.example.com/v1"; }"#,
        pass: false,
    },
    Example {
        label: "hardcoded http URL",
        code: r#"fn f() { let url = "http://localhost:3000"; }"#,
        pass: false,
    },
    Example {
        label: "no URL",
        code: "fn f() { let x = 42; }",
        pass: true,
    },
    Example {
        label: "URL in doc comment",
        code: "/// See https://docs.rs/foo for details.",
        pass: true,
    },
    Example {
        label: "URL in regular comment",
        code: "// Reference: https://example.com/spec",
        pass: true,
    },
];

crate::line_rule!(
    hardcoded_url,
    "Flag hardcoded URLs in source code (should use config/env).",
    "Hardcoded URLs break when environments change. Use configuration or environment variables for host-specific URLs.",
    Medium,
);

fn check_hardcoded_url(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let trimmed = line.trim();

        if parse::is_comment(trimmed) {
            continue;
        }

        if let Some(idx) = parse::find_url(line) {
            let Some(before) = line.get(..idx) else {
                continue;
            };
            let has_open_quote = before.contains('"');

            if has_open_quote {
                out.push(violation(
                    ctx.rel,
                    i + 1,
                    "hardcoded URL in source — use configuration or environment variable instead",
                ));
            }
        }
    }

    out
}

crate::rulewright_test!(check_hardcoded_url, {
    crate::example_tests!(EXAMPLES, check_hardcoded_url);
});
