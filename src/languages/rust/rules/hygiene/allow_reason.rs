use crate::{Example, FileCtx, Violation, infra::parse, violation};

#[rustfmt::skip]
const EXAMPLES: &[Example] = &[
    Example {
        label: "allow without comment",
        code: "#[allow(dead_code)]",
        pass: false,
    },
    Example {
        label: "allow with inline comment",
        code: "#[allow(dead_code)] // webhook response fields",
        pass: true,
    },
    Example {
        label: "allow with preceding comment",
        code: "// DBC fields use PascalCase\n#[allow(non_snake_case)]",
        pass: true,
    },
    Example {
        label: "allow with reason argument",
        code: "#[allow(dead_code, reason = \"webhook response fields\")]",
        pass: true,
    },
    Example {
        label: "expect without reason",
        code: "#[expect(clippy::unused_async)]",
        pass: false,
    },
    Example {
        label: "expect with reason argument",
        code: "#[expect(clippy::unused_async, reason = \"API fixed, will use I/O later\")]",
        pass: true,
    },
    Example {
        label: "multiline allow with reason argument",
        code: "#[allow(\n    dead_code,\n    reason = \"webhook response fields\",\n)]",
        pass: true,
    },
    Example {
        label: "multiline expect with reason argument",
        code: "#[expect(\n    clippy::cast_precision_loss,\n    reason = \"sample count enters the f64 domain\",\n)]",
        pass: true,
    },
    Example {
        label: "multiline module expect with reason argument",
        code: "#![expect(\n    clippy::doc_markdown,\n    reason = \"generated documentation is external\",\n)]",
        pass: true,
    },
    Example {
        label: "multiline expect without reason",
        code: "#[expect(\n    clippy::cast_precision_loss,\n    clippy::cast_sign_loss,\n)]",
        pass: false,
    },
    Example {
        label: "module-level allow without comment",
        code: "#![allow(clippy::too_many_arguments)]",
        pass: false,
    },
    Example {
        label: "module-level allow with comment",
        code: "// compatibility callback has many parameters by design\n#![allow(clippy::too_many_arguments)]",
        pass: true,
    },
    Example {
        label: "non-allow attr",
        code: "#[derive(Debug)]",
        pass: true,
    },
    Example {
        label: "deny attr",
        code: "#[deny(unused)]",
        pass: true,
    },
];

crate::line_rule!(
    allow_reason,
    "Require a `reason = \"...\"` or comment explaining why `#[allow(...)]`/`#[expect(...)]` is used.",
    "Unexplained lint overrides hide whether a warning is understood or merely silenced. Keep the override as narrow as practical and state the concrete reason the lint does not apply at that location; do not restate the lint name.",
);

fn is_expect_attr(trimmed: &str) -> bool {
    trimmed.starts_with("#[expect(") || trimmed.starts_with("#![expect(")
}

fn code_before_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;

    while index < bytes.len() {
        let Some(&byte) = bytes.get(index) else {
            break;
        };

        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            return line.get(..index).unwrap_or(line);
        }

        index += 1;
    }

    line
}

fn contains_reason(attribute_line: &str) -> bool {
    attribute_line.contains("reason = \"") || attribute_line.contains("reason=\"")
}

fn attribute_span_has_reason(lines: &[&str], start: usize) -> bool {
    let mut depth = 0_u32;
    let mut entered_attribute = false;

    for line in lines.iter().skip(start) {
        let code = code_before_line_comment(line);

        if contains_reason(code) {
            return true;
        }

        let mut in_string = false;
        let mut escaped = false;

        for byte in code.bytes() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }

                continue;
            }

            match byte {
                b'"' => in_string = true,

                b'(' => {
                    entered_attribute = true;
                    depth += 1;
                }

                b')' if entered_attribute => {
                    depth = depth.saturating_sub(1);

                    if depth == 0 {
                        return false;
                    }
                }

                _ => {}
            }
        }
    }

    false
}

fn check_allow_reason(ctx: &FileCtx<'_>) -> Vec<Violation> {
    let mut out = Vec::new();

    for (i, line) in ctx.lines.iter().enumerate() {
        let trimmed = line.trim();

        if !parse::is_allow_attr(trimmed) && !is_expect_attr(trimmed) {
            continue;
        }

        if attribute_span_has_reason(ctx.lines, i) {
            continue;
        }

        if parse::has_inline_comment(trimmed) {
            continue;
        }

        if i.checked_sub(1)
            .and_then(|previous| ctx.lines.get(previous))
            .is_some_and(|previous| parse::is_comment(previous.trim()))
        {
            continue;
        }

        out.push(violation(
            ctx.rel,
            i + 1,
            "lint override without a `reason = \"...\"` or comment explaining why",
        ));
    }

    out
}

crate::rulewright_test!(check_allow_reason, {
    crate::example_tests!(EXAMPLES, check_allow_reason);
});
